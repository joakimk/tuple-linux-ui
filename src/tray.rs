//! StatusNotifier tray icon (ksni): a state-coloured dot, tooltip and quick actions.

use async_channel::Sender;
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, ToolTip, Tray, TrayMethods};

use crate::model::{AppState, Contact, Room};
use crate::viewmodel::{tray_state, tray_tooltip, TrayState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCommand {
    ToggleWindow,
    ToggleMute,
    ToggleShare,
    Leave,
    CallPerson(String),
    JoinRoom(String),
    Settings,
    Quit,
}

/// The subset of app state the tray renders; updated through the ksni handle.
#[derive(Debug, Clone, Default)]
pub struct TraySnapshot {
    pub state: Option<TrayState>,
    pub tooltip: String,
    pub in_call: bool,
    pub muted: bool,
    pub sharing: bool,
    pub favorites: Vec<(String, String)>, // (label, email)
    pub rooms: Vec<(String, String)>,     // (label, slug)
}

impl TraySnapshot {
    pub fn from_state(state: &AppState) -> TraySnapshot {
        let favorites = state
            .sorted_contacts()
            .into_iter()
            .filter(|c: &&Contact| c.favorited && c.is_present())
            .take(8)
            .map(|c| (format!("Call {}", c.person.full_name), c.person.email.clone()))
            .collect();
        let rooms = state
            .all_rooms()
            .into_iter()
            .take(12)
            .map(|r: &Room| {
                let label = if r.occupied() { format!("{}  ({})", r.display_name(), r.members.len()) } else { r.display_name() };
                (label, r.slug.clone())
            })
            .collect();
        let call = state.call.as_ref();
        TraySnapshot {
            state: Some(tray_state(state)),
            tooltip: tray_tooltip(state),
            in_call: call.is_some(),
            muted: call.map(|c| c.muted).unwrap_or(false),
            sharing: call.map(|c| c.sharing).unwrap_or(false),
            favorites,
            rooms,
        }
    }
}

pub struct TupleTray {
    pub snapshot: TraySnapshot,
    commands: Sender<TrayCommand>,
}

impl TupleTray {
    fn send(&self, command: TrayCommand) {
        let _ = self.commands.send_blocking(command);
    }
}

/// Render the status dot as ARGB32 pixels for the given size.
pub fn render_icon(state: Option<TrayState>, sharing: bool, size: i32) -> Icon {
    let (r, g, b): (u8, u8, u8) = match state {
        None | Some(TrayState::Unavailable) | Some(TrayState::Connecting) | Some(TrayState::SignedOut) => (0x9a, 0x9f, 0xab),
        Some(TrayState::Idle) => (0x7c, 0x5c, 0xff),
        Some(TrayState::InCallMuted) => (0xe5, 0x48, 0x4d),
        Some(TrayState::InCallLive) => (0x3d, 0xd6, 0x8c),
    };
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let center = (size as f32 - 1.0) / 2.0;
    let radius = size as f32 * 0.42;
    let dot_center = (size as f32 * 0.78, size as f32 * 0.78);
    let dot_radius = size as f32 * 0.18;
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 - center, y as f32 - center);
            let inside = (dx * dx + dy * dy).sqrt() <= radius;
            let (ddx, ddy) = (x as f32 - dot_center.0, y as f32 - dot_center.1);
            let in_dot = sharing && (ddx * ddx + ddy * ddy).sqrt() <= dot_radius;
            let pixel = if in_dot { [0xff, 0xf5, 0xa5, 0x24] } else if inside { [0xff, r, g, b] } else { [0, 0, 0, 0] };
            data.extend_from_slice(&pixel);
        }
    }
    Icon { width: size, height: size, data }
}

impl Tray for TupleTray {
    /// The StatusNotifierItem id. Bars (e.g. the Omarchy shell) key their pin/hide
    /// lists on it, and "Tuple UI" is what the earlier PyQt app registered as.
    fn id(&self) -> String {
        "Tuple UI".into()
    }

    fn title(&self) -> String {
        "Tuple UI (unofficial)".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        [22, 32, 48].iter().map(|size| render_icon(self.snapshot.state, self.snapshot.sharing, *size)).collect()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip { title: self.snapshot.tooltip.clone(), ..Default::default() }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::ToggleWindow);
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::ToggleMute);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = vec![item("Show Tuple", |t| t.send(TrayCommand::ToggleWindow))];
        if self.snapshot.in_call {
            items.push(MenuItem::Separator);
            items.push(item(if self.snapshot.muted { "Unmute" } else { "Mute" }, |t| t.send(TrayCommand::ToggleMute)));
            items.push(item(if self.snapshot.sharing { "Stop sharing" } else { "Share screen" }, |t| t.send(TrayCommand::ToggleShare)));
            items.push(item("Leave call", |t| t.send(TrayCommand::Leave)));
        } else {
            if !self.snapshot.favorites.is_empty() {
                items.push(MenuItem::Separator);
                for (label, email) in &self.snapshot.favorites {
                    let email = email.clone();
                    items.push(MenuItem::Standard(StandardItem {
                        label: label.clone(),
                        activate: Box::new(move |t: &mut TupleTray| t.send(TrayCommand::CallPerson(email.clone()))),
                        ..Default::default()
                    }));
                }
            }
            if !self.snapshot.rooms.is_empty() {
                items.push(MenuItem::Separator);
                for (label, slug) in &self.snapshot.rooms {
                    let slug = slug.clone();
                    items.push(MenuItem::Standard(StandardItem {
                        label: label.clone(),
                        activate: Box::new(move |t: &mut TupleTray| t.send(TrayCommand::JoinRoom(slug.clone()))),
                        ..Default::default()
                    }));
                }
            }
        }
        items.push(MenuItem::Separator);
        items.push(item("Settings…", |t| t.send(TrayCommand::Settings)));
        items.push(item("Quit", |t| t.send(TrayCommand::Quit)));
        items
    }
}

fn item(label: &str, activate: fn(&mut TupleTray)) -> MenuItem<TupleTray> {
    MenuItem::Standard(StandardItem { label: label.into(), activate: Box::new(activate), ..Default::default() })
}

/// Spawn the tray on the tokio runtime. Commands arrive on the returned receiver.
pub async fn spawn(commands: Sender<TrayCommand>) -> Result<ksni::Handle<TupleTray>, ksni::Error> {
    TupleTray { snapshot: TraySnapshot::default(), commands }.spawn().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::raw_idle;

    #[test]
    fn icon_center_follows_state() {
        let center = |icon: &Icon| {
            let i = ((icon.height / 2) * icon.width + icon.width / 2) as usize * 4;
            icon.data[i..i + 4].to_vec()
        };
        assert_eq!(center(&render_icon(Some(TrayState::Idle), false, 22)), vec![0xff, 0x7c, 0x5c, 0xff]);
        assert_eq!(center(&render_icon(Some(TrayState::InCallMuted), false, 22)), vec![0xff, 0xe5, 0x48, 0x4d]);
        assert_eq!(center(&render_icon(Some(TrayState::InCallLive), true, 22)), vec![0xff, 0x3d, 0xd6, 0x8c]);
        assert_eq!(render_icon(None, false, 22).data[..4], [0, 0, 0, 0]); // transparent corner
    }

    #[test]
    fn snapshot_lists_online_favorites_and_rooms() {
        let state = AppState::from_raw(&raw_idle());
        let snapshot = TraySnapshot::from_state(&state);
        assert_eq!(snapshot.state, Some(TrayState::Idle));
        assert_eq!(snapshot.favorites.len(), 6);
        assert!(snapshot.favorites.iter().all(|(label, _)| label.starts_with("Call ")));
        assert_eq!(snapshot.rooms[0].0, "Your personal room");
        assert_eq!(snapshot.rooms.len(), 11);
    }
}
