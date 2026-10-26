//! Incoming call UI: a banner in the main window plus a small separate window,
//! mirroring the always-on-top incoming call window of the Mac and Windows apps.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use super::avatar;
use crate::avatars::AvatarLoader;
use crate::model::AppState;
use crate::store::Store;
use crate::viewmodel::{invite_call_id, invite_person};

pub struct IncomingCall {
    pub banner: adw::Banner,
    window: RefCell<Option<adw::Window>>,
    current: RefCell<Option<String>>,
    store: Store,
    loader: AvatarLoader,
    app: adw::Application,
}

impl IncomingCall {
    pub fn new(store: Store, loader: AvatarLoader, app: adw::Application) -> Rc<IncomingCall> {
        let banner = adw::Banner::new("");
        banner.set_button_label(Some("Accept"));
        let incoming = Rc::new(IncomingCall {
            banner,
            window: RefCell::new(None),
            current: RefCell::new(None),
            store,
            loader,
            app,
        });
        incoming.banner.connect_button_clicked({
            let incoming = incoming.clone();
            move |_| incoming.accept()
        });
        incoming.store.subscribe({
            let incoming = incoming.clone();
            move |state| incoming.render(state)
        });
        incoming
    }

    fn call_id(&self) -> Option<String> {
        self.current.borrow().clone()
    }

    fn accept(&self) {
        if let Some(id) = self.call_id() {
            self.store.accept(&id);
        }
        self.close_window();
    }

    fn render(&self, state: &AppState) {
        let Some(invite) = state.invites.first() else {
            self.banner.set_revealed(false);
            *self.current.borrow_mut() = None;
            self.close_window();
            return;
        };
        let call_id = invite_call_id(invite);
        let person = invite_person(invite);
        let extra = if state.invites.len() > 1 { format!(" (+{} more)", state.invites.len() - 1) } else { String::new() };
        self.banner.set_title(&format!("{} is calling you{extra}", person.full_name));
        self.banner.set_revealed(true);
        if self.current.borrow().as_deref() == Some(call_id.as_str()) {
            return;
        }
        *self.current.borrow_mut() = Some(call_id);
        self.open_window(&person);
    }

    fn open_window(&self, person: &crate::model::Person) {
        self.close_window();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);
        let picture = avatar(person, 72, None, &self.loader);
        picture.set_halign(gtk::Align::Center);
        content.append(&picture);
        let name = gtk::Label::new(Some(&person.full_name));
        name.add_css_class("title-2");
        content.append(&name);
        let caption = gtk::Label::new(Some("is calling you on Tuple"));
        caption.add_css_class("dim-label");
        content.append(&caption);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        buttons.set_halign(gtk::Align::Center);
        buttons.set_margin_top(12);
        let decline = gtk::Button::with_label("Decline");
        decline.add_css_class("destructive-action");
        decline.add_css_class("pill");
        let accept = gtk::Button::with_label("Accept");
        accept.add_css_class("suggested-action");
        accept.add_css_class("pill");
        buttons.append(&decline);
        buttons.append(&accept);
        content.append(&buttons);

        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new("Incoming call", "")));
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
        let window = adw::Window::builder().application(&self.app).title("Incoming call").content(&toolbar).resizable(false).build();
        window.set_default_size(320, -1);
        let store = self.store.clone();
        let id = self.call_id().unwrap_or_default();
        accept.connect_clicked({
            let store = store.clone();
            let id = id.clone();
            let window = window.clone();
            move |_| {
                store.accept(&id);
                window.close();
            }
        });
        decline.connect_clicked({
            let store = store.clone();
            let id = id.clone();
            let window = window.clone();
            move |_| {
                store.leave(&id);
                window.close();
            }
        });
        window.present();
        *self.window.borrow_mut() = Some(window);
    }

    fn close_window(&self) {
        if let Some(window) = self.window.borrow_mut().take() {
            window.close();
        }
    }
}
