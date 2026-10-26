//! Links tab: share the personal room link and join a call by URL or name.

use std::rc::Rc;

use adw::prelude::*;

use super::padded_column;
use crate::store::Store;

pub struct LinksView {
    pub root: gtk::Box,
    link_row: adw::ActionRow,
    active_label: gtk::Label,
    store: Store,
}

impl LinksView {
    pub fn new(store: Store) -> Rc<LinksView> {
        let root = padded_column();

        let share = adw::PreferencesGroup::new();
        share.set_title("Your personal room");
        share.set_description(Some("Anyone with this link can join you there. Copy it to invite a guest or teammate."));
        let link_row = adw::ActionRow::new();
        link_row.set_title("Loading…");
        link_row.add_css_class("property");
        let copy = gtk::Button::from_icon_name("edit-copy-symbolic");
        copy.set_tooltip_text(Some("Copy link"));
        copy.set_valign(gtk::Align::Center);
        copy.add_css_class("flat");
        link_row.add_suffix(&copy);
        share.add(&link_row);
        let join_copy = gtk::Button::with_label("Join call and copy link");
        join_copy.add_css_class("suggested-action");
        join_copy.add_css_class("pill");
        join_copy.set_halign(gtk::Align::Center);
        join_copy.set_margin_top(12);
        root.append(&share);
        root.append(&join_copy);

        let join_group = adw::PreferencesGroup::new();
        join_group.set_title("Join a call by link");
        join_group.set_margin_top(12);
        let entry_row = adw::EntryRow::new();
        entry_row.set_title("https://tuple.app/c/… or a person's name");
        entry_row.set_show_apply_button(true);
        join_group.add(&entry_row);
        root.append(&join_group);

        let active_label = gtk::Label::new(None);
        active_label.add_css_class("caption-dim");
        active_label.set_wrap(true);
        active_label.set_xalign(0.0);
        active_label.set_selectable(true);
        root.append(&active_label);

        let view = Rc::new(LinksView { root, link_row, active_label, store });
        copy.connect_clicked({
            let view = view.clone();
            move |button| button.clipboard().set_text(&view.personal_url())
        });
        join_copy.connect_clicked({
            let view = view.clone();
            move |button| {
                let (url, slug, in_call) = {
                    let state = view.store.state();
                    match &state.personal_room {
                        Some(room) => (room.url.clone(), room.slug.clone(), state.in_call()),
                        None => return,
                    }
                };
                button.clipboard().set_text(&url);
                view.store.join_room(&slug, in_call);
            }
        });
        entry_row.connect_apply({
            let view = view.clone();
            move |row| {
                let target = row.text().trim().to_string();
                if !target.is_empty() {
                    let in_call = view.store.state().in_call();
                    view.store.join(&target, in_call);
                    row.set_text("");
                }
            }
        });
        view.store.subscribe({
            let view = view.clone();
            move |_| view.render()
        });
        view.render();
        view
    }

    fn personal_url(&self) -> String {
        self.store.state().personal_room.as_ref().map(|r| r.url.clone()).unwrap_or_default()
    }

    fn render(&self) {
        let url = self.personal_url();
        self.link_row.set_title(if url.is_empty() { "No personal room yet" } else { &url });
        let state = self.store.state();
        match state.call.as_ref().and_then(|c| c.room.as_ref()) {
            Some(room) => self.active_label.set_text(&format!("Current call: {}", room.url)),
            None => self.active_label.set_text(""),
        }
    }
}
