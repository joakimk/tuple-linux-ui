//! Desktop notifications (org.freedesktop.Notifications) for incoming and missed calls.
//!
//! Each notification runs on its own thread because `notify-rust` blocks while
//! waiting for the user to pick an action; the chosen action comes back on a channel.

use std::collections::HashSet;

use async_channel::{Receiver, Sender};
use notify_rust::{Notification, Timeout, Urgency};

use crate::model::AppState;
use crate::prefs::Prefs;
use crate::viewmodel::{invite_call_id, invite_person};

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
}

impl Notifier {
    pub fn new() -> (Notifier, Receiver<NotificationAction>) {
        let (actions, receiver) = async_channel::unbounded();
        (Notifier { actions, seen_invites: HashSet::new(), seen_missed: HashSet::new() }, receiver)
    }

    /// Show notifications for invites and missed calls that are new since the last state.
    pub fn on_state(&mut self, state: &AppState, prefs: &Prefs) {
        for invite in state.invites.iter().filter(|i| i.is_object()) {
            let call_id = invite_call_id(invite);
            if !self.seen_invites.insert(call_id.clone()) {
                continue;
            }
            if !prefs.notify_incoming_calls {
                continue;
            }
            let person = invite_person(invite);
            let actions = self.actions.clone();
            std::thread::spawn(move || {
                let shown = Notification::new()
                    .appname("Tuple")
                    .summary(&format!("{} is calling you", person.full_name))
                    .body("Tuple call")
                    .icon("tuple")
                    .action("accept", "Accept")
                    .action("decline", "Decline")
                    .urgency(Urgency::Critical)
                    .timeout(Timeout::Never)
                    .show();
                if let Ok(handle) = shown {
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
                }
            });
        }
        for missed in state.missed_calls.iter().filter(|m| m.is_object()) {
            let key = missed.to_string();
            if !self.seen_missed.insert(key) {
                continue;
            }
            if !prefs.notify_missed_calls {
                continue;
            }
            let person = invite_person(missed);
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
