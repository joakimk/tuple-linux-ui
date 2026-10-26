"""Immutable app state built from `tuple state --raw` and `tuple state follow --raw` events.

The raw daemon payload is the single source of truth. `AppState.from_raw`
normalizes it into small frozen dataclasses the views can render directly,
and `apply_event` folds a follow-stream event (`state`, `current_call`,
`rooms`, `contacts`, ...) into a new state.
"""
from __future__ import annotations

from dataclasses import dataclass, field, replace
from datetime import datetime, timezone
from enum import Enum
from typing import Any

PRESENCE_RANK = {"online": 0, "busy": 1, "offline": 2, "unknown": 3}


class Phase(Enum):
    UNAVAILABLE = "unavailable"   # `tuple` CLI missing or failing
    CONNECTING = "connecting"     # daemon starting / websocket connecting / sign-in pending
    SIGNED_OUT = "signed_out"
    READY = "ready"


@dataclass(frozen=True)
class Person:
    id: int
    email: str
    full_name: str
    short_name: str
    avatar_url: str = ""

    @classmethod
    def from_raw(cls, raw: dict[str, Any]) -> "Person":
        return cls(
            id=int(raw.get("id") or 0),
            email=raw.get("email") or "",
            full_name=raw.get("full_name") or raw.get("short_name") or raw.get("email") or "",
            short_name=raw.get("short_name") or raw.get("full_name") or "",
            avatar_url=raw.get("avatar_url") or "",
        )

    @property
    def initials(self) -> str:
        parts = [p for p in self.full_name.split() if p]
        return "".join(p[0] for p in parts[:2]).upper() or "?"


@dataclass(frozen=True)
class Contact(Person):
    kind: str = "teammate"
    favorited: bool = False
    recent: bool = False
    status: str = "unknown"
    is_agent: bool = False

    @classmethod
    def from_raw(cls, raw: dict[str, Any]) -> "Contact":
        person = Person.from_raw(raw)
        return cls(
            **person.__dict__,
            kind=raw.get("kind") or "teammate",
            favorited=bool(raw.get("favorited")),
            recent=bool(raw.get("recent")),
            status=raw.get("status") or "unknown",
            is_agent=bool(raw.get("is_agent")),
        )

    @property
    def sort_key(self):
        return (0 if self.favorited else 1, PRESENCE_RANK.get(self.status, 3), self.full_name.lower())


@dataclass(frozen=True)
class Room:
    id: str
    slug: str
    name: str
    url: str
    kind: str  # "personal" | "team"
    favorited: bool = False
    members: tuple[Person, ...] = ()
    auto_join_behavior: str = ""
    created_at: str = ""
    owner: Person | None = None

    @classmethod
    def from_raw(cls, raw: dict[str, Any], kind: str) -> "Room":
        owner_raw = raw.get("user") or raw.get("creator")
        return cls(
            owner=Person.from_raw(owner_raw) if owner_raw else None,
            id=str(raw.get("id") or ""),
            slug=raw.get("slug") or "",
            name=raw.get("name") or "",
            url=raw.get("http_value") or "",
            kind=kind,
            favorited=bool(raw.get("favorited")),
            members=tuple(Person.from_raw(m) for m in raw.get("members") or []),
            auto_join_behavior=raw.get("auto_join_behavior") or "",
            created_at=raw.get("created_at") or "",
        )

    @property
    def occupied(self) -> bool:
        return bool(self.members)

    @property
    def display_name(self) -> str:
        return self.name or ("Your personal room" if self.kind == "personal" else self.slug)

    @property
    def sort_key(self):
        return (0 if self.favorited else 1, self.display_name.lower())


@dataclass(frozen=True)
class Participant(Person):
    muted: bool | None = None
    sharing: bool = False
    capturing: bool = False
    is_current_user: bool = False

    @classmethod
    def from_raw(cls, raw: dict[str, Any]) -> "Participant":
        person = Person.from_raw(raw.get("user") or raw)
        audio = raw.get("audio_enabled")
        return cls(
            **person.__dict__,
            muted=(not audio) if isinstance(audio, bool) else None,
            sharing=_is_sharing(raw.get("share_state")),
            capturing=bool(raw.get("transcribing") or raw.get("capturing")),
            is_current_user=bool(raw.get("is_current_user")),
        )


def _is_sharing(share_state: Any) -> bool:
    return isinstance(share_state, str) and share_state not in ("", "audio", "none")


@dataclass(frozen=True)
class Call:
    id: str
    connection_state: str
    muted: bool
    sharing: bool
    transcribing: bool
    started_at: datetime | None
    room: Room | None
    participants: tuple[Participant, ...]
    capacity: int = 0
    webcam_enabled: bool = False

    @classmethod
    def from_raw(cls, raw: dict[str, Any]) -> "Call":
        local = raw.get("local") or {}
        room_raw = (raw.get("room") or {}).get("url")
        room = None
        if room_raw:
            kind = "personal" if "auto_join_behavior" in room_raw and not room_raw.get("name") else "team"
            room = Room.from_raw(dict(room_raw, members=(raw.get("room") or {}).get("members") or []), kind)
        started = raw.get("started_at")
        return cls(
            id=str(raw.get("id") or ""),
            connection_state=local.get("connection_state") or "unknown",
            muted=not local.get("audio_enabled", True),
            sharing=_is_sharing(local.get("share_state")),
            transcribing=bool(raw.get("transcribing")),
            started_at=datetime.fromtimestamp(started / 1000, tz=timezone.utc) if started else None,
            room=room,
            participants=tuple(Participant.from_raw(p) for p in raw.get("participants") or []),
            capacity=int(raw.get("capacity") or 0),
            webcam_enabled=bool(local.get("webcam_enabled")),
        )

    @property
    def others(self) -> tuple[Participant, ...]:
        return tuple(p for p in self.participants if not p.is_current_user)


@dataclass(frozen=True)
class AppState:
    phase: Phase
    raw: dict[str, Any] = field(default_factory=dict)
    user: Person | None = None
    team_name: str = ""
    contacts: tuple[Contact, ...] = ()
    personal_room: Room | None = None
    team_rooms: tuple[Room, ...] = ()
    call: Call | None = None
    missed_calls: tuple[dict, ...] = ()
    invites: tuple[dict, ...] = ()
    websocket_state: str = ""
    internet: bool = True
    app_version: str = ""
    available_update: str | None = None
    error: str = ""

    # ------------------------------------------------------------ builders
    @classmethod
    def from_raw(cls, raw: dict[str, Any]) -> "AppState":
        user_raw = raw.get("current_user") or {}
        rooms = raw.get("rooms") or {}
        personal = [Room.from_raw(r, "personal") for r in rooms.get("personal") or []]
        sign_in = raw.get("sign_in") or ""
        if sign_in == "signed_in" and user_raw:
            phase = Phase.READY
        elif sign_in == "signed_out":
            phase = Phase.SIGNED_OUT
        else:
            phase = Phase.CONNECTING
        call_raw = raw.get("current_call")
        return cls(
            phase=phase,
            raw=raw,
            user=Person.from_raw(user_raw) if user_raw else None,
            team_name=((user_raw.get("team") or {}).get("name") or ""),
            contacts=tuple(Contact.from_raw(c) for c in raw.get("contacts") or []),
            personal_room=personal[0] if personal else None,
            team_rooms=tuple(Room.from_raw(r, "team") for r in rooms.get("team") or []),
            call=Call.from_raw(call_raw) if call_raw else None,
            missed_calls=tuple(raw.get("missed_calls") or []),
            invites=tuple(raw.get("invites") or []),
            websocket_state=raw.get("websocket_state") or "",
            internet=bool(raw.get("is_internet_connected", True)),
            app_version=raw.get("app_version") or "",
            available_update=raw.get("available_update_version"),
        )

    @classmethod
    def unavailable(cls, error: str) -> "AppState":
        return cls(phase=Phase.UNAVAILABLE, error=error)

    def apply_event(self, event_type: str, data: Any) -> "AppState":
        """Fold one `tuple state follow --raw` event into a new state."""
        if event_type == "state":
            return AppState.from_raw(data if isinstance(data, dict) else {})
        return AppState.from_raw(dict(self.raw, **{event_type: data}))

    # ------------------------------------------------------------- helpers
    @property
    def connected(self) -> bool:
        return self.websocket_state == "connected"

    @property
    def in_call(self) -> bool:
        return self.call is not None

    @property
    def sorted_contacts(self) -> tuple[Contact, ...]:
        return tuple(sorted(self.contacts, key=lambda c: c.sort_key))

    @property
    def sorted_team_rooms(self) -> tuple[Room, ...]:
        return tuple(sorted(self.team_rooms, key=lambda r: r.sort_key))

    @property
    def all_rooms(self) -> tuple[Room, ...]:
        return ((self.personal_room,) if self.personal_room else ()) + self.sorted_team_rooms

    def contact(self, contact_id: int) -> Contact | None:
        return next((c for c in self.contacts if c.id == contact_id), None)

    def room(self, slug: str) -> Room | None:
        return next((r for r in self.all_rooms if r.slug == slug), None)

    def with_room_favorited(self, slug: str, favorited: bool) -> "AppState":
        """Optimistic local update while `tuple rooms favorite` round-trips."""
        rooms = tuple(replace(r, favorited=favorited) if r.slug == slug else r for r in self.team_rooms)
        return replace(self, team_rooms=rooms)

    def with_contact_favorited(self, contact_id: int, favorited: bool) -> "AppState":
        contacts = tuple(replace(c, favorited=favorited) if c.id == contact_id else c for c in self.contacts)
        return replace(self, contacts=contacts)
