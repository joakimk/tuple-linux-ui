"""One contact in the People list: avatar, name, presence and a Call button."""
from __future__ import annotations

from typing import Callable

from PyQt6.QtCore import Qt
from PyQt6.QtGui import QAction
from PyQt6.QtWidgets import QFrame, QHBoxLayout, QLabel, QMenu, QPushButton, QVBoxLayout

from ..model import Contact
from .avatar import Avatar

STATUS_TEXT = {"online": "Online", "busy": "Busy", "offline": "Offline"}


class ContactRow(QFrame):
    def __init__(self, contact: Contact, on_call: Callable[[Contact], None],
                 on_favorite: Callable[[Contact, bool], None] | None = None,
                 on_remove: Callable[[Contact], None] | None = None,
                 on_add_to_call: Callable[[Contact], None] | None = None, parent=None):
        super().__init__(parent)
        self.contact = contact
        self._on_call = on_call
        self._on_favorite = on_favorite
        self._on_remove = on_remove
        self._on_add_to_call = on_add_to_call
        self.setObjectName("card")
        self.setToolTip(contact.email)
        self.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.customContextMenuRequested.connect(self._show_menu)

        layout = QHBoxLayout(self)
        layout.setContentsMargins(10, 6, 10, 6)
        layout.setSpacing(10)
        self.avatar = Avatar(contact, size=36, presence=contact.status)
        layout.addWidget(self.avatar)

        text = QVBoxLayout()
        text.setSpacing(0)
        self.name_label = QLabel(contact.full_name)
        self.status_label = QLabel(self._status_text())
        self.status_label.setObjectName("subtle")
        text.addWidget(self.name_label)
        text.addWidget(self.status_label)
        layout.addLayout(text, 1)

        self.call_button = QPushButton("Call")
        self.call_button.setProperty("kind", "primary" if contact.status in ("online", "busy") else "")
        self.call_button.setCursor(Qt.CursorShape.PointingHandCursor)
        self.call_button.setToolTip(f"Call {contact.short_name}")
        self.call_button.clicked.connect(lambda: self._on_call(self.contact))
        layout.addWidget(self.call_button)

    def _status_text(self) -> str:
        base = STATUS_TEXT.get(self.contact.status, "")
        if self.contact.is_agent or self.contact.kind == "agent":
            return f"Agent · {base}" if base else "Agent"
        if self.contact.kind == "external":
            return f"External · {base}" if base else "External"
        return base

    def _show_menu(self, pos):
        menu = QMenu(self)
        if self._on_add_to_call:
            menu.addAction("Add to call", lambda: self._on_add_to_call(self.contact))
            menu.addSeparator()
        if self._on_favorite:
            label = "Remove from favorites" if self.contact.favorited else "Add to favorites"
            menu.addAction(label, lambda: self._on_favorite(self.contact, not self.contact.favorited))
        copy = QAction("Copy email", menu)
        copy.triggered.connect(lambda: _copy(self.contact.email))
        menu.addAction(copy)
        if self._on_remove:
            menu.addSeparator()
            menu.addAction("Remove contact…", lambda: self._on_remove(self.contact))
        menu.exec(self.mapToGlobal(pos))


def _copy(text: str):
    from PyQt6.QtWidgets import QApplication
    QApplication.clipboard().setText(text)
