//! Full-window status pages for the non-ready phases: connecting, unavailable, signed out.

use std::rc::Rc;

use adw::prelude::*;

use crate::store::{ActionResult, Store};

pub fn connecting_page() -> adw::StatusPage {
    let spinner = adw::Spinner::new();
    spinner.set_size_request(32, 32);
    spinner.set_halign(gtk::Align::Center);
    adw::StatusPage::builder()
        .title("Connecting to Tuple…")
        .description("Starting the Tuple daemon and waiting for it to sign in.")
        .child(&spinner)
        .build()
}

pub struct UnavailablePage {
    pub root: adw::StatusPage,
}

impl UnavailablePage {
    pub fn new(store: Store) -> UnavailablePage {
        let button = gtk::Button::with_label("Start Tuple");
        button.add_css_class("suggested-action");
        button.add_css_class("pill");
        button.set_halign(gtk::Align::Center);
        button.connect_clicked(move |_| store.start_daemon());
        let root = adw::StatusPage::builder()
            .icon_name("network-offline-symbolic")
            .title("Tuple isn't available")
            .description("Tuple UI needs the `tuple` command-line app on your PATH. It starts the daemon on demand and retries automatically.")
            .child(&button)
            .build();
        UnavailablePage { root }
    }

    pub fn set_error(&self, error: &str) {
        let base = "Tuple UI needs the `tuple` command-line app on your PATH. It starts the daemon on demand and retries automatically.";
        let text = if error.is_empty() { base.to_string() } else { format!("{}\n\n{base}", glib::markup_escape_text(error)) };
        self.root.set_description(Some(&text));
    }
}

pub struct SignedOutPage {
    pub root: adw::StatusPage,
    output: gtk::Label,
}

impl SignedOutPage {
    pub fn new(store: Store) -> Rc<SignedOutPage> {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_halign(gtk::Align::Center);
        let sign_in = gtk::Button::with_label("Sign in with browser");
        sign_in.add_css_class("suggested-action");
        sign_in.add_css_class("pill");
        sign_in.set_halign(gtk::Align::Center);
        content.append(&sign_in);
        let output = gtk::Label::new(None);
        output.set_wrap(true);
        output.set_selectable(true);
        output.set_use_markup(true);
        output.add_css_class("dim-label");
        content.append(&output);
        let code_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let code = gtk::Entry::new();
        code.set_placeholder_text(Some("Sign-in code"));
        code.set_hexpand(true);
        let finish = gtk::Button::with_label("Finish sign-in");
        code_row.append(&code);
        code_row.append(&finish);
        content.append(&code_row);

        let root = adw::StatusPage::builder()
            .icon_name("avatar-default-symbolic")
            .title("Sign in to Tuple")
            .description("Sign in opens tuple.app in your browser. Paste the code it shows you below if the browser doesn't hand you back automatically.")
            .child(&content)
            .build();
        let page = Rc::new(SignedOutPage { root, output });
        sign_in.connect_clicked({
            let store = store.clone();
            move |_| store.login()
        });
        let submit = {
            let store = store.clone();
            let code = code.clone();
            move || {
                let text = code.text().trim().to_string();
                if !text.is_empty() {
                    store.auth(&text);
                    code.set_text("");
                }
            }
        };
        finish.connect_clicked({
            let submit = submit.clone();
            move |_| submit()
        });
        code.connect_activate(move |_| submit());
        page
    }

    /// Show the output of `tuple login` / `tuple auth` (it prints the URL to open).
    pub fn on_result(&self, result: &ActionResult) {
        if result.label != "login" && result.label != "auth" {
            return;
        }
        let text = format!("{}\n{}", result.result.stdout, result.result.stderr).trim().to_string();
        let text = if text.is_empty() {
            if result.result.ok() { "Done.".to_string() } else { result.result.error() }
        } else {
            text
        };
        let escaped = glib::markup_escape_text(&text);
        let linked = escaped
            .split_whitespace()
            .map(|word| if word.starts_with("http") { format!("<a href=\"{word}\">{word}</a>") } else { word.to_string() })
            .collect::<Vec<_>>()
            .join(" ");
        self.output.set_markup(&linked);
    }
}
