//! Keyboard shortcuts: one table that the app binds and the help dialog lists.

use adw::prelude::*;

/// (action name, accelerator, human label). Tab actions carry their target.
pub const SHORTCUTS: [(&str, &str, &str); 13] = [
    ("app.tab::people", "<Ctrl>1", "People"),
    ("app.tab::rooms", "<Ctrl>2", "Rooms"),
    ("app.tab::links", "<Ctrl>3", "Links"),
    ("app.tab::history", "<Ctrl>4", "Local history"),
    ("app.search", "<Ctrl>f", "Search people"),
    ("app.mute", "<Ctrl>m", "Mute / unmute"),
    ("app.share", "<Ctrl><Shift>s", "Share / stop sharing screen"),
    ("app.capture", "<Ctrl><Shift>c", "Start / stop Capture"),
    ("app.leave", "<Ctrl><Shift>l", "Leave the call"),
    ("app.settings", "<Ctrl>comma", "Settings"),
    ("app.shortcuts", "<Ctrl>question", "Keyboard shortcuts"),
    ("app.hide", "Escape", "Hide window"),
    ("app.quit", "<Ctrl>q", "Quit Tuple UI"),
];

pub fn pretty(accel: &str) -> String {
    accel
        .replace("<Ctrl>", "Ctrl+")
        .replace("<Shift>", "Shift+")
        .replace("comma", ",")
        .replace("question", "?")
        .replace("Escape", "Esc")
        .split('+')
        .map(|part| if part.len() == 1 { part.to_uppercase() } else { part.to_string() })
        .collect::<Vec<_>>()
        .join("+")
}

pub fn open(parent: &impl IsA<gtk::Widget>) {
    let dialog = adw::Dialog::new();
    dialog.set_title("Keyboard Shortcuts");
    dialog.set_content_width(380);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    for (_action, accel, label) in SHORTCUTS {
        let row = adw::ActionRow::new();
        row.set_title(label);
        let keys = gtk::Label::new(Some(&pretty(accel)));
        keys.add_css_class("keycap");
        keys.set_valign(gtk::Align::Center);
        row.add_suffix(&keys);
        list.append(&row);
    }
    content.append(&list);
    let hint = gtk::Label::new(Some(
        "Global hotkeys: bind `tuple-ui --toggle-mute` or `tuple-ui --show` in your compositor (for example a Hyprland `bind`) to control Tuple from anywhere.",
    ));
    hint.set_wrap(true);
    hint.add_css_class("dim-label");
    hint.set_xalign(0.0);
    content.append(&hint);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_child(Some(&content));
    scroller.set_propagate_natural_height(true);
    scroller.set_max_content_height(560);
    toolbar.set_content(Some(&scroller));
    dialog.set_child(Some(&toolbar));
    dialog.present(Some(parent));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerators_render_as_readable_labels() {
        assert_eq!(pretty("<Ctrl>m"), "Ctrl+M");
        assert_eq!(pretty("<Ctrl><Shift>s"), "Ctrl+Shift+S");
        assert_eq!(pretty("<Ctrl>comma"), "Ctrl+,");
        assert_eq!(pretty("Escape"), "Esc");
    }
}
