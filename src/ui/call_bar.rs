//! In-call bar at the bottom of the window: title, participants and the control strip
//! (mic, screen share, Capture, add, react, leave) in the order the Mac app uses.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use super::{avatar, clear};
use crate::avatars::AvatarLoader;
use crate::model::{Call, Participant};
use crate::store::Store;
use crate::viewmodel::{call_title, duration_text, hangup_label, participant_detail, participants_text};

const REACTIONS: [(&str, &str); 3] = [("📯 Airhorn", "airhorn"), ("🎺 Trombone", "trombone"), ("🪈 Flute", "flute")];

pub struct CallBar {
    pub root: gtk::Box,
    title: gtk::Label,
    duration: gtk::Label,
    participants_label: gtk::Label,
    participants: gtk::Box,
    mute: gtk::ToggleButton,
    share: gtk::ToggleButton,
    capture: gtk::ToggleButton,
    leave: gtk::Button,
    add_menu: gtk::Popover,
    store: Store,
    loader: AvatarLoader,
    rendered: RefCell<Option<Call>>,
}

impl CallBar {
    pub fn new(store: Store, loader: AvatarLoader) -> Rc<CallBar> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("call-bar");
        root.add_css_class("toolbar");
        root.set_visible(false);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::new(None);
        title.add_css_class("heading");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let duration = gtk::Label::new(None);
        duration.add_css_class("numeric");
        duration.add_css_class("dim-label");
        header.append(&title);
        header.append(&duration);
        root.append(&header);

        let participants_label = gtk::Label::new(None);
        participants_label.add_css_class("dim-label");
        participants_label.set_xalign(0.0);
        participants_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        root.append(&participants_label);
        let participants = gtk::Box::new(gtk::Orientation::Vertical, 2);
        root.append(&participants);

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let mute = control("microphone-sensitivity-muted-symbolic", "Mute / unmute (Ctrl+M)");
        let share = control("video-display-symbolic", "Share / stop sharing your screen (Ctrl+Shift+S)");
        let capture = control("media-record-symbolic", "Start / stop Capture (Ctrl+Shift+C)");
        let add = gtk::MenuButton::new();
        add.set_icon_name("list-add-symbolic");
        add.set_tooltip_text(Some("Add another participant"));
        let add_menu = gtk::Popover::new();
        add.set_popover(Some(&add_menu));
        let react = gtk::MenuButton::new();
        react.set_icon_name("face-smile-symbolic");
        react.set_tooltip_text(Some("Send a sound reaction"));
        let react_menu = gtk::Popover::new();
        let react_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        for (label, sound) in REACTIONS {
            let button = gtk::Button::with_label(label);
            button.add_css_class("flat");
            button.connect_clicked({
                let store = store.clone();
                let popover = react_menu.clone();
                move |_| {
                    store.react(sound);
                    popover.popdown();
                }
            });
            react_box.append(&button);
        }
        react_menu.set_child(Some(&react_box));
        react.set_popover(Some(&react_menu));
        let leave = gtk::Button::with_label("Leave");
        leave.add_css_class("destructive-action");
        leave.set_tooltip_text(Some("Leave the call (Ctrl+Shift+L)"));
        leave.set_hexpand(true);
        leave.set_halign(gtk::Align::End);
        for widget in [mute.upcast_ref::<gtk::Widget>(), share.upcast_ref(), capture.upcast_ref(), add.upcast_ref(), react.upcast_ref(), leave.upcast_ref()] {
            controls.append(widget);
        }
        root.append(&controls);

        let bar = Rc::new(CallBar {
            root,
            title,
            duration,
            participants_label,
            participants,
            mute,
            share,
            capture,
            leave,
            add_menu,
            store,
            loader,
            rendered: RefCell::new(None),
        });
        bar.mute.connect_clicked({
            let store = bar.store.clone();
            move |_| store.toggle_mute()
        });
        bar.share.connect_clicked({
            let store = bar.store.clone();
            move |_| store.toggle_share()
        });
        bar.capture.connect_clicked({
            let store = bar.store.clone();
            move |_| store.toggle_capture()
        });
        bar.leave.connect_clicked({
            let store = bar.store.clone();
            move |_| store.leave("current")
        });
        bar.store.subscribe({
            let bar = bar.clone();
            move |_| bar.render()
        });
        glib::timeout_add_seconds_local(1, {
            let bar = bar.clone();
            move || {
                bar.tick();
                glib::ControlFlow::Continue
            }
        });
        bar.render();
        bar
    }

    fn render(&self) {
        let state = self.store.state();
        let Some(call) = &state.call else {
            self.root.set_visible(false);
            *self.rendered.borrow_mut() = None;
            return;
        };
        if self.rendered.borrow().as_ref() == Some(call) && self.root.is_visible() {
            return;
        }
        *self.rendered.borrow_mut() = Some(call.clone());
        self.title.set_text(&call_title(&state));
        self.participants_label.set_text(&participants_text(call));
        set_toggle(&self.mute, call.muted, "Unmute", "Mute");
        set_toggle(&self.share, call.sharing, "Stop sharing", "Share screen");
        set_toggle(&self.capture, call.transcribing, "Stop Capture", "Capture");
        self.leave.set_label(hangup_label(call));
        self.render_participants(call);
        self.render_add_menu(&state);
        self.tick();
        self.root.set_visible(true);
    }

    fn render_participants(&self, call: &Call) {
        clear(&self.participants);
        for participant in call.others() {
            self.participants.append(&self.participant_row(participant));
        }
    }

    fn participant_row(&self, participant: &Participant) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&avatar(&participant.person, 24, None, &self.loader));
        let name = gtk::Label::new(Some(&participant.person.full_name));
        name.set_xalign(0.0);
        name.set_hexpand(true);
        row.append(&name);
        let detail = gtk::Label::new(Some(&participant_detail(participant)));
        detail.add_css_class("dim-label");
        row.append(&detail);
        let kick = gtk::Button::from_icon_name("list-remove-symbolic");
        kick.add_css_class("flat");
        kick.set_tooltip_text(Some("Remove from call"));
        kick.connect_clicked({
            let store = self.store.clone();
            let target = if participant.person.email.is_empty() { participant.person.full_name.clone() } else { participant.person.email.clone() };
            move |_| store.remove_participant(&target)
        });
        row.append(&kick);
        row
    }

    fn render_add_menu(&self, state: &crate::model::AppState) {
        let in_call: Vec<i64> = state.call.iter().flat_map(|c| c.participants.iter().map(|p| p.person.id)).collect();
        let candidates: Vec<_> = state.sorted_contacts().into_iter().filter(|c| !in_call.contains(&c.person.id)).collect();
        let present: Vec<_> = candidates.iter().copied().filter(|c| c.is_present()).collect();
        let shown = if present.is_empty() { candidates.iter().copied().take(12).collect() } else { present };
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        if shown.is_empty() {
            let label = gtk::Label::new(Some("No one to add"));
            label.add_css_class("dim-label");
            list.append(&label);
        }
        for contact in shown {
            let button = gtk::Button::with_label(&format!("{} ({})", contact.person.full_name, contact.status));
            button.add_css_class("flat");
            button.connect_clicked({
                let store = self.store.clone();
                let email = contact.person.email.clone();
                let popover = self.add_menu.clone();
                move |_| {
                    store.add_participant(&email);
                    popover.popdown();
                }
            });
            list.append(&button);
        }
        self.add_menu.set_child(Some(&list));
    }

    fn tick(&self) {
        let state = self.store.state();
        match &state.call {
            Some(call) => self.duration.set_text(&duration_text(call, chrono::Utc::now())),
            None => self.duration.set_text(""),
        }
    }
}

fn control(icon: &str, tooltip: &str) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_icon_name(icon);
    button.set_tooltip_text(Some(tooltip));
    button
}

fn set_toggle(button: &gtk::ToggleButton, active: bool, active_label: &str, idle_label: &str) {
    button.set_active(active);
    button.set_tooltip_text(Some(if active { active_label } else { idle_label }));
    if active {
        button.add_css_class("call-active");
    } else {
        button.remove_css_class("call-active");
    }
}
