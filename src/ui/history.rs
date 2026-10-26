//! Local History tab: captured calls from `tuple capture list`, searchable.
//!
//! Capture is not available on Linux yet, so this mostly shows a status page
//! until the CLI reports a capture store.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use serde_json::Value;

use super::{clear, icon_button, padded_column, scrolled, section};
use crate::store::Store;
use crate::tuple::CliResult;

pub struct HistoryView {
    pub root: gtk::Box,
    search: gtk::SearchEntry,
    list: gtk::Box,
    status: adw::StatusPage,
    store: Store,
    loaded: Cell<bool>,
}

pub fn entry_title(entry: &Value) -> String {
    if let Some(title) = entry.get("title").and_then(Value::as_str).filter(|t| !t.is_empty()) {
        return title.to_string();
    }
    match entry.get("started_at").and_then(Value::as_str) {
        Some(started) => format!("Call on {}", &started[..started.len().min(10)]),
        None => "Call".into(),
    }
}

pub fn entry_detail(entry: &Value) -> String {
    let mut bits = Vec::new();
    let names: Vec<String> = entry
        .get("participants")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|p| p.get("full_name").or_else(|| p.get("name")).and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    if !names.is_empty() {
        bits.push(names.join(", "));
    }
    if let Some(started) = entry.get("started_at").and_then(Value::as_str) {
        bits.push(started.replace('T', " ").chars().take(16).collect());
    }
    if let Some(summary) = entry.get("summary").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        bits.push(summary.chars().take(80).collect());
    }
    bits.join(" · ")
}

impl HistoryView {
    pub fn new(store: Store) -> Rc<HistoryView> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.set_margin_top(8);
        top.set_margin_start(12);
        top.set_margin_end(12);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search past calls"));
        search.set_hexpand(true);
        top.append(&search);
        let refresh = icon_button("view-refresh-symbolic", "Reload");
        top.append(&refresh);
        root.append(&top);
        let status = adw::StatusPage::builder()
            .icon_name("document-open-recent-symbolic")
            .title("Local History")
            .description("Captured calls will appear here.")
            .build();
        status.set_vexpand(true);
        root.append(&status);
        let list = padded_column();
        let scroller = scrolled(&list);
        scroller.set_visible(false);
        root.append(&scroller);

        let view = Rc::new(HistoryView { root, search, list, status, store, loaded: Cell::new(false) });
        refresh.connect_clicked({
            let view = view.clone();
            move |_| view.refresh()
        });
        view.search.connect_activate({
            let view = view.clone();
            move |_| view.refresh()
        });
        view
    }

    pub fn ensure_loaded(&self) {
        if !self.loaded.replace(true) {
            self.refresh();
        }
    }

    pub fn refresh(&self) {
        let query = self.search.text().trim().to_string();
        let mut args: Vec<String> = if query.is_empty() {
            vec!["capture".into(), "list".into()]
        } else {
            vec!["capture".into(), "search".into(), query]
        };
        args.extend(["--format".into(), "json".into()]);
        self.status.set_description(Some("Loading…"));
        let cli = self.store.cli();
        let (sender, receiver) = async_channel::bounded::<CliResult>(1);
        self.store.runtime().spawn(async move {
            let _ = sender.send(cli.run(args).await).await;
        });
        let list = self.list.clone();
        let status = self.status.clone();
        glib::spawn_future_local(async move {
            if let Ok(result) = receiver.recv().await {
                render(&list, &status, &result);
            }
        });
    }
}

fn render(list: &gtk::Box, status: &adw::StatusPage, result: &CliResult) {
    let scroller = list.parent().and_then(|p| p.parent()).and_then(|p| p.downcast::<gtk::ScrolledWindow>().ok());
    let entries = result.json().and_then(|v| v.as_array().cloned()).filter(|_| result.ok());
    let Some(entries) = entries else {
        let message = result.error();
        let friendly = if message.contains("unavailable") || message.contains("503") {
            "Capture isn't available on Linux yet. Captured calls will appear here once the CLI reports a capture store.".to_string()
        } else {
            message
        };
        status.set_description(Some(&friendly));
        status.set_visible(true);
        if let Some(scroller) = scroller {
            scroller.set_visible(false);
        }
        return;
    };
    clear(list);
    if entries.is_empty() {
        status.set_description(Some("No captured calls yet."));
        status.set_visible(true);
        if let Some(scroller) = scroller {
            scroller.set_visible(false);
        }
        return;
    }
    let (container, listbox) = section("Captured calls");
    for entry in entries.iter().filter(|e| e.is_object()) {
        let row = adw::ActionRow::new();
        row.set_title(&glib::markup_escape_text(&entry_title(entry)));
        row.set_subtitle(&glib::markup_escape_text(&entry_detail(entry)));
        let id = entry.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        let copy = icon_button("edit-copy-symbolic", "Copy call ID");
        copy.connect_clicked(move |button| button.clipboard().set_text(&id));
        row.add_suffix(&copy);
        listbox.append(&row);
    }
    list.append(&container);
    status.set_visible(false);
    if let Some(scroller) = scroller {
        scroller.set_visible(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_details_fall_back_gracefully() {
        let entry = serde_json::json!({"id": "x", "title": "Fixing the invoice bug", "started_at": "2026-10-20T09:00:00Z",
                                       "participants": [{"full_name": "Ada Lovelace"}]});
        assert_eq!(entry_title(&entry), "Fixing the invoice bug");
        assert_eq!(entry_detail(&entry), "Ada Lovelace · 2026-10-20 09:00");
        let bare = serde_json::json!({"id": "y", "title": null, "started_at": "2026-10-21T13:00:00Z"});
        assert_eq!(entry_title(&bare), "Call on 2026-10-21");
        assert_eq!(entry_title(&serde_json::json!({})), "Call");
    }
}
