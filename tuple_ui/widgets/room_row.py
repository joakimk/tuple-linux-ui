"""One room in the Rooms list: name, who is inside, Join and a context menu."""
from __future__ import annotations

from typing import Callable

from PyQt6.QtCore import Qt
from PyQt6.QtWidgets import QApplication, QFrame, QHBoxLayout, QLabel, QMenu, QPushButton, QVBoxLayout

from ..model import Room
from .avatar import Avatar


def occupancy_text(room: Room) -> str:
    names = [m.full_name for m in room.members]
    if not names:
        return "Empty"
    if len(names) <= 3:
        return ", ".join(names)
    return f"{', '.join(names[:2])} and {len(names) - 2} more"


class RoomRow(QFrame):
    def __init__(self, room: Room, on_join: Callable[[Room], None],
                 on_favorite: Callable[[Room, bool], None] | None = None,
                 on_leave: Callable[[Room], None] | None = None, in_this_room: bool = False, parent=None):
        super().__init__(parent)
        self.room = room
        self._on_join = on_join
        self._on_favorite = on_favorite
        self._on_leave = on_leave
        self.setObjectName("card")
        self.setToolTip(room.url)
        self.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.customContextMenuRequested.connect(self._show_menu)

        layout = QHBoxLayout(self)
        layout.setContentsMargins(10, 8, 10, 8)
        layout.setSpacing(10)

        text = QVBoxLayout()
        text.setSpacing(0)
        self.name_label = QLabel(room.display_name)
        self.star_label = QLabel("★" if room.favorited else "")
        self.star_label.setObjectName("warn")
        self.occupancy_label = QLabel(occupancy_text(room))
        self.occupancy_label.setObjectName("subtle" if not room.occupied else "")
        text.addWidget(self.name_label)
        text.addWidget(self.occupancy_label)
        layout.addLayout(text, 1)

        self.avatars = QHBoxLayout()
        self.avatars.setSpacing(-6)
        for member in room.members[:4]:
            self.avatars.addWidget(Avatar(member, size=24))
        layout.addLayout(self.avatars)
        layout.addWidget(self.star_label)

        if in_this_room and self._on_leave:
            self.join_button = QPushButton("Leave")
            self.join_button.setProperty("kind", "danger")
            self.join_button.clicked.connect(lambda: self._on_leave(self.room))
        else:
            self.join_button = QPushButton("Join")
            self.join_button.setProperty("kind", "primary" if room.occupied else "")
            self.join_button.clicked.connect(lambda: self._on_join(self.room))
        self.join_button.setCursor(Qt.CursorShape.PointingHandCursor)
        layout.addWidget(self.join_button)

    def _show_menu(self, pos):
        menu = QMenu(self)
        menu.addAction("Copy room link", lambda: QApplication.clipboard().setText(self.room.url))
        if self._on_favorite and self.room.kind == "team":
            label = "Unfavorite room" if self.room.favorited else "Favorite room"
            menu.addAction(label, lambda: self._on_favorite(self.room, not self.room.favorited))
        menu.exec(self.mapToGlobal(pos))
