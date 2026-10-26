"""Reusable widgets: avatars, contact rows and room rows."""
from dataclasses import replace

import pytest
from PyQt6.QtWidgets import QApplication

from tuple_ui.model import AppState
from tuple_ui.widgets.avatar import Avatar
from tuple_ui.widgets.contact_row import ContactRow
from tuple_ui.widgets.room_row import RoomRow
from tuple_ui.widgets.section_list import SectionList


@pytest.fixture(scope="module")
def qapp():
    return QApplication.instance() or QApplication([])


@pytest.fixture
def state(raw_idle):
    return AppState.from_raw(raw_idle)


def test_avatar_shows_initials_and_presence(qapp, state):
    contact = state.sorted_contacts[0]
    avatar = Avatar(contact, size=32, presence=contact.status)
    assert avatar.initials == contact.initials
    assert avatar.presence == "online"
    assert avatar.size().width() == 32


def test_contact_row_calls_back_with_the_contact(qapp, state):
    contact = state.sorted_contacts[0]
    called = []
    row = ContactRow(contact, on_call=called.append)
    assert row.name_label.text() == contact.full_name
    assert row.status_label.text() == "Online"

    row.call_button.click()
    assert called == [contact]


def test_contact_row_status_text_for_each_presence(qapp, state):
    texts = {}
    for status in ("online", "busy", "offline", "unknown"):
        contact = replace(state.contacts[0], status=status)
        texts[status] = ContactRow(contact, on_call=lambda c: None).status_label.text()
    assert texts == {"online": "Online", "busy": "Busy", "offline": "Offline", "unknown": ""}


def test_room_row_shows_occupants_and_joins(qapp, state, follow_events):
    for event in follow_events[1:]:
        state = state.apply_event(event["type"], event["data"])
        if state.personal_room.occupied:
            break
    joined = []
    row = RoomRow(state.personal_room, on_join=joined.append)
    assert row.name_label.text() == "Your personal room"
    assert row.occupancy_label.text() == "Jo Example"
    row.join_button.click()
    assert joined == [state.personal_room]

    empty = RoomRow(state.sorted_team_rooms[0], on_join=joined.append)
    assert empty.occupancy_label.text() == "Empty"


def test_section_list_rebuilds_sections(qapp, state):
    section_list = SectionList()
    rows = [ContactRow(c, on_call=lambda c: None) for c in state.sorted_contacts[:3]]
    section_list.set_sections([("Favorites", rows[:2]), ("Teammates", rows[2:])])
    assert section_list.section_titles() == ["Favorites", "Teammates"]
    assert section_list.row_count() == 3

    section_list.set_sections([("Teammates", [ContactRow(state.sorted_contacts[0], on_call=lambda c: None)])])
    assert section_list.section_titles() == ["Teammates"]
    assert section_list.row_count() == 1
