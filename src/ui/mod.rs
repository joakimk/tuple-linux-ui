//! GTK 4 + libadwaita views. Every view is built once and refreshed from [`AppState`].

pub mod call_bar;
pub mod history;
pub mod incoming;
pub mod links;
pub mod people;
pub mod phases;
pub mod rooms;
pub mod settings;
pub mod shortcuts;
pub mod window;

use adw::prelude::*;

use crate::avatars::AvatarLoader;
use crate::model::Person;

/// Colours and small styles that libadwaita does not provide out of the box.
pub const CSS: &str = "
.presence-dot { min-width: 10px; min-height: 10px; border-radius: 6px;
                border: 2px solid @window_bg_color; margin: 0; }
.presence-online { background-color: @success_color; }
.presence-busy { background-color: @warning_color; }
.presence-offline { background-color: alpha(@window_fg_color, 0.3); }
.in-call-mark { background-color: @warning_color; min-width: 8px; min-height: 8px; border-radius: 4px; }
.section-title { font-weight: bold; font-size: 0.8em; opacity: 0.55; margin-top: 12px; margin-bottom: 4px; }
.call-bar { padding: 8px 12px; }
.call-active { background-color: alpha(@accent_bg_color, 0.25); }
.caption-dim { opacity: 0.6; }
";

pub fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}

/// An `adw::Avatar` with initials, a lazily loaded Gravatar and an optional presence dot.
pub fn avatar(person: &Person, size: i32, presence: Option<&str>, loader: &AvatarLoader) -> gtk::Widget {
    let avatar = adw::Avatar::new(size, Some(&person.full_name), true);
    loader.load(&person.avatar_url, {
        let avatar = avatar.clone();
        move |texture| avatar.set_custom_image(Some(texture))
    });
    let Some(status) = presence else { return avatar.upcast() };
    let class = match status {
        "online" => "presence-online",
        "busy" => "presence-busy",
        "offline" => "presence-offline",
        _ => return avatar.upcast(),
    };
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&avatar));
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("presence-dot");
    dot.add_css_class(class);
    dot.set_halign(gtk::Align::End);
    dot.set_valign(gtk::Align::End);
    dot.set_can_target(false);
    overlay.add_overlay(&dot);
    overlay.upcast()
}

/// A boxed list with a small section title above it.
pub fn section(title: &str) -> (gtk::Box, gtk::ListBox) {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if !title.is_empty() {
        let label = gtk::Label::new(Some(&title.to_uppercase()));
        label.add_css_class("section-title");
        label.set_xalign(0.0);
        label.set_margin_start(6);
        container.append(&label);
    }
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    container.append(&list);
    (container, list)
}

pub fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

pub fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(tooltip));
    button.set_valign(gtk::Align::Center);
    button.add_css_class("flat");
    button
}

pub fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_child(Some(child));
    scroller.set_vexpand(true);
    scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroller
}

pub fn padded_column() -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_margin_top(8);
    column.set_margin_bottom(12);
    column.set_margin_start(12);
    column.set_margin_end(12);
    column
}

/// Ask a yes/no question with an `adw::AlertDialog`.
pub fn confirm(parent: &impl IsA<gtk::Widget>, heading: &str, body: &str, verb: &str, on_yes: impl Fn() + 'static) {
    let dialog = adw::AlertDialog::new(Some(heading), Some(body));
    dialog.add_responses(&[("cancel", "Cancel"), ("yes", verb)]);
    dialog.set_response_appearance("yes", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        if response == "yes" {
            on_yes();
        }
    });
    dialog.present(Some(parent));
}

/// Ask for one line of text with an `adw::AlertDialog`.
pub fn prompt(parent: &impl IsA<gtk::Widget>, heading: &str, placeholder: &str, verb: &str, on_submit: impl Fn(String) + 'static) {
    let dialog = adw::AlertDialog::new(Some(heading), None);
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some(placeholder));
    entry.set_activates_default(true);
    dialog.set_extra_child(Some(&entry));
    dialog.add_responses(&[("cancel", "Cancel"), ("ok", verb)]);
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        let text = entry.text().trim().to_string();
        if response == "ok" && !text.is_empty() {
            on_submit(text);
        }
    });
    dialog.present(Some(parent));
}
