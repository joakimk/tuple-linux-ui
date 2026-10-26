"""AppState is built from `tuple state --raw` and updated by `tuple state follow --raw` events."""
from tuple_ui.model import AppState, Phase


def test_idle_state_from_raw_snapshot(raw_idle):
    state = AppState.from_raw(raw_idle)

    assert state.phase is Phase.READY
    assert state.user.full_name == "Jo Example"
    assert state.team_name == "Acme"
    assert state.call is None
    assert state.connected
    assert len(state.contacts) == 24
    assert state.personal_room.slug == "r24ab2"
    assert len(state.team_rooms) == 10


def test_contacts_sort_favorites_then_presence_then_name(raw_idle):
    state = AppState.from_raw(raw_idle)
    names = [c.full_name for c in state.sorted_contacts]

    favorites_online = [c for c in state.sorted_contacts if c.favorited and c.status == "online"]
    assert names[: len(favorites_online)] == sorted(c.full_name for c in favorites_online)
    assert names[len(favorites_online)] == "Vint Cerf"  # favorited but offline comes after online favorites
    rest = names[len(favorites_online) + 1 :]
    assert rest == sorted(rest, key=str.lower)


def test_rooms_sort_favorites_first_then_alphabetically(raw_idle):
    state = AppState.from_raw(raw_idle)
    favorited = state.team_rooms[2]
    state = state.with_room_favorited(favorited.slug, True)

    names = [r.name for r in state.sorted_team_rooms]
    assert names[0] == favorited.name
    assert names[1:] == sorted(names[1:], key=str.lower)


def test_follow_stream_tracks_call_lifecycle_and_mute(follow_events):
    first = follow_events[0]
    assert first["type"] == "state"
    state = AppState.from_raw(first["data"])
    assert state.call is None

    timeline = []
    for event in follow_events[1:]:
        state = state.apply_event(event["type"], event["data"])
        call = state.call
        timeline.append((call.connection_state, call.muted) if call else None)

    connected = [t for t in timeline if t and t[0] == "connected"]
    assert connected[0] == ("connected", True)   # joined muted (start-calls-muted off, audio_enabled false while signaling)
    assert ("connected", False) in connected      # unmuted
    assert timeline[-2] is None                   # current_call: null after leaving
    assert state.call is None


def test_in_call_state_exposes_room_and_occupancy(follow_events):
    state = AppState.from_raw(follow_events[0]["data"])
    for event in follow_events[1:]:
        state = state.apply_event(event["type"], event["data"])
        if state.call and state.call.connection_state == "connected":
            break

    assert state.phase is Phase.READY
    assert state.call.room.slug == state.personal_room.slug
    assert state.call.room.kind == "personal"
    assert state.call.started_at is not None
    assert state.call.sharing is False
    assert [m.full_name for m in state.personal_room.members] == ["Jo Example"]
    assert state.personal_room.occupied


def test_signed_out_and_unavailable_phases(raw_idle):
    signed_out = AppState.from_raw(dict(raw_idle, sign_in="signed_out", current_user=None))
    assert signed_out.phase is Phase.SIGNED_OUT

    off = AppState.unavailable("daemon is not running")
    assert off.phase is Phase.UNAVAILABLE
    assert off.contacts == ()
    assert off.error == "daemon is not running"


def test_unknown_follow_event_types_are_kept_without_crashing(raw_idle):
    state = AppState.from_raw(raw_idle)
    updated = state.apply_event("whisper_model_manager", {"has_model": True})
    assert updated.raw["whisper_model_manager"] == {"has_model": True}
    assert updated.contacts == state.contacts
