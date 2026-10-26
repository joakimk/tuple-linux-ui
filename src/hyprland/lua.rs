//! Lua snippets for `hyprctl eval` (Hyprland 0.56+ configures itself in Lua).

use super::{Placement, Window};

/// Escape a literal for Hyprland's regex window matching.
fn regex_escape(text: &str) -> String {
    text.chars().fold(String::new(), |mut out, c| {
        if r"\.^$|()[]{}*+?".contains(c) {
            out.push('\\');
        }
        out.push(c);
        out
    })
}

/// Lua that installs (replacing any earlier one) a window rule making the next
/// `class`/`title` window map floating at `placement` with `size`, so it never
/// shows up tiled first. Disable it with `popup_rule_off_lua` once mapped.
pub fn popup_rule_lua(class: &str, title: &str, placement: &Placement, size: (i32, i32)) -> String {
    format!(
        "if tuple_ui_popup_rule then tuple_ui_popup_rule:set_enabled(false) end\n\
         tuple_ui_popup_rule = hl.window_rule({{ name = \"tuple-ui-popup\", match = {{ class = [[^{}$]], title = [[^{}]] }}, \
         monitor = \"{}\", float = true, size = \"{} {}\", move = \"{} {}\" }})\nreturn \"ok\"",
        regex_escape(class),
        regex_escape(title),
        placement.monitor,
        size.0,
        size.1,
        placement.monitor_x,
        placement.monitor_y
    )
}

pub fn popup_rule_off_lua() -> &'static str {
    "if tuple_ui_popup_rule then tuple_ui_popup_rule:set_enabled(false) end\nreturn \"ok\""
}

/// Lua for `hyprctl eval` that focuses the window by address. Hyprland's window
/// dispatchers (float/resize/move) act on the focused window and ignore any
/// selector, so this must run (and be verified) before `placement_lua`.
pub fn focus_lua(address: &str) -> String {
    format!("return hl.dispatch(hl.dsp.focus({{ window = \"address:{address}\" }}))")
}

/// Lua statements, in order, that float (the dispatcher only toggles), size and
/// finally position the focused window. Resize keeps the centre, so move last.
pub fn placement_lua(window: &Window, size: (i32, i32), position: (i32, i32)) -> Vec<String> {
    let mut lua = Vec::new();
    if !window.floating {
        lua.push("return hl.dispatch(hl.dsp.window.float({ action = \"toggle\" }))".to_string());
    }
    lua.push(format!("return hl.dispatch(hl.dsp.window.resize({{ x = {}, y = {} }}))", size.0, size.1));
    lua.push(format!("return hl.dispatch(hl.dsp.window.move({{ x = {}, y = {} }}))", position.0, position.1));
    lua
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_focuses_floats_sizes_then_moves() {
        assert_eq!(focus_lua("0xabc"), "return hl.dispatch(hl.dsp.focus({ window = \"address:0xabc\" }))");
        let tiled = Window { address: "0xabc".into(), floating: false, at: (0, 0) };
        let lua = placement_lua(&tiled, (420, 640), (3177, 38));
        assert_eq!(
            lua,
            [
                "return hl.dispatch(hl.dsp.window.float({ action = \"toggle\" }))",
                "return hl.dispatch(hl.dsp.window.resize({ x = 420, y = 640 }))",
                "return hl.dispatch(hl.dsp.window.move({ x = 3177, y = 38 }))",
            ]
        );
        // Already floating: toggling would tile it, so no float step.
        let floating = Window { floating: true, ..tiled };
        assert_eq!(placement_lua(&floating, (420, 640), (3177, 38)).len(), 2);
        assert!(placement_lua(&floating, (420, 640), (3177, 38))[0].contains("resize"));
    }

    #[test]
    fn popup_rule_matches_the_window_and_uses_monitor_relative_coordinates() {
        let placement = Placement { monitor: "HDMI-A-1".into(), x: 3177, y: 38, monitor_x: 3177, monitor_y: 38 };
        let lua = popup_rule_lua("dev.joakimk.TupleUi", "Tuple UI (unofficial)", &placement, (420, 640));
        assert!(lua.contains(r"class = [[^dev\.joakimk\.TupleUi$]]"), "{lua}");
        assert!(lua.contains(r"title = [[^Tuple UI \(unofficial\)]]"), "{lua}");
        assert!(lua.contains(r#"monitor = "HDMI-A-1", float = true, size = "420 640", move = "3177 38""#), "{lua}");
        assert!(lua.starts_with("if tuple_ui_popup_rule then tuple_ui_popup_rule:set_enabled(false) end"));
        assert!(popup_rule_off_lua().contains("set_enabled(false)"));
    }
}
