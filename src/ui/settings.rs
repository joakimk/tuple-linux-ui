//! Settings: an `adw::PreferencesDialog` with Tuple UI preferences and the Tuple
//! daemon settings read from `tuple settings` and written with `tuple set`.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use crate::autostart;
use crate::prefs::Prefs;
use crate::store::Store;
use crate::tuple::CliResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    pub name: String,
    pub value: String,
    pub description: String,
    pub options: Option<Vec<String>>,
}

/// Parse the `name | value | description (a|b|c)` table printed by `tuple settings`.
pub fn parse_settings(text: &str) -> Vec<Setting> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '|');
            let name = parts.next()?.trim();
            let value = parts.next()?.trim();
            let description = parts.next()?.trim();
            if name.is_empty() || name.contains(' ') {
                return None;
            }
            let options = description
                .rsplit_once('(')
                .and_then(|(_, tail)| tail.strip_suffix(')'))
                .filter(|inner| inner.contains('|'))
                .map(|inner| inner.split('|').map(|o| o.trim().to_string()).collect());
            Some(Setting { name: name.into(), value: value.into(), description: description.into(), options })
        })
        .collect()
}

pub fn open(parent: &impl IsA<gtk::Widget>, store: &Store, prefs: &Rc<RefCell<Prefs>>) {
    let dialog = adw::PreferencesDialog::new();
    dialog.set_title("Settings");

    // ---- Tuple UI page ---------------------------------------------------
    let ui_page = adw::PreferencesPage::new();
    ui_page.set_title("General");
    ui_page.set_icon_name(Some("preferences-system-symbolic"));
    let window_group = adw::PreferencesGroup::new();
    window_group.set_title("Window");
    let show_on_start = switch_row("Show the window when Tuple UI starts", prefs.borrow().show_window_on_start);
    let start_in_tray = switch_row("Start hidden in the tray", prefs.borrow().start_in_tray);
    let launch = switch_row("Launch at login", autostart::is_enabled());
    window_group.add(&show_on_start);
    window_group.add(&start_in_tray);
    window_group.add(&launch);
    let notify_group = adw::PreferencesGroup::new();
    notify_group.set_title("Notifications");
    let notify_incoming = switch_row("Incoming calls", prefs.borrow().notify_incoming_calls);
    let notify_missed = switch_row("Missed calls", prefs.borrow().notify_missed_calls);
    notify_group.add(&notify_incoming);
    notify_group.add(&notify_missed);
    ui_page.add(&window_group);
    ui_page.add(&notify_group);
    dialog.add(&ui_page);

    {
        let prefs = prefs.clone();
        show_on_start.connect_active_notify(move |row| save(&prefs, |p| p.show_window_on_start = row.is_active()));
    }
    {
        let prefs = prefs.clone();
        start_in_tray.connect_active_notify(move |row| save(&prefs, |p| p.start_in_tray = row.is_active()));
    }
    {
        let prefs = prefs.clone();
        notify_incoming.connect_active_notify(move |row| save(&prefs, |p| p.notify_incoming_calls = row.is_active()));
    }
    {
        let prefs = prefs.clone();
        notify_missed.connect_active_notify(move |row| save(&prefs, |p| p.notify_missed_calls = row.is_active()));
    }
    launch.connect_active_notify(|row| {
        if let Err(err) = autostart::set_enabled(row.is_active()) {
            eprintln!("tuple-ui: could not update autostart entry: {err}");
        }
    });

    // ---- Tuple daemon page ---------------------------------------------
    let tuple_page = adw::PreferencesPage::new();
    tuple_page.set_title("Tuple");
    tuple_page.set_icon_name(Some("applications-system-symbolic"));
    let tuple_group = adw::PreferencesGroup::new();
    tuple_group.set_title("Daemon settings");
    tuple_group.set_description(Some("Loading from `tuple settings`…"));
    tuple_page.add(&tuple_group);
    dialog.add(&tuple_page);

    let cli = store.cli();
    let (sender, receiver) = async_channel::bounded::<CliResult>(1);
    store.runtime().spawn(async move {
        let _ = sender.send(cli.run(vec!["settings".into()]).await).await;
    });
    let store = store.clone();
    let state_version = store.state().app_version.clone();
    glib::spawn_future_local(async move {
        let Ok(result) = receiver.recv().await else { return };
        if !result.ok() {
            tuple_group.set_description(Some(&format!("Could not read Tuple settings: {}", result.error())));
            return;
        }
        let settings = parse_settings(&result.stdout);
        tuple_group.set_description(Some(&format!("Tuple {state_version}")));
        for setting in settings {
            tuple_group.add(&setting_row(&setting, &store));
        }
    });

    dialog.present(Some(parent));
}

fn save(prefs: &Rc<RefCell<Prefs>>, change: impl FnOnce(&mut Prefs)) {
    let mut prefs = prefs.borrow_mut();
    change(&mut prefs);
    if let Err(err) = prefs.save() {
        eprintln!("tuple-ui: could not save prefs: {err}");
    }
}

fn switch_row(title: &str, active: bool) -> adw::SwitchRow {
    let row = adw::SwitchRow::new();
    row.set_title(title);
    row.set_active(active);
    row
}

fn setting_row(setting: &Setting, store: &Store) -> gtk::Widget {
    let name = setting.name.clone();
    match &setting.options {
        Some(options) if options == &["0", "1"] => {
            let row = switch_row(&setting.description.replace("(0|1)", "").trim().to_string(), setting.value == "1");
            row.set_subtitle(&setting.name);
            let store = store.clone();
            row.connect_active_notify(move |row| store.set_setting(&name, if row.is_active() { "1" } else { "0" }));
            row.upcast()
        }
        Some(options) => {
            let row = adw::ComboRow::new();
            row.set_title(&setting.name);
            row.set_subtitle(&setting.description);
            let list: Vec<&str> = options.iter().map(String::as_str).collect();
            row.set_model(Some(&gtk::StringList::new(&list)));
            if let Some(index) = options.iter().position(|o| o == &setting.value) {
                row.set_selected(index as u32);
            }
            let store = store.clone();
            let options = options.clone();
            row.connect_selected_notify(move |row| {
                if let Some(value) = options.get(row.selected() as usize) {
                    store.set_setting(&name, value);
                }
            });
            row.upcast()
        }
        None => {
            let row = adw::EntryRow::new();
            row.set_title(&format!("{} — {}", setting.name, setting.description));
            row.set_text(&setting.value);
            row.set_show_apply_button(true);
            let store = store.clone();
            row.connect_apply(move |row| store.set_setting(&name, row.text().trim()));
            row.upcast()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_settings_table() {
        let text = "overlay | 1 | Show \"overlay\" annotation window when sharing screen (0|1)\n\
                    capture | auto | Screen capture mechanism (auto|x11|portal)\n\
                    transcription-model |  | File path to whisper ggml model\n";
        let parsed = parse_settings(text);
        assert_eq!(parsed.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["overlay", "capture", "transcription-model"]);
        assert_eq!(parsed[0].options.as_deref(), Some(&["0".to_string(), "1".to_string()][..]));
        assert_eq!(parsed[0].value, "1");
        assert_eq!(parsed[1].options.as_deref(), Some(&["auto".to_string(), "x11".into(), "portal".into()][..]));
        assert_eq!(parsed[2].options, None);
        assert_eq!(parsed[2].value, "");
    }
}
