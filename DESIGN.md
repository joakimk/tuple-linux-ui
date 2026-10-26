# Tuple UI for Linux — design notes

Tuple ships a macOS app and a Windows app that share one design language. This
document records what that language is (from Tuple's docs and changelogs) and how
it maps onto a native Linux app built with GTK 4 + libadwaita.

## What the Mac and Windows apps have in common

**A compact popover, not a big window.** Both apps live in the menu bar (macOS)
or system tray (Windows) and open a narrow popover. Windows also has a
"Popout Window" mode that turns the popover into a normal window.

**Tabs along one edge.** People, Rooms, Links and Local History. The Directory
tab was removed in 2026. The search field is focused automatically when
switching tabs.

**People tab.** Search first. Collapsible sections: Favorites, Recent, Ongoing
Calls, then everyone else; collapsed sections stay collapsed. Rows show an
avatar, name, a presence dot (green online, "Busy" while on a call, nothing when
offline) and a Call button on hover. A yellow mark beside someone's avatar means
they are in a call and the button becomes Join. A missed call shows a banner
with "Call Back" at the top of the list. Sort order is favorites, then presence,
then name.

**Rooms tab.** A pinned "Your personal room" card joins the room and copies its
link. Below it the team rooms, alphabetical, favorites pinned to the top, each
showing who is inside. A "+" in the corner creates a room. A door control
(open/closed) sits with the call controls while you are in your personal room;
Windows mirrors the door state as a padlock on the tray icon.

**Links tab.** Copy call link, Join call and copy link, join by URL.

**Call controls.** One strip, ordered mic, webcam, screen share, Capture, then
add participant, reactions (with a Sounds section) and hang up. Hover labels on
every control. The hang-up button reads "Leave" unless it ends the call, then
"End Call". Inactive controls look clearly inactive; the active share state is
labelled "Sharing".

**Incoming calls.** A small always-on-top window with the caller's avatar and
name and Accept / Decline buttons, plus a system notification.

**Status icon.** Monochrome, follows light/dark (Windows), turns distinct during
a call, shows the door padlock. Left click opens the popover; a context menu
offers the main actions.

**Visual style.** System font (Segoe UI on Windows, SF on macOS), dark mode by
default with a light mode, "Tuple Purple" as the accent (screen-share borders,
primary buttons), subtle borders, rounded corners, toasts for transient events,
a Settings window with a sidebar and native forms (General, Audio, Capture,
Screen Share, Network, App Veil, Account, About).

## The Linux translation

Native on Linux means GNOME HIG + libadwaita widgets, system theme, accent
colour and dark/light preference from the desktop (Omarchy sets
`color-scheme=prefer-dark` and an accent colour). No custom chrome.

| Mac / Windows element | Linux (libadwaita) |
| --- | --- |
| Menu bar / tray popover | `adw::ApplicationWindow` about 420×640, hidden to the StatusNotifier tray on close; this is the "Popout Window" mode |
| Tabs | `adw::ViewStack` + `adw::ViewSwitcherBar` at the bottom (People, Rooms, Links, History) |
| Avatar menu bottom-left | `gtk::MenuButton` with an `adw::Avatar` in the header bar: Settings, Keyboard Shortcuts, Copy personal link, Sign out, Quit |
| Search field | `gtk::SearchEntry`, focused on tab switch and Ctrl+F |
| Sections | `gtk::ListBox` with the `boxed-list` style inside `adw::PreferencesGroup`-style headers; `gtk::Expander`-free, sections collapse via a header toggle |
| Contact row | `adw::ActionRow`: `adw::Avatar` prefix with a presence dot overlay, name as title, status as subtitle, star toggle + Call/Join `suggested-action` button as suffix |
| Missed-call banner | `adw::Banner` with "Call Back" |
| Personal room card | first `adw::ActionRow` in Rooms, with door state and "Join" |
| "+" create room | header-bar button, `adw::AlertDialog` with an entry |
| Call controls strip | `adw::ToolbarView` bottom bar: icon buttons with tooltips (mic, screen share, Capture, add, react, Leave as `destructive-action`) |
| Incoming call window | small `adw::Window` kept above with Accept (`suggested-action`) / Decline, plus a freedesktop notification with the same actions |
| Toasts | `adw::ToastOverlay` for command results and errors |
| Status icon | StatusNotifierItem via `ksni`: symbolic icon tinted by state (idle, in call, muted, sharing), door padlock overlay, left click toggles the window |
| Settings window | `adw::PreferencesDialog` with pages General (window, autostart, notifications), Tuple (overlay, start muted, capture backend, guest mode, Whisper model), About (`adw::AboutDialog`) |
| Keyboard shortcuts | `gtk::ShortcutsWindow`; Ctrl+1–4 tabs, Ctrl+F search, Ctrl+M mute, Ctrl+Shift+S share, Ctrl+Shift+C capture, Ctrl+Shift+L leave, Ctrl+, settings, Ctrl+? shortcuts, Esc hide, Ctrl+Q quit |
| Accent | the system accent colour; Tuple purple only as the fallback when the desktop has none |
| Global hotkeys | `tuple-ui --toggle-mute` / `--show` over a local socket, bound in the compositor |

Everything the app shows comes from `tuple state follow --raw`; every action is
a `tuple` CLI call. Capture (Local History) is not available on Linux yet, so
that tab shows a status page until the CLI reports a capture store.
