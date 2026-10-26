//! Hyprland integration: place the main window under the tray icon.
//!
//! Wayland clients cannot position their own windows, so when the window is
//! opened from the tray we ask `hyprctl` where the pointer is, float the window
//! and move it just below the bar, centred on the click. Everything here is a
//! no-op when not running under Hyprland.

use serde_json::Value;
use std::time::Duration;
use tokio::process::Command;

mod lua;
use lua::{focus_lua, placement_lua, popup_rule_lua, popup_rule_off_lua};

/// Space between the window and the bar / monitor edges.
pub const GAP: i32 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Reserved space (bars) as [left, top, right, bottom].
    pub reserved: [i32; 4],
}

impl Monitor {
    fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

pub fn available() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
}

/// `hyprctl cursorpos -j`
pub fn parse_cursor(json: &str) -> Option<(i32, i32)> {
    let value: Value = serde_json::from_str(json).ok()?;
    Some((int(&value, "x")?, int(&value, "y")?))
}

/// `hyprctl monitors -j`
pub fn parse_monitors(json: &str) -> Vec<Monitor> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json) else { return Vec::new() };
    items
        .iter()
        .filter_map(|m| {
            let reserved: Vec<i32> = m.get("reserved")?.as_array()?.iter().filter_map(Value::as_i64).map(|n| n as i32).collect();
            Some(Monitor {
                name: m.get("name")?.as_str()?.to_string(),
                x: int(m, "x")?,
                y: int(m, "y")?,
                width: int(m, "width")?,
                height: int(m, "height")?,
                reserved: reserved.try_into().ok()?,
            })
        })
        .collect()
}

fn int(value: &Value, key: &str) -> Option<i32> {
    value.get(key).and_then(Value::as_f64).map(|n| n.round() as i32)
}

/// Where the popup goes: global top-left corner, plus the monitor and the
/// corner relative to it (window rules take monitor-relative coordinates).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub monitor: String,
    pub x: i32,
    pub y: i32,
    pub monitor_x: i32,
    pub monitor_y: i32,
}

/// Top-left corner for a `size` window under the bar of the monitor holding
/// `cursor`, centred on the cursor and kept inside the monitor's usable area.
pub fn popup_position(cursor: (i32, i32), monitors: &[Monitor], size: (i32, i32)) -> Option<Placement> {
    let monitor = monitors.iter().find(|m| m.contains(cursor))?;
    let [left, top, right, _] = monitor.reserved;
    let min_x = monitor.x + left + GAP;
    let max_x = (monitor.x + monitor.width - right - size.0 - GAP).max(min_x);
    let x = (cursor.0 - size.0 / 2).clamp(min_x, max_x);
    let y = monitor.y + top + GAP;
    Some(Placement { monitor: monitor.name.clone(), x, y, monitor_x: x - monitor.x, monitor_y: y - monitor.y })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub address: String,
    pub floating: bool,
    pub at: (i32, i32),
}

/// The mapped window with this class whose title starts with `title` (`hyprctl clients -j`).
pub fn find_window(clients_json: &str, class: &str, title: &str) -> Option<Window> {
    let Ok(Value::Array(clients)) = serde_json::from_str::<Value>(clients_json) else { return None };
    clients
        .iter()
        .find(|c| {
            c.get("mapped").and_then(Value::as_bool) == Some(true)
                && c.get("class").and_then(Value::as_str) == Some(class)
                && c.get("title").and_then(Value::as_str).is_some_and(|t| t.starts_with(title))
        })
        .and_then(|c| {
            let at = c.get("at")?.as_array()?;
            Some(Window {
                address: c.get("address")?.as_str()?.to_string(),
                floating: c.get("floating").and_then(Value::as_bool).unwrap_or(false),
                at: (at.first()?.as_i64()? as i32, at.get(1)?.as_i64()? as i32),
            })
        })
}

async fn hyprctl(args: &[&str]) -> Option<String> {
    let output = Command::new("hyprctl").args(args).output().await.ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run one Lua chunk. Hyprland reports Lua failures as text, so look at the reply too.
async fn eval(lua: &str) -> Result<(), String> {
    let output = Command::new("hyprctl").args(["eval", lua]).output().await.map_err(|e| format!("hyprctl: {e}"))?;
    let reply = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    if !output.status.success() || reply.contains("error") || reply.contains("warning") {
        let summary: String = reply.trim().chars().take(200).collect();
        return Err(format!("hyprctl eval ({}): {summary}", output.status));
    }
    Ok(())
}

async fn active_window_address() -> Option<String> {
    let json = hyprctl(&["activewindow", "-j"]).await?;
    serde_json::from_str::<Value>(&json).ok()?.get("address")?.as_str().map(String::from)
}

/// Decide where a `size` window goes to sit under the pointer right now and
/// install the window rule that makes it map there. Call this before showing
/// the window: Hyprland warps the pointer to a newly focused window, so reading
/// it afterwards gives the window's centre, not the icon.
pub async fn prepare_popup(class: &str, title: &str, size: (i32, i32)) -> Result<Placement, String> {
    let cursor = parse_cursor(&hyprctl(&["cursorpos", "-j"]).await.ok_or("hyprctl cursorpos failed")?).ok_or("no cursor position")?;
    let monitors = parse_monitors(&hyprctl(&["monitors", "-j"]).await.ok_or("hyprctl monitors failed")?);
    let placement = popup_position(cursor, &monitors, size).ok_or("pointer is on no monitor")?;
    eval(&popup_rule_lua(class, title, &placement, size)).await?;
    Ok(placement)
}

/// After the window is shown: wait for it to map, drop the one-shot rule, and
/// if it still did not land at `placement` (e.g. the rule could not pick the
/// monitor) float, size and move it explicitly.
pub async fn finish_popup(class: &str, title: &str, size: (i32, i32), placement: &Placement) -> Result<(), String> {
    let window = wait_for_window(class, title).await;
    let _ = eval(popup_rule_off_lua()).await;
    let window = window.ok_or("window did not appear")?;
    if window.floating && window.at == (placement.x, placement.y) {
        return Ok(());
    }
    place_window(&window, size, (placement.x, placement.y)).await
}

async fn wait_for_window(class: &str, title: &str) -> Option<Window> {
    for _ in 0..40 {
        if let Some(window) = hyprctl(&["clients", "-j"]).await.and_then(|json| find_window(&json, class, title)) {
            return Some(window);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    None
}

/// Float `window` if needed, size it and move its top-left corner to `(x, y)`.
async fn place_window(window: &Window, size: (i32, i32), (x, y): (i32, i32)) -> Result<(), String> {
    eval(&focus_lua(&window.address)).await?;
    // Never size or move whatever else happens to be focused.
    if active_window_address().await.as_deref() != Some(window.address.as_str()) {
        return Err("could not focus the window".into());
    }
    for lua in placement_lua(window, size, (x, y)) {
        eval(&lua).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITORS: &str = r#"[
      {"id": 0, "name": "eDP-1", "x": 3840, "y": 0, "width": 1920, "height": 1080, "scale": 1.0, "focused": true, "reserved": [0, 30, 0, 0]},
      {"id": 1, "name": "HDMI-A-1", "x": 0, "y": 0, "width": 3840, "height": 2160, "scale": 1.0, "focused": false, "reserved": [0, 30, 0, 0]}
    ]"#;

    const CLIENTS: &str = r#"[
      {"address": "0x55bd85aba1e0", "mapped": true, "hidden": false, "at": [1710, 775], "size": [420, 640], "class": "dev.joakimk.TupleUi", "title": "Tuple UI (unofficial) — In call", "floating": true},
      {"address": "0x55bd85ffff00", "mapped": true, "hidden": false, "at": [0, 0], "size": [300, 200], "class": "dev.joakimk.TupleUi", "title": "Incoming call", "floating": true},
      {"address": "0x55bd85eeee00", "mapped": false, "hidden": false, "at": [0, 0], "size": [420, 640], "class": "dev.joakimk.TupleUi", "title": "Tuple UI (unofficial)", "floating": false},
      {"address": "0x55bd85dddd00", "mapped": true, "hidden": false, "at": [1, 31], "size": [1930, 1294], "class": "firefox", "title": "Tuple UI (unofficial) — Mozilla Firefox", "floating": false}
    ]"#;

    #[test]
    fn parses_cursor_and_monitors() {
        assert_eq!(parse_cursor("{\n    \"x\": 4247,\n    \"y\": 965\n}\n"), Some((4247, 965)));
        assert_eq!(parse_cursor("garbage"), None);
        let monitors = parse_monitors(MONITORS);
        assert_eq!(monitors.len(), 2);
        assert_eq!(monitors[1], Monitor { name: "HDMI-A-1".into(), x: 0, y: 0, width: 3840, height: 2160, reserved: [0, 30, 0, 0] });
    }

    #[test]
    fn window_opens_under_the_bar_centred_on_the_click() {
        let monitors = parse_monitors(MONITORS);
        let corner = |p: Option<Placement>| p.map(|p| (p.x, p.y));
        // Tray icon on the external monitor's bar.
        let external = popup_position((3387, 15), &monitors, (420, 640)).unwrap();
        assert_eq!((external.x, external.y), (3387 - 210, 30 + GAP));
        assert_eq!((external.monitor.as_str(), external.monitor_x, external.monitor_y), ("HDMI-A-1", 3177, 38));
        // Near the right edge of the laptop screen: pushed left to stay on screen.
        let laptop = popup_position((5700, 15), &monitors, (420, 640)).unwrap();
        assert_eq!((laptop.x, laptop.y), (3840 + 1920 - 420 - GAP, 30 + GAP));
        assert_eq!((laptop.monitor.as_str(), laptop.monitor_x, laptop.monitor_y), ("eDP-1", 1920 - 420 - GAP, 38));
        // Near the left edge.
        assert_eq!(corner(popup_position((3850, 15), &monitors, (420, 640))), Some((3840 + GAP, 30 + GAP)));
        // Pointer on no monitor.
        assert_eq!(corner(popup_position((9000, 15), &monitors, (420, 640))), None);
    }

    #[test]
    fn finds_the_mapped_main_window_only() {
        let window = find_window(CLIENTS, "dev.joakimk.TupleUi", "Tuple UI (unofficial)").unwrap();
        assert_eq!(window, Window { address: "0x55bd85aba1e0".into(), floating: true, at: (1710, 775) });
        assert_eq!(find_window(CLIENTS, "dev.joakimk.TupleUi", "Settings"), None);
        assert_eq!(find_window("[]", "dev.joakimk.TupleUi", "Tuple UI (unofficial)"), None);
    }

}
