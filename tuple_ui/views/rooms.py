"""Rooms tab: pinned personal room card, then team rooms (favorites first)."""
from __future__ import annotations

from PyQt6.QtWidgets import (QApplication, QHBoxLayout, QInputDialog, QLabel, QMessageBox, QPushButton,
                             QVBoxLayout, QWidget)

from ..model import Room
from ..store import Store
from ..widgets.room_row import RoomRow
from ..widgets.section_list import SectionList


class RoomsView(QWidget):
    def __init__(self, store: Store, parent=None):
        super().__init__(parent)
        self.store = store
        self._rendered: tuple = ()
        layout = QVBoxLayout(self)
        layout.setContentsMargins(8, 8, 8, 0)
        layout.setSpacing(6)

        header = QHBoxLayout()
        title = QLabel("Rooms")
        title.setObjectName("title")
        header.addWidget(title, 1)
        self.create_button = QPushButton("＋")
        self.create_button.setProperty("kind", "flat")
        self.create_button.setToolTip("Create a team room")
        self.create_button.clicked.connect(self.create_room)
        header.addWidget(self.create_button)
        layout.addLayout(header)

        self.personal_holder = QVBoxLayout()
        self.personal_holder.setContentsMargins(4, 0, 4, 0)
        layout.addLayout(self.personal_holder)
        self.personal_card: RoomRow | None = None

        self.list = SectionList()
        layout.addWidget(self.list, 1)
        store.state_changed.connect(lambda _s: self.render())
        self.render(force=True)

    def current_room_slug(self) -> str | None:
        call = self.store.state.call
        return call.room.slug if call and call.room else None

    def render(self, force: bool = False):
        state = self.store.state
        key = (state.personal_room, state.team_rooms, self.current_room_slug())
        if not force and key == self._rendered:
            return
        self._rendered = key
        current = self.current_room_slug()

        if self.personal_card is not None:
            self.personal_holder.removeWidget(self.personal_card)
            self.personal_card.deleteLater()
            self.personal_card = None
        if state.personal_room:
            self.personal_card = self._row(state.personal_room, current)
            self.personal_holder.addWidget(self.personal_card)

        rows = [self._row(room, current) for room in state.sorted_team_rooms]
        self.list.set_sections([("Team rooms", rows)], empty_text="No team rooms yet. Create one with ＋.")

    def _row(self, room: Room, current: str | None) -> RoomRow:
        return RoomRow(room, on_join=self.join, on_favorite=self.favorite, on_leave=self.leave,
                       in_this_room=(room.slug == current))

    # ------------------------------------------------------------- actions
    def join(self, room: Room):
        switch = False
        if self.store.state.in_call and room.slug != self.current_room_slug():
            answer = QMessageBox.question(self, "Switch rooms?",
                                          f"You're already in a call. Leave it and join {room.display_name}?")
            if answer != QMessageBox.StandardButton.Yes:
                return
            switch = True
        if room.kind == "personal":
            QApplication.clipboard().setText(room.url)
        self.store.join_room(room.slug, switch=switch)

    def leave(self, _room: Room):
        self.store.leave()

    def favorite(self, room: Room, favorited: bool):
        self.store.favorite_room(room.slug, favorited)

    def create_room(self):
        name, ok = QInputDialog.getText(self, "Create room", "Room name:")
        if ok and name.strip():
            self.store.create_room(name.strip())
