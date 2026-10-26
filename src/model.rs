//! Immutable app state built from `tuple state --raw` and `tuple state follow --raw` events.
//!
//! The raw daemon payload is the single source of truth. [`AppState::from_raw`]
//! normalizes it into small value types the views render directly, and
//! [`AppState::apply_event`] folds a follow-stream event (`state`,
//! `current_call`, `rooms`, `contacts`, ...) into a new state.

use chrono::{DateTime, TimeZone, Utc};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// `tuple` CLI missing or failing.
    Unavailable,
    /// Daemon starting, websocket connecting or sign-in pending.
    Connecting,
    SignedOut,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Person {
    pub id: i64,
    pub email: String,
    pub full_name: String,
    pub short_name: String,
    pub avatar_url: String,
}

impl Person {
    pub fn from_raw(raw: &Value) -> Person {
        let s = |key: &str| str_of(raw, key);
        let full = first_non_empty(&[s("full_name"), s("short_name"), s("email")]);
        Person {
            id: raw.get("id").and_then(Value::as_i64).unwrap_or(0),
            email: s("email"),
            full_name: full.clone(),
            short_name: first_non_empty(&[s("short_name"), full]),
            avatar_url: s("avatar_url"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    pub person: Person,
    pub kind: String,
    pub favorited: bool,
    pub recent: bool,
    pub status: String,
    pub is_agent: bool,
}

impl Contact {
    pub fn from_raw(raw: &Value) -> Contact {
        Contact {
            person: Person::from_raw(raw),
            kind: first_non_empty(&[str_of(raw, "kind"), "teammate".into()]),
            favorited: bool_of(raw, "favorited"),
            recent: bool_of(raw, "recent"),
            status: first_non_empty(&[str_of(raw, "status"), "unknown".into()]),
            is_agent: bool_of(raw, "is_agent"),
        }
    }

    pub fn presence_rank(&self) -> u8 {
        match self.status.as_str() {
            "online" => 0,
            "busy" => 1,
            "offline" => 2,
            _ => 3,
        }
    }

    pub fn is_present(&self) -> bool {
        matches!(self.status.as_str(), "online" | "busy")
    }

    fn sort_key(&self) -> (u8, u8, String) {
        (u8::from(!self.favorited), self.presence_rank(), self.person.full_name.to_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub url: String,
    pub kind: RoomKind,
    pub favorited: bool,
    pub members: Vec<Person>,
    pub owner: Option<Person>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomKind {
    Personal,
    Team,
}

impl Room {
    pub fn from_raw(raw: &Value, kind: RoomKind) -> Room {
        Room {
            id: str_of(raw, "id"),
            slug: str_of(raw, "slug"),
            name: str_of(raw, "name"),
            url: str_of(raw, "http_value"),
            kind,
            favorited: bool_of(raw, "favorited"),
            members: list_of(raw, "members").iter().map(Person::from_raw).collect(),
            owner: raw.get("user").or_else(|| raw.get("creator")).filter(|v| v.is_object()).map(Person::from_raw),
        }
    }

    pub fn occupied(&self) -> bool {
        !self.members.is_empty()
    }

    pub fn display_name(&self) -> String {
        if !self.name.is_empty() {
            self.name.clone()
        } else if self.kind == RoomKind::Personal {
            "Your personal room".into()
        } else {
            self.slug.clone()
        }
    }

    fn sort_key(&self) -> (u8, String) {
        (u8::from(!self.favorited), self.display_name().to_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub person: Person,
    pub muted: Option<bool>,
    pub sharing: bool,
    pub capturing: bool,
    pub is_current_user: bool,
}

impl Participant {
    pub fn from_raw(raw: &Value) -> Participant {
        let person_raw = raw.get("user").filter(|v| v.is_object()).unwrap_or(raw);
        Participant {
            person: Person::from_raw(person_raw),
            muted: raw.get("audio_enabled").and_then(Value::as_bool).map(|enabled| !enabled),
            sharing: is_sharing(raw.get("share_state")),
            capturing: bool_of(raw, "transcribing") || bool_of(raw, "capturing"),
            is_current_user: bool_of(raw, "is_current_user"),
        }
    }
}

fn is_sharing(share_state: Option<&Value>) -> bool {
    matches!(share_state.and_then(Value::as_str), Some(s) if !matches!(s, "" | "audio" | "none"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub id: String,
    pub connection_state: String,
    pub muted: bool,
    pub sharing: bool,
    pub transcribing: bool,
    pub webcam_enabled: bool,
    pub started_at: Option<DateTime<Utc>>,
    pub room: Option<Room>,
    pub participants: Vec<Participant>,
    pub capacity: i64,
}

impl Call {
    pub fn from_raw(raw: &Value) -> Call {
        let local = raw.get("local").cloned().unwrap_or(Value::Null);
        let room = raw.get("room").and_then(|r| r.get("url")).filter(|u| u.is_object()).map(|url| {
            let kind = if url.get("auto_join_behavior").is_some() && str_of(url, "name").is_empty() {
                RoomKind::Personal
            } else {
                RoomKind::Team
            };
            let mut room = Room::from_raw(url, kind);
            room.members = raw
                .get("room")
                .map(|r| list_of(r, "members"))
                .unwrap_or_default()
                .iter()
                .map(Person::from_raw)
                .collect();
            room
        });
        Call {
            id: str_of(raw, "id"),
            connection_state: first_non_empty(&[str_of(&local, "connection_state"), "unknown".into()]),
            muted: !local.get("audio_enabled").and_then(Value::as_bool).unwrap_or(true),
            sharing: is_sharing(local.get("share_state")),
            transcribing: bool_of(raw, "transcribing"),
            webcam_enabled: bool_of(&local, "webcam_enabled"),
            started_at: raw
                .get("started_at")
                .and_then(Value::as_i64)
                .and_then(|ms| Utc.timestamp_millis_opt(ms).single()),
            room,
            participants: list_of(raw, "participants").iter().map(Participant::from_raw).collect(),
            capacity: raw.get("capacity").and_then(Value::as_i64).unwrap_or(0),
        }
    }

    pub fn others(&self) -> impl Iterator<Item = &Participant> {
        self.participants.iter().filter(|p| !p.is_current_user)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppState {
    pub phase: PhaseValue,
    pub raw: Map<String, Value>,
    pub user: Option<Person>,
    pub team_name: String,
    pub contacts: Vec<Contact>,
    pub personal_room: Option<Room>,
    pub team_rooms: Vec<Room>,
    pub call: Option<Call>,
    pub missed_calls: Vec<Value>,
    pub invites: Vec<Value>,
    pub websocket_state: String,
    pub app_version: String,
    pub error: String,
}

/// `Phase` with a `Default` so `AppState::default()` starts out connecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseValue(pub Phase);

impl Default for PhaseValue {
    fn default() -> Self {
        PhaseValue(Phase::Connecting)
    }
}

impl AppState {
    pub fn from_raw(raw: &Value) -> AppState {
        let map = raw.as_object().cloned().unwrap_or_default();
        let user_raw = raw.get("current_user").filter(|v| v.is_object());
        let phase = match (str_of(raw, "sign_in").as_str(), user_raw) {
            ("signed_in", Some(_)) => Phase::Ready,
            ("signed_out", _) => Phase::SignedOut,
            _ => Phase::Connecting,
        };
        let rooms = raw.get("rooms").cloned().unwrap_or(Value::Null);
        AppState {
            phase: PhaseValue(phase),
            user: user_raw.map(Person::from_raw),
            team_name: user_raw.and_then(|u| u.get("team")).map(|t| str_of(t, "name")).unwrap_or_default(),
            contacts: list_of(raw, "contacts").iter().map(Contact::from_raw).collect(),
            personal_room: list_of(&rooms, "personal").first().map(|r| Room::from_raw(r, RoomKind::Personal)),
            team_rooms: list_of(&rooms, "team").iter().map(|r| Room::from_raw(r, RoomKind::Team)).collect(),
            call: raw.get("current_call").filter(|c| c.is_object()).map(Call::from_raw),
            missed_calls: list_of(raw, "missed_calls"),
            invites: list_of(raw, "invites"),
            websocket_state: str_of(raw, "websocket_state"),
            app_version: str_of(raw, "app_version"),
            error: String::new(),
            raw: map,
        }
    }

    pub fn unavailable(error: impl Into<String>) -> AppState {
        AppState { phase: PhaseValue(Phase::Unavailable), error: error.into(), ..AppState::default() }
    }

    pub fn connecting(error: impl Into<String>) -> AppState {
        AppState { phase: PhaseValue(Phase::Connecting), error: error.into(), ..AppState::default() }
    }

    /// Fold one `tuple state follow --raw` event into a new state.
    pub fn apply_event(&self, event_type: &str, data: Value) -> AppState {
        if event_type == "state" {
            return AppState::from_raw(&data);
        }
        let mut map = self.raw.clone();
        map.insert(event_type.to_string(), data);
        AppState::from_raw(&Value::Object(map))
    }

    pub fn phase(&self) -> Phase {
        self.phase.0
    }

    pub fn in_call(&self) -> bool {
        self.call.is_some()
    }

    pub fn sorted_contacts(&self) -> Vec<&Contact> {
        let mut contacts: Vec<&Contact> = self.contacts.iter().collect();
        contacts.sort_by_key(|c| c.sort_key());
        contacts
    }

    pub fn sorted_team_rooms(&self) -> Vec<&Room> {
        let mut rooms: Vec<&Room> = self.team_rooms.iter().collect();
        rooms.sort_by_key(|r| r.sort_key());
        rooms
    }

    pub fn all_rooms(&self) -> Vec<&Room> {
        self.personal_room.iter().chain(self.sorted_team_rooms()).collect()
    }

    /// Optimistic local update while `tuple rooms favorite` round-trips.
    pub fn with_room_favorited(&self, slug: &str, favorited: bool) -> AppState {
        let mut state = self.clone();
        for room in &mut state.team_rooms {
            if room.slug == slug {
                room.favorited = favorited;
            }
        }
        state
    }
}

fn str_of(raw: &Value, key: &str) -> String {
    raw.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn bool_of(raw: &Value, key: &str) -> bool {
    raw.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn list_of(raw: &Value, key: &str) -> Vec<Value> {
    raw.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn first_non_empty(candidates: &[String]) -> String {
    candidates.iter().find(|s| !s.is_empty()).cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{follow_events, raw_idle};

    #[test]
    fn idle_state_from_raw_snapshot() {
        let state = AppState::from_raw(&raw_idle());
        assert_eq!(state.phase(), Phase::Ready);
        assert_eq!(state.user.as_ref().unwrap().full_name, "Jo Example");
        assert_eq!(state.team_name, "Acme");
        assert!(state.call.is_none());
        assert_eq!(state.websocket_state, "connected");
        assert_eq!(state.contacts.len(), 24);
        assert_eq!(state.personal_room.as_ref().unwrap().slug, "r24ab2");
        assert_eq!(state.team_rooms.len(), 10);
    }

    #[test]
    fn contacts_sort_favorites_then_presence_then_name() {
        let state = AppState::from_raw(&raw_idle());
        let names: Vec<&str> = state.sorted_contacts().iter().map(|c| c.person.full_name.as_str()).collect();
        let mut online_favorites = names[..6].to_vec();
        online_favorites.sort();
        assert_eq!(names[..6], online_favorites[..]);
        assert_eq!(names[6], "Vint Cerf"); // favorited but offline comes after online favorites
        let rest: Vec<String> = names[7..].iter().map(|n| n.to_lowercase()).collect();
        let mut sorted = rest.clone();
        sorted.sort();
        assert_eq!(rest, sorted);
    }

    #[test]
    fn rooms_sort_favorites_first_then_alphabetically() {
        let state = AppState::from_raw(&raw_idle());
        let favorite = state.team_rooms[2].slug.clone();
        let state = state.with_room_favorited(&favorite, true);
        let names: Vec<String> = state.sorted_team_rooms().iter().map(|r| r.display_name()).collect();
        assert_eq!(state.sorted_team_rooms()[0].slug, favorite);
        let mut rest: Vec<String> = names[1..].iter().map(|n| n.to_lowercase()).collect();
        let unsorted = rest.clone();
        rest.sort();
        assert_eq!(unsorted, rest);
    }

    #[test]
    fn follow_stream_tracks_call_lifecycle_and_mute() {
        let events = follow_events();
        assert_eq!(events[0].0, "state");
        let mut state = AppState::from_raw(&events[0].1);
        let mut timeline = Vec::new();
        for (kind, data) in &events[1..] {
            state = state.apply_event(kind, data.clone());
            timeline.push(state.call.as_ref().map(|c| (c.connection_state.clone(), c.muted)));
        }
        let connected: Vec<_> = timeline.iter().flatten().filter(|(s, _)| s == "connected").collect();
        assert_eq!(connected[0], &("connected".to_string(), true)); // joined with audio disabled
        assert!(connected.contains(&&("connected".to_string(), false))); // unmuted later
        assert!(timeline[timeline.len() - 2].is_none()); // current_call: null after leaving
        assert!(state.call.is_none());
    }

    #[test]
    fn in_call_state_exposes_room_and_occupancy() {
        let events = follow_events();
        let mut state = AppState::from_raw(&events[0].1);
        for (kind, data) in &events[1..] {
            state = state.apply_event(kind, data.clone());
            if state.call.as_ref().is_some_and(|c| c.connection_state == "connected") {
                break;
            }
        }
        let call = state.call.as_ref().unwrap();
        let room = call.room.as_ref().unwrap();
        assert_eq!(room.slug, state.personal_room.as_ref().unwrap().slug);
        assert_eq!(room.kind, RoomKind::Personal);
        assert!(call.started_at.is_some());
        assert!(!call.sharing);
        let personal = state.personal_room.as_ref().unwrap();
        assert_eq!(personal.members.iter().map(|m| m.full_name.as_str()).collect::<Vec<_>>(), ["Jo Example"]);
        assert!(personal.occupied());
    }

    #[test]
    fn signed_out_and_unavailable_phases() {
        let mut raw = raw_idle();
        raw["sign_in"] = Value::from("signed_out");
        raw["current_user"] = Value::Null;
        assert_eq!(AppState::from_raw(&raw).phase(), Phase::SignedOut);

        let off = AppState::unavailable("daemon is not running");
        assert_eq!(off.phase(), Phase::Unavailable);
        assert!(off.contacts.is_empty());
        assert_eq!(off.error, "daemon is not running");
    }

    #[test]
    fn unknown_follow_event_types_are_kept_without_crashing() {
        let state = AppState::from_raw(&raw_idle());
        let updated = state.apply_event("whisper_model_manager", serde_json::json!({"has_model": true}));
        assert_eq!(updated.raw["whisper_model_manager"], serde_json::json!({"has_model": true}));
        assert_eq!(updated.contacts, state.contacts);
    }

    #[test]
    fn participant_details() {
        let p = Participant::from_raw(&serde_json::json!({
            "id": 2, "email": "ada@example.com", "full_name": "Ada Lovelace",
            "audio_enabled": false, "share_state": "screen"
        }));
        assert_eq!(p.muted, Some(true));
        assert!(p.sharing);
    }
}
