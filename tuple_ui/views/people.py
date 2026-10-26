"""People tab: searchable contact list grouped like the Mac popover."""
from __future__ import annotations

from PyQt6.QtWidgets import QHBoxLayout, QInputDialog, QLineEdit, QMessageBox, QPushButton, QVBoxLayout, QWidget

from ..model import AppState, Contact
from ..store import Store
from ..widgets.contact_row import ContactRow
from ..widgets.section_list import SectionList


def matches(contact: Contact, query: str) -> bool:
    query = query.strip().lower()
    return not query or query in contact.full_name.lower() or query in contact.email.lower()


def group_contacts(state: AppState, query: str) -> list[tuple[str, list[Contact]]]:
    contacts = [c for c in state.sorted_contacts if matches(c, query)]
    favorites = [c for c in contacts if c.favorited]
    online = [c for c in contacts if not c.favorited and c.status in ("online", "busy")]
    everyone = [c for c in contacts if not c.favorited and c.status not in ("online", "busy")]
    return [("Favorites", favorites), ("Online", online), ("Everyone", everyone)]


class PeopleView(QWidget):
    def __init__(self, store: Store, parent=None):
        super().__init__(parent)
        self.store = store
        self._rendered: tuple = ()
        layout = QVBoxLayout(self)
        layout.setContentsMargins(8, 8, 8, 0)
        layout.setSpacing(6)

        top = QHBoxLayout()
        self.search = QLineEdit()
        self.search.setPlaceholderText("Search people")
        self.search.setClearButtonEnabled(True)
        self.search.textChanged.connect(lambda _t: self.render(force=True))
        top.addWidget(self.search, 1)
        self.invite_button = QPushButton("＋")
        self.invite_button.setProperty("kind", "flat")
        self.invite_button.setToolTip("Invite someone to Tuple by email")
        self.invite_button.clicked.connect(self.invite)
        top.addWidget(self.invite_button)
        layout.addLayout(top)

        self.list = SectionList()
        layout.addWidget(self.list, 1)
        store.state_changed.connect(lambda _s: self.render())
        self.render(force=True)

    def focus_search(self):
        self.search.setFocus()
        self.search.selectAll()

    def render(self, force: bool = False):
        state = self.store.state
        key = (state.contacts, state.in_call)
        if not force and key == self._rendered:
            return
        self._rendered = key
        sections = []
        for title, contacts in group_contacts(state, self.search.text()):
            rows = [ContactRow(c, on_call=self.call, on_favorite=self.favorite, on_remove=self.remove,
                               on_add_to_call=self.add_to_call if state.in_call else None)
                    for c in contacts]
            sections.append((title, rows))
        empty = "No contacts match your search." if self.search.text() else "No contacts yet. Invite a teammate with ＋."
        self.list.set_sections(sections, empty_text=empty)

    # ------------------------------------------------------------- actions
    def call(self, contact: Contact):
        if self.store.state.in_call:
            self.store.add_participant(contact.email)
        else:
            self.store.call_person(contact.email)

    def add_to_call(self, contact: Contact):
        self.store.add_participant(contact.email)

    def favorite(self, contact: Contact, favorited: bool):
        self.store.favorite_contact(contact.email, favorited)

    def remove(self, contact: Contact):
        answer = QMessageBox.question(self, "Remove contact",
                                      f"Remove {contact.full_name} from your contacts?")
        if answer == QMessageBox.StandardButton.Yes:
            self.store.remove_contact(contact.email)

    def invite(self):
        email, ok = QInputDialog.getText(self, "Invite to Tuple", "Email address:")
        if ok and email.strip():
            self.store.invite(email.strip())
