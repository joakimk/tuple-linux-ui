//! Pure presentation helpers shared by the views: grouping, labels, tray state.
//! Everything here is testable without a display.

use chrono::{DateTime, Utc};

use crate::model::{AppState, Call, Contact, Participant, Phase, Room, RoomKind};

pub fn matches(contact: &Contact, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || contact.person.full_name.to_lowercase().contains(&query)
        || contact.person.email.to_lowercase().contains(&query)
}

/// People tab sections in display order: Favorites, Recent, Online, Everyone.
pub fn group_contacts<'a>(state: &'a AppState, query: &str) -> Vec<(&'static str, Vec<&'a Contact>)> {
    let contacts: Vec<&Contact> = state.sorted_contacts().into_iter().filter(|c| matches(c, query)).collect();
    let favorites: Vec<&Contact> = contacts.iter().copied().filter(|c| c.favorited).collect();
    let recent: Vec<&Contact> = contacts.iter().copied().filter(|c| !c.favorited && c.recent).collect();
    let online: Vec<&Contact> = contacts.iter().copied().filter(|c| !c.favorited && !c.recent && c.is_present()).collect();
    let everyone: Vec<&Contact> =
        contacts.iter().copied().filter(|c| !c.favorited && !c.recent && !c.is_present()).collect();
    [("Favorites", favorites), ("Recent", recent), ("Online", online), ("Everyone", everyone)]
        .into_iter()
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
}

pub fn status_text(contact: &Contact) -> String {
    let base = match contact.status.as_str() {
        "online" => "Online",
        "busy" => "Busy",
        "offline" => "Offline",
        _ => "",
    };
    let prefix = if contact.is_agent || contact.kind == "agent" {
        "Agent"
    } else if contact.kind == "external" {
        "External"
    } else {
        ""
    };
    match (prefix, base) {
        ("", base) => base.to_string(),
        (prefix, "") => prefix.to_string(),
        (prefix, base) => format!("{prefix} · {base}"),
    }
}

pub fn occupancy_text(room: &Room) -> String {
    let names: Vec<&str> = room.members.iter().map(|m| m.full_name.as_str()).collect();
    match names.len() {
        0 => "Empty".to_string(),
        1..=3 => names.join(", "),
        n => format!("{}, {} and {} more", names[0], names[1], n - 2),
    }
}

pub fn call_title(state: &AppState) -> String {
    let Some(call) = &state.call else { return String::new() };
    let Some(room) = &call.room else { return "Call".into() };
    if room.kind == RoomKind::Personal {
        if let (Some(owner), Some(user)) = (&room.owner, &state.user) {
            if owner.id != user.id {
                return format!("{}'s room", owner.short_name);
            }
        }
        return "Your personal room".into();
    }
    room.display_name()
}

pub fn participants_text(call: &Call) -> String {
    let names: Vec<&str> = call.others().map(|p| p.person.full_name.as_str()).collect();
    if names.is_empty() { "Just you".into() } else { names.join(", ") }
}

pub fn participant_detail(p: &Participant) -> String {
    let mut bits = Vec::new();
    if p.sharing {
        bits.push("Sharing screen");
    }
    if p.muted == Some(true) {
        bits.push("Muted");
    }
    if p.capturing {
        bits.push("Capturing");
    }
    bits.join(" · ")
}

pub fn duration_text(call: &Call, now: DateTime<Utc>) -> String {
    let Some(started) = call.started_at else { return String::new() };
    let seconds = (now - started).num_seconds().max(0);
    let (hours, rest) = (seconds / 3600, seconds % 3600);
    let (minutes, secs) = (rest / 60, rest % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{secs:02}") } else { format!("{minutes}:{secs:02}") }
}

/// Whether the hang-up button ends the call (last person) or just leaves it.
pub fn hangup_label(call: &Call) -> &'static str {
    if call.others().count() == 0 { "End Call" } else { "Leave" }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Unavailable,
    Connecting,
    SignedOut,
    Idle,
    InCallMuted,
    InCallLive,
}

pub fn tray_state(state: &AppState) -> TrayState {
    match (state.phase(), &state.call) {
        (Phase::Unavailable, _) => TrayState::Unavailable,
        (Phase::Connecting, _) => TrayState::Connecting,
        (Phase::SignedOut, _) => TrayState::SignedOut,
        (Phase::Ready, None) => TrayState::Idle,
        (Phase::Ready, Some(call)) if call.muted => TrayState::InCallMuted,
        (Phase::Ready, Some(_)) => TrayState::InCallLive,
    }
}

pub fn tray_tooltip(state: &AppState) -> String {
    match tray_state(state) {
        TrayState::Unavailable => "Tuple UI (unofficial) — Unavailable".into(),
        TrayState::Connecting => "Tuple UI (unofficial) — Connecting…".into(),
        TrayState::SignedOut => "Tuple UI (unofficial) — Signed out".into(),
        TrayState::Idle => {
            let online = state.contacts.iter().filter(|c| c.favorited && c.status == "online").count();
            format!("Tuple UI (unofficial) — Ready · {online} favorites online")
        }
        TrayState::InCallMuted | TrayState::InCallLive => {
            let call = state.call.as_ref().expect("in call");
            let mut bits = vec!["In call", if call.muted { "muted" } else { "mic on" }];
            if call.sharing {
                bits.push("sharing screen");
            }
            format!("Tuple UI (unofficial) — {}", bits.join(" · "))
        }
    }
}

/// Call id and caller name for an invite object from the daemon.
pub fn invite_call_id(invite: &serde_json::Value) -> String {
    invite
        .get("call_id")
        .or_else(|| invite.get("call").and_then(|c| c.get("id")))
        .or_else(|| invite.get("id"))
        .and_then(|v| v.as_str().map(String::from).or_else(|| v.as_i64().map(|n| n.to_string())))
        .unwrap_or_else(|| "current".into())
}

pub fn invite_person(invite: &serde_json::Value) -> crate::model::Person {
    for key in ["user", "from", "caller", "inviter", "person", "owner"] {
        if let Some(value) = invite.get(key).filter(|v| v.is_object()) {
            return crate::model::Person::from_raw(value);
        }
    }
    if invite.get("full_name").is_some() || invite.get("email").is_some() {
        return crate::model::Person::from_raw(invite);
    }
    crate::model::Person { full_name: "Someone".into(), short_name: "Someone".into(), ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{follow_events, raw_idle};

    fn in_call_state() -> AppState {
        let events = follow_events();
        let mut state = AppState::from_raw(&events[0].1);
        for (kind, data) in &events[1..] {
            state = state.apply_event(kind, data.clone());
            if state.call.as_ref().is_some_and(|c| c.connection_state == "connected") {
                break;
            }
        }
        state
    }

    #[test]
    fn people_sections_follow_the_popover_order_without_duplicates() {
        let state = AppState::from_raw(&raw_idle());
        let sections = group_contacts(&state, "");
        let titles: Vec<&str> = sections.iter().map(|(t, _)| *t).collect();
        assert_eq!(titles, ["Favorites", "Recent", "Everyone"]); // every online contact is a favorite
        let total: usize = sections.iter().map(|(_, rows)| rows.len()).sum();
        assert_eq!(total, 24);
        assert_eq!(sections[0].1.len(), 7);
        let names: Vec<&str> = sections[0].1.iter().map(|c| c.person.full_name.as_str()).collect();
        assert_eq!(names[6], "Vint Cerf"); // offline favorite last in Favorites
    }

    #[test]
    fn search_filters_by_name_or_email() {
        let state = AppState::from_raw(&raw_idle());
        let by_name = group_contacts(&state, "lovel");
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].1[0].person.full_name, "Ada Lovelace");
        let by_email = group_contacts(&state, "grace.hopper@");
        assert_eq!(by_email[0].1[0].person.full_name, "Grace Hopper");
    }

    #[test]
    fn labels_for_status_occupancy_and_call() {
        let state = AppState::from_raw(&raw_idle());
        let contact = state.sorted_contacts()[0];
        assert_eq!(status_text(contact), "Online");
        let room = &state.team_rooms[0];
        assert_eq!(occupancy_text(room), "Empty");

        let state = in_call_state();
        assert_eq!(call_title(&state), "Your personal room");
        let call = state.call.as_ref().unwrap();
        assert_eq!(participants_text(call), "Just you");
        assert_eq!(hangup_label(call), "End Call");
        let later = call.started_at.unwrap() + chrono::Duration::seconds(3725);
        assert_eq!(duration_text(call, later), "1:02:05");
        assert_eq!(occupancy_text(state.personal_room.as_ref().unwrap()), "Jo Example");
    }

    #[test]
    fn tray_state_and_tooltip_follow_the_call() {
        let idle = AppState::from_raw(&raw_idle());
        assert_eq!(tray_state(&idle), TrayState::Idle);
        assert_eq!(tray_tooltip(&idle), "Tuple UI (unofficial) — Ready · 6 favorites online");
        let in_call = in_call_state();
        assert_eq!(tray_state(&in_call), TrayState::InCallMuted);
        assert_eq!(tray_tooltip(&in_call), "Tuple UI (unofficial) — In call · muted");
        assert_eq!(tray_state(&AppState::unavailable("x")), TrayState::Unavailable);
        assert_eq!(tray_tooltip(&AppState::connecting("")), "Tuple UI (unofficial) — Connecting…");
    }

    #[test]
    fn invites_are_read_defensively() {
        let invite = serde_json::json!({"call_id": "abc-123", "user": {"id": 9, "email": "ada@example.com",
                                        "full_name": "Ada Lovelace", "short_name": "Ada"}});
        assert_eq!(invite_call_id(&invite), "abc-123");
        assert_eq!(invite_person(&invite).full_name, "Ada Lovelace");
        assert_eq!(invite_call_id(&serde_json::json!({})), "current");
        assert_eq!(invite_person(&serde_json::json!({})).full_name, "Someone");
    }
}
