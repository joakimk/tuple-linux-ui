"""In-call panel shown at the bottom of the window while a call is active."""
from __future__ import annotations

from datetime import datetime, timezone

from PyQt6.QtCore import Qt, QTimer
from PyQt6.QtWidgets import QFrame, QHBoxLayout, QLabel, QMenu, QPushButton, QVBoxLayout, QWidget

from ..model import AppState, Call, Participant
from ..store import Store
from ..widgets.avatar import Avatar

REACTIONS = [("📯 Airhorn", "airhorn"), ("🎺 Trombone", "trombone"), ("🪈 Flute", "flute")]


def call_title(state: AppState) -> str:
    call = state.call
    if call is None:
        return ""
    room = call.room
    if room is None:
        return "Call"
    if room.kind == "personal":
        if room.owner and state.user and room.owner.id != state.user.id:
            return f"{room.owner.short_name}'s room"
        return "Your personal room"
    return room.display_name


def participants_text(call: Call) -> str:
    names = [p.full_name for p in call.others]
    return ", ".join(names) if names else "Just you"


def participant_detail(p: Participant) -> str:
    bits = []
    if p.sharing:
        bits.append("Sharing screen")
    if p.muted:
        bits.append("Muted")
    if p.capturing:
        bits.append("Capturing")
    return " · ".join(bits)


def duration_text(call: Call) -> str:
    if call.started_at is None:
        return ""
    seconds = int((datetime.now(timezone.utc) - call.started_at).total_seconds())
    hours, rest = divmod(max(0, seconds), 3600)
    minutes, secs = divmod(rest, 60)
    return f"{hours}:{minutes:02d}:{secs:02d}" if hours else f"{minutes}:{secs:02d}"


class ParticipantRow(QWidget):
    def __init__(self, participant: Participant, on_remove, parent=None):
        super().__init__(parent)
        self.participant = participant
        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 2, 0, 2)
        layout.addWidget(Avatar(participant, size=24))
        self.name_label = QLabel(participant.full_name)
        layout.addWidget(self.name_label, 1)
        self.detail_label = QLabel(participant_detail(participant))
        self.detail_label.setObjectName("subtle")
        layout.addWidget(self.detail_label)
        self.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.customContextMenuRequested.connect(
            lambda pos: self._menu(pos, on_remove))

    def _menu(self, pos, on_remove):
        menu = QMenu(self)
        menu.addAction("Remove from call", lambda: on_remove(self.participant))
        menu.exec(self.mapToGlobal(pos))


class CallPanel(QFrame):
    def __init__(self, store: Store, parent=None):
        super().__init__(parent)
        self.store = store
        self.setObjectName("callPanel")
        self._rendered_call: Call | None = None
        self._rows: list[ParticipantRow] = []

        layout = QVBoxLayout(self)
        layout.setContentsMargins(12, 10, 12, 10)
        layout.setSpacing(6)

        header = QHBoxLayout()
        self.title_label = QLabel("")
        self.title_label.setObjectName("title")
        header.addWidget(self.title_label, 1)
        self.duration_label = QLabel("")
        self.duration_label.setObjectName("muted")
        header.addWidget(self.duration_label)
        layout.addLayout(header)

        self.participants_label = QLabel("")
        self.participants_label.setObjectName("muted")
        layout.addWidget(self.participants_label)
        self.participants_box = QVBoxLayout()
        self.participants_box.setSpacing(0)
        layout.addLayout(self.participants_box)

        controls = QHBoxLayout()
        controls.setSpacing(6)
        self.mute_button = self._control("Mute", self.store.toggle_mute, "Toggle microphone (Ctrl+M)")
        self.share_button = self._control("Share screen", self.store.toggle_share, "Share or stop sharing your screen (Ctrl+Shift+S)")
        self.capture_button = self._control("Capture", self.toggle_capture, "Start or stop Capture for this call (C)")
        self.add_button = self._control("Add…", self.show_add_menu, "Add another participant")
        self.react_button = self._control("React", self.show_react_menu, "Send a sound reaction")
        for button in (self.mute_button, self.share_button, self.capture_button, self.add_button, self.react_button):
            controls.addWidget(button)
        controls.addStretch(1)
        self.leave_button = QPushButton("Leave")
        self.leave_button.setProperty("kind", "danger")
        self.leave_button.setToolTip("Leave the call (Ctrl+Shift+L)")
        self.leave_button.clicked.connect(lambda: self.store.leave())
        controls.addWidget(self.leave_button)
        layout.addLayout(controls)

        self._timer = QTimer(self)
        self._timer.setInterval(1000)
        self._timer.timeout.connect(self._tick)
        store.state_changed.connect(lambda _s: self.render())
        self.hide()
        self.render()

    def _control(self, text: str, slot, tooltip: str) -> QPushButton:
        button = QPushButton(text)
        button.setProperty("kind", "control")
        button.setToolTip(tooltip)
        button.clicked.connect(lambda: slot())
        return button

    # -------------------------------------------------------------- render
    def render(self):
        state = self.store.state
        call = state.call
        if call is None:
            if not self.isHidden():
                self.hide()
                self._timer.stop()
            self._rendered_call = None
            return
        if call == self._rendered_call and not self.isHidden():
            return
        self._rendered_call = call
        self.title_label.setText(call_title(state))
        self.participants_label.setText(participants_text(call))
        self._set_active(self.mute_button, call.muted, "Unmute", "Mute")
        self._set_active(self.share_button, call.sharing, "Stop sharing", "Share screen")
        self._set_active(self.capture_button, call.transcribing, "Stop capture", "Capture")
        self._rebuild_participants(call)
        self._tick()
        if self.isHidden():
            self.show()
        if not self._timer.isActive():
            self._timer.start()

    def _set_active(self, button: QPushButton, active: bool, active_text: str, idle_text: str):
        button.setText(active_text if active else idle_text)
        button.setProperty("active", "true" if active else "false")
        button.style().unpolish(button)
        button.style().polish(button)

    def _rebuild_participants(self, call: Call):
        for row in self._rows:
            self.participants_box.removeWidget(row)
            row.deleteLater()
        self._rows = [ParticipantRow(p, self.remove_participant) for p in call.others]
        for row in self._rows:
            self.participants_box.addWidget(row)

    def participant_rows(self) -> list[ParticipantRow]:
        return list(self._rows)

    def _tick(self):
        call = self.store.state.call
        self.duration_label.setText(duration_text(call) if call else "")

    # ------------------------------------------------------------- actions
    def toggle_capture(self):
        call = self.store.state.call
        if call is None:
            return
        self.store.capture_stop() if call.transcribing else self.store.capture_start()

    def react(self, sound: str):
        self.store.react(sound)

    def show_react_menu(self):
        menu = QMenu(self)
        for label, sound in REACTIONS:
            menu.addAction(label, lambda s=sound: self.react(s))
        menu.exec(self.react_button.mapToGlobal(self.react_button.rect().bottomLeft()))

    def show_add_menu(self):
        menu = QMenu(self)
        in_call = {p.id for p in self.store.state.call.participants} if self.store.state.call else set()
        candidates = [c for c in self.store.state.sorted_contacts if c.id not in in_call]
        online = [c for c in candidates if c.status in ("online", "busy")]
        for contact in online or candidates[:15]:
            menu.addAction(f"{contact.full_name} ({contact.status})",
                           lambda c=contact: self.store.add_participant(c.email))
        if menu.isEmpty():
            menu.addAction("No one to add").setEnabled(False)
        menu.exec(self.add_button.mapToGlobal(self.add_button.rect().bottomLeft()))

    def remove_participant(self, participant: Participant):
        self.store.remove_participant(participant.email or participant.full_name)
