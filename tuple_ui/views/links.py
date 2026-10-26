"""Links tab: share your personal room link and join a call by URL."""
from __future__ import annotations

from PyQt6.QtWidgets import QApplication, QHBoxLayout, QLabel, QLineEdit, QPushButton, QVBoxLayout, QWidget

from ..store import Store


class LinksView(QWidget):
    def __init__(self, store: Store, parent=None):
        super().__init__(parent)
        self.store = store
        layout = QVBoxLayout(self)
        layout.setContentsMargins(12, 12, 12, 12)
        layout.setSpacing(8)

        title = QLabel("Links")
        title.setObjectName("title")
        layout.addWidget(title)

        hint = QLabel("Anyone with your personal room link can join you there. "
                      "Copy it to invite a guest or teammate.")
        hint.setObjectName("muted")
        hint.setWordWrap(True)
        layout.addWidget(hint)

        row = QHBoxLayout()
        self.link_field = QLineEdit()
        self.link_field.setReadOnly(True)
        row.addWidget(self.link_field, 1)
        self.copy_button = QPushButton("Copy link")
        self.copy_button.clicked.connect(self.copy_link)
        row.addWidget(self.copy_button)
        layout.addLayout(row)

        self.join_copy_button = QPushButton("Join call and copy link")
        self.join_copy_button.setProperty("kind", "primary")
        self.join_copy_button.clicked.connect(self.join_and_copy)
        layout.addWidget(self.join_copy_button)

        layout.addSpacing(16)
        join_title = QLabel("Join a call by link")
        join_title.setObjectName("title")
        layout.addWidget(join_title)
        join_row = QHBoxLayout()
        self.join_field = QLineEdit()
        self.join_field.setPlaceholderText("https://tuple.app/c/… or a person's name")
        self.join_field.returnPressed.connect(self.join_url)
        join_row.addWidget(self.join_field, 1)
        self.join_button = QPushButton("Join")
        self.join_button.clicked.connect(self.join_url)
        join_row.addWidget(self.join_button)
        layout.addLayout(join_row)

        self.active_label = QLabel("")
        self.active_label.setObjectName("subtle")
        self.active_label.setWordWrap(True)
        layout.addWidget(self.active_label)
        layout.addStretch(1)

        store.state_changed.connect(lambda _s: self.render())
        self.render()

    def personal_url(self) -> str:
        room = self.store.state.personal_room
        return room.url if room else ""

    def render(self):
        self.link_field.setText(self.personal_url())
        call = self.store.state.call
        if call and call.room:
            self.active_label.setText(f"Current call: {call.room.url}")
        else:
            self.active_label.setText("")

    def copy_link(self):
        QApplication.clipboard().setText(self.personal_url())

    def join_and_copy(self):
        room = self.store.state.personal_room
        if not room:
            return
        self.copy_link()
        self.store.join_room(room.slug, switch=self.store.state.in_call)

    def join_url(self):
        target = self.join_field.text().strip()
        if not target:
            return
        self.join_field.clear()
        self.store.join(target, switch=self.store.state.in_call)
