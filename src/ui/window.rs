//! The main window: header bar with the account menu, the phase stack, the view
//! switcher tabs, banners, the in-call bar and toasts.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use super::call_bar::CallBar;
use super::history::HistoryView;
use super::incoming::IncomingCall;
use super::links::LinksView;
use super::people::PeopleView;
use super::phases::{connecting_page, SignedOutPage, UnavailablePage};
use super::rooms::RoomsView;
use super::{avatar, settings, shortcuts};
use crate::avatars::AvatarLoader;
use crate::model::{AppState, Phase};
use crate::prefs::Prefs;
use crate::store::{ActionResult, Store};

pub const TABS: [(&str, &str, &str); 4] = [
    ("people", "People", "system-users-symbolic"),
    ("rooms", "Rooms", "user-home-symbolic"),
    ("links", "Links", "insert-link-symbolic"),
    ("history", "History", "document-open-recent-symbolic"),
];

pub struct MainWindow {
    pub window: adw::ApplicationWindow,
    pub toasts: adw::ToastOverlay,
    pages: gtk::Stack,
    tabs: adw::ViewStack,
    people: Rc<PeopleView>,
    history: Rc<HistoryView>,
    unavailable: UnavailablePage,
    signed_out: Rc<SignedOutPage>,
    account: gtk::MenuButton,
    store: Store,
    prefs: Rc<RefCell<Prefs>>,
    loader: AvatarLoader,
}

impl MainWindow {
    pub fn new(app: &adw::Application, store: Store, prefs: Rc<RefCell<Prefs>>, loader: AvatarLoader) -> Rc<MainWindow> {
        let people = PeopleView::new(store.clone(), loader.clone());
        let rooms = RoomsView::new(store.clone(), loader.clone());
        let links = LinksView::new(store.clone());
        let history = HistoryView::new(store.clone());
        let call_bar = CallBar::new(store.clone(), loader.clone());
        let incoming = IncomingCall::new(store.clone(), loader.clone(), app.clone());

        let tabs = adw::ViewStack::new();
        for (name, title, icon) in TABS {
            let widget: &gtk::Box = match name {
                "people" => &people.root,
                "rooms" => &rooms.root,
                "links" => &links.root,
                _ => &history.root,
            };
            let page = tabs.add_titled(widget, Some(name), title);
            page.set_icon_name(Some(icon));
        }
        let switcher = adw::ViewSwitcherBar::new();
        switcher.set_stack(Some(&tabs));
        switcher.set_reveal(true);

        let ready = gtk::Box::new(gtk::Orientation::Vertical, 0);
        ready.append(&incoming.banner);
        tabs.set_vexpand(true);
        ready.append(&tabs);
        ready.append(&call_bar.root);

        let pages = gtk::Stack::new();
        pages.set_transition_type(gtk::StackTransitionType::Crossfade);
        pages.add_named(&connecting_page(), Some("connecting"));
        let unavailable = UnavailablePage::new(store.clone());
        pages.add_named(&unavailable.root, Some("unavailable"));
        let signed_out = SignedOutPage::new(store.clone());
        pages.add_named(&signed_out.root, Some("signed_out"));
        pages.add_named(&ready, Some("ready"));

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new("Tuple", "")));
        let account = gtk::MenuButton::new();
        account.set_tooltip_text(Some("Account"));
        account.set_icon_name("avatar-default-symbolic");
        account.set_menu_model(Some(&account_menu()));
        header.pack_end(&account);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&pages));
        toolbar.add_bottom_bar(&switcher);
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&toolbar));

        let (width, height) = {
            let p = prefs.borrow();
            (p.window_width as i32, p.window_height as i32)
        };
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Tuple")
            .default_width(width)
            .default_height(height)
            .width_request(360)
            .height_request(420)
            .content(&toasts)
            .build();

        let main = Rc::new(MainWindow {
            window,
            toasts,
            pages,
            tabs,
            people,
            history,
            unavailable,
            signed_out,
            account,
            store,
            prefs,
            loader,
        });
        main.window.connect_close_request({
            let main = main.clone();
            move |window| {
                let (w, h) = (window.width(), window.height());
                {
                    let mut prefs = main.prefs.borrow_mut();
                    prefs.window_width = w as f32;
                    prefs.window_height = h as f32;
                    let _ = prefs.save();
                }
                window.set_visible(false);
                glib::Propagation::Stop
            }
        });
        main.tabs.connect_visible_child_name_notify({
            let main = main.clone();
            move |tabs| {
                if let Some(name) = tabs.visible_child_name() {
                    main.on_tab_changed(name.as_str());
                }
            }
        });
        main.store.subscribe({
            let main = main.clone();
            move |state| main.render(state)
        });
        let initial_tab = main.prefs.borrow().last_tab.clone();
        main.show_tab(&initial_tab);
        main.render(&main.store.state());
        main
    }

    fn render(&self, state: &AppState) {
        let page = match state.phase() {
            Phase::Unavailable => "unavailable",
            Phase::Connecting => "connecting",
            Phase::SignedOut => "signed_out",
            Phase::Ready => "ready",
        };
        if page == "unavailable" {
            self.unavailable.set_error(&state.error);
        }
        self.pages.set_visible_child_name(page);
        self.window.set_title(Some(if state.in_call() { "Tuple — In call" } else { "Tuple" }));
        if let Some(user) = &state.user {
            self.account.set_tooltip_text(Some(&user.full_name));
            self.account.set_child(Some(&avatar(user, 24, None, &self.loader)));
        }
    }

    pub fn on_result(&self, result: &ActionResult) {
        self.signed_out.on_result(result);
        if !result.result.ok() {
            let toast = adw::Toast::new(&format!("{}: {}", result.label.replace('_', " "), result.result.error()));
            toast.set_timeout(6);
            self.toasts.add_toast(toast);
        } else if result.label == "invite" || result.label == "create_room" || result.label == "favorite_room" {
            let toast = adw::Toast::new(&format!("{} done", result.label.replace('_', " ")));
            toast.set_timeout(2);
            self.toasts.add_toast(toast);
        }
    }

    pub fn show_tab(&self, name: &str) {
        if TABS.iter().any(|(n, _, _)| *n == name) {
            self.tabs.set_visible_child_name(name);
        }
    }

    fn on_tab_changed(&self, name: &str) {
        self.prefs.borrow_mut().last_tab = name.to_string();
        let _ = self.prefs.borrow().save();
        match name {
            "people" => self.people.focus_search(),
            "history" => self.history.ensure_loaded(),
            _ => {}
        }
    }

    pub fn focus_search(&self) {
        self.show_tab("people");
        self.people.focus_search();
    }

    pub fn open_settings(&self) {
        settings::open(&self.window, &self.store, &self.prefs);
    }

    pub fn open_shortcuts(&self) {
        shortcuts::open(&self.window);
    }

    pub fn copy_personal_link(&self) {
        if let Some(room) = &self.store.state().personal_room {
            self.window.clipboard().set_text(&room.url);
            self.toasts.add_toast(adw::Toast::new("Personal room link copied"));
        }
    }

    pub fn sign_out(&self) {
        let store = self.store.clone();
        super::confirm(&self.window, "Sign out", "Sign out of Tuple on this machine?", "Sign out", move || store.logout());
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn toggle(&self) {
        if self.window.is_visible() && self.window.is_active() {
            self.window.set_visible(false);
        } else {
            self.window.present();
        }
    }
}

fn account_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let main = gio::Menu::new();
    main.append(Some("Settings"), Some("app.settings"));
    main.append(Some("Keyboard Shortcuts"), Some("app.shortcuts"));
    main.append(Some("Copy Personal Room Link"), Some("app.copy-link"));
    menu.append_section(None, &main);
    let session = gio::Menu::new();
    session.append(Some("Sign Out"), Some("app.sign-out"));
    session.append(Some("Quit"), Some("app.quit"));
    menu.append_section(None, &session);
    menu
}
