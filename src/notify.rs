//! Desktop notifications (org.freedesktop.Notifications) for incoming and missed calls.
//!
//! Each notification runs on its own thread because `notify-rust` blocks while
//! waiting for the user to pick an action; the chosen action comes back on a channel.
//! Incoming-call notifications are closed again as soon as the invite disappears.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_channel::{Receiver, Sender};
use notify_rust::{Notification, Timeout, Urgency};

use crate::model::AppState;
use crate::prefs::Prefs;
use crate::viewmodel::{invite_call_id, invite_person};

const RING_TIMEOUT_MS: u32 = 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationAction {
    Accept(String),
    Decline(String),
    CallBack(String),
}

pub struct Notifier {
    actions: Sender<NotificationAction>,
    seen_invites: HashSet<String>,
    seen_missed: HashSet<String>,
    /// call id -> notification id, for notifications currently on screen.
    shown: Arc<Mutex<HashMap<String, u32>>>,
}

/// Which invites are new (notify) and which notified ones are gone (close).
pub fn diff_invites(seen: &HashSet<String>, current: &HashSet<String>) -> (Vec<String>, Vec<String>) {
    let mut new: Vec<String> = current.difference(seen).cloned().collect();
    let mut gone: Vec<String> = seen.difference(current).cloned().collect();
    new.sort();
    gone.sort();
    (new, gone)
}

impl Notifier {
    pub fn new() -> (Notifier, Receiver<NotificationAction>) {
        let (actions, receiver) = async_channel::unbounded();
        let notifier = Notifier {
            actions,
            seen_invites: HashSet::new(),
            seen_missed: HashSet::new(),
            shown: Arc::new(Mutex::new(HashMap::new())),
        };
        (notifier, receiver)
    }

    /// Show notifications for invites and missed calls that are new since the last state,
    /// and close incoming-call notifications whose invite is gone.
    pub fn on_state(&mut self, state: &AppState, prefs: &Prefs) {
        let invites: HashMap<String, &serde_json::Value> =
            state.invites.iter().filter(|i| i.is_object()).map(|i| (invite_call_id(i), i)).collect();
        let current: HashSet<String> = invites.keys().cloned().collect();
        let (new, gone) = diff_invites(&self.seen_invites, &current);
        for call_id in gone {
            self.seen_invites.remove(&call_id);
            if let Some(id) = self.shown.lock().unwrap().remove(&call_id) {
                std::thread::spawn(move || close_notification(id));
            }
        }
        for call_id in new {
            self.seen_invites.insert(call_id.clone());
            if prefs.notify_incoming_calls {
                self.ring(call_id.clone(), invite_person(invites[&call_id]));
            }
        }
        for missed in state.missed_calls.iter().filter(|m| m.is_object()) {
            if !self.seen_missed.insert(missed.to_string()) {
                continue;
            }
            if prefs.notify_missed_calls {
                self.missed(invite_person(missed));
            }
        }
    }

    fn ring(&self, call_id: String, person: crate::model::Person) {
        let actions = self.actions.clone();
        let shown = self.shown.clone();
        std::thread::spawn(move || {
            let handle = Notification::new()
                .appname("Tuple")
                .summary(&format!("{} is calling you", person.full_name))
                .body("Tuple call")
                .icon("tuple")
                .action("accept", "Accept")
                .action("decline", "Decline")
                .urgency(Urgency::Critical)
                .timeout(Timeout::Milliseconds(RING_TIMEOUT_MS))
                .show();
            let Ok(handle) = handle else { return };
            shown.lock().unwrap().insert(call_id.clone(), handle.id());
            handle.wait_for_action(|action| {
                let chosen = match action {
                    "accept" | "default" => Some(NotificationAction::Accept(call_id.clone())),
                    "decline" => Some(NotificationAction::Decline(call_id.clone())),
                    _ => None,
                };
                if let Some(chosen) = chosen {
                    let _ = actions.send_blocking(chosen);
                }
            });
            shown.lock().unwrap().remove(&call_id);
        });
    }

    fn missed(&self, person: crate::model::Person) {
        let target = if person.email.is_empty() { person.full_name.clone() } else { person.email.clone() };
        let actions = self.actions.clone();
        std::thread::spawn(move || {
            let shown = Notification::new()
                .appname("Tuple")
                .summary(&format!("Missed call from {}", person.full_name))
                .icon("tuple")
                .action("callback", "Call back")
                .show();
            if let Ok(handle) = shown {
                handle.wait_for_action(|action| {
                    if action == "callback" {
                        let _ = actions.send_blocking(NotificationAction::CallBack(target.clone()));
                    }
                });
            }
        });
    }

    /// A plain notice without actions (errors, command results while the window is hidden).
    pub fn notice(title: &str, body: &str) {
        let title = title.to_string();
        let body = body.to_string();
        std::thread::spawn(move || {
            let _ = Notification::new().appname("Tuple").summary(&title).body(&body).icon("tuple").timeout(Timeout::Milliseconds(6000)).show();
        });
    }
}

fn close_notification(id: u32) {
    let Ok(connection) = zbus::blocking::Connection::session() else { return };
    let _ = connection.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.Notifications"),
        "CloseNotification",
        &(id,),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invites_are_diffed_into_new_and_gone() {
        let seen: HashSet<String> = ["a".to_string(), "b".to_string()].into();
        let current: HashSet<String> = ["b".to_string(), "c".to_string()].into();
        assert_eq!(diff_invites(&seen, &current), (vec!["c".to_string()], vec!["a".to_string()]));
        assert_eq!(diff_invites(&current, &current), (vec![], vec![]));
    }
}
