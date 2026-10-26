//! People tab: search, missed-call banner and the sectioned contact list.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use super::{avatar, clear, confirm, icon_button, padded_column, prompt, scrolled, section};
use crate::avatars::AvatarLoader;
use crate::model::{AppState, Contact};
use crate::store::Store;
use crate::viewmodel::{group_contacts, invite_person, status_text};

pub struct PeopleView {
    pub root: gtk::Box,
    pub search: gtk::SearchEntry,
    list: gtk::Box,
    banner: adw::Banner,
    store: Store,
    loader: AvatarLoader,
    rendered: RefCell<Option<(Vec<Contact>, bool, String)>>,
    dismissed_missed: RefCell<Vec<String>>,
}

impl PeopleView {
    pub fn new(store: Store, loader: AvatarLoader) -> Rc<PeopleView> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let banner = adw::Banner::new("");
        banner.set_button_label(Some("Call Back"));
        root.append(&banner);

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.set_margin_top(8);
        top.set_margin_start(12);
        top.set_margin_end(12);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search people"));
        search.set_hexpand(true);
        top.append(&search);
        let invite = icon_button("contact-new-symbolic", "Invite someone to Tuple by email");
        top.append(&invite);
        root.append(&top);

        let list = padded_column();
        root.append(&scrolled(&list));

        let view = Rc::new(PeopleView {
            root,
            search,
            list,
            banner,
            store,
            loader,
            rendered: RefCell::new(None),
            dismissed_missed: RefCell::new(Vec::new()),
        });
        view.search.connect_search_changed({
            let view = view.clone();
            move |_| view.render(true)
        });
        invite.connect_clicked({
            let view = view.clone();
            move |button| {
                let store = view.store.clone();
                prompt(button, "Invite to Tuple", "name@company.com", "Invite", move |email| store.invite(&email));
            }
        });
        view.banner.connect_button_clicked({
            let view = view.clone();
            move |_| view.call_back()
        });
        view.store.subscribe({
            let view = view.clone();
            move |_| view.render(false)
        });
        view.render(true);
        view
    }

    pub fn focus_search(&self) {
        self.search.grab_focus();
    }

    fn render(&self, force: bool) {
        let state = self.store.state();
        self.render_missed(&state);
        let key = (state.contacts.clone(), state.in_call(), self.search.text().to_string());
        if !force && self.rendered.borrow().as_ref() == Some(&key) {
            return;
        }
        *self.rendered.borrow_mut() = Some(key);
        clear(&self.list);
        let sections = group_contacts(&state, &self.search.text());
        if sections.is_empty() {
            let text = if self.search.text().is_empty() { "No contacts yet. Invite a teammate." } else { "No one matches your search." };
            let label = gtk::Label::new(Some(text));
            label.add_css_class("dim-label");
            label.set_margin_top(24);
            self.list.append(&label);
            return;
        }
        for (title, contacts) in sections {
            let (container, list) = section(title);
            for contact in contacts {
                list.append(&self.row(contact, state.in_call()));
            }
            self.list.append(&container);
        }
    }

    fn render_missed(&self, state: &AppState) {
        let dismissed = self.dismissed_missed.borrow();
        let pending = state.missed_calls.iter().filter(|m| !dismissed.contains(&m.to_string())).last();
        match pending {
            Some(missed) => {
                self.banner.set_title(&format!("Missed call from {}", invite_person(missed).full_name));
                self.banner.set_revealed(true);
            }
            None => self.banner.set_revealed(false),
        }
    }

    fn call_back(&self) {
        let state = self.store.state();
        let dismissed = self.dismissed_missed.borrow().clone();
        if let Some(missed) = state.missed_calls.iter().filter(|m| !dismissed.contains(&m.to_string())).last() {
            let person = invite_person(missed);
            let target = if person.email.is_empty() { person.full_name } else { person.email };
            self.store.call_person(&target);
            self.dismissed_missed.borrow_mut().push(missed.to_string());
        }
        drop(state);
        self.banner.set_revealed(false);
    }

    fn row(&self, contact: &Contact, in_call: bool) -> adw::ActionRow {
        let row = adw::ActionRow::new();
        row.set_title(&glib::markup_escape_text(&contact.person.full_name));
        row.set_subtitle(&status_text(contact));
        row.set_tooltip_text(Some(&contact.person.email));
        row.add_prefix(&avatar(&contact.person, 36, Some(&contact.status), &self.loader));

        let star = icon_button(
            if contact.favorited { "starred-symbolic" } else { "non-starred-symbolic" },
            if contact.favorited { "Remove from favorites" } else { "Add to favorites" },
        );
        star.connect_clicked({
            let store = self.store.clone();
            let email = contact.person.email.clone();
            let favorited = contact.favorited;
            move |_| store.favorite_contact(&email, !favorited)
        });
        row.add_suffix(&star);

        let call = gtk::Button::with_label(if in_call { "Add" } else { "Call" });
        call.set_valign(gtk::Align::Center);
        call.set_tooltip_text(Some(&if in_call {
            format!("Add {} to this call", contact.person.short_name)
        } else {
            format!("Call {}", contact.person.short_name)
        }));
        if contact.is_present() {
            call.add_css_class("suggested-action");
        }
        call.connect_clicked({
            let store = self.store.clone();
            let email = contact.person.email.clone();
            move |_| if in_call { store.add_participant(&email) } else { store.call_person(&email) }
        });
        row.add_suffix(&call);

        let remove = icon_button("user-trash-symbolic", "Remove contact");
        remove.connect_clicked({
            let store = self.store.clone();
            let email = contact.person.email.clone();
            let name = contact.person.full_name.clone();
            move |button| {
                let store = store.clone();
                let email = email.clone();
                confirm(button, "Remove contact", &format!("Remove {name} from your contacts?"), "Remove", move || {
                    store.remove_contact(&email)
                });
            }
        });
        remove.set_opacity(0.0);
        row.add_suffix(&remove);
        let hover = gtk::EventControllerMotion::new();
        hover.connect_enter({
            let remove = remove.clone();
            move |_, _, _| remove.set_opacity(1.0)
        });
        hover.connect_leave({
            let remove = remove.clone();
            move |_| remove.set_opacity(0.0)
        });
        row.add_controller(hover);
        row
    }
}
