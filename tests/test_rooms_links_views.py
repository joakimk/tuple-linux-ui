"""Rooms and Links tabs."""
import pytest
from PyQt6.QtWidgets import QApplication

from tuple_ui.model import Phase
from tuple_ui.store import Store
from tuple_ui.views.links import LinksView
from tuple_ui.views.rooms import RoomsView

from .helpers import FakeTuple, wait_for


@pytest.fixture(scope="module")
def qapp():
    return QApplication.instance() or QApplication([])


@pytest.fixture
def ready(qapp, tmp_path):
    fake = FakeTuple(tmp_path)
    store = Store(executable=fake.executable, env=fake.env)
    store.start()
    assert wait_for(lambda: store.state.phase is Phase.READY)
    yield fake, store
    store.stop()


def test_rooms_view_pins_personal_room_then_lists_team_rooms(ready):
    fake, store = ready
    view = RoomsView(store)
    assert view.personal_card.name_label.text() == "Your personal room"
    names = [row.room.name for row in view.list.rows()]
    assert len(names) == 10
    assert names == sorted(names, key=str.lower)


def test_favorited_rooms_float_to_the_top(ready):
    fake, store = ready
    view = RoomsView(store)
    target = view.list.rows()[-1].room
    store.favorite_room(target.slug, True)
    assert view.list.rows()[0].room.slug == target.slug          # optimistic update
    assert wait_for(lambda: ["rooms", "favorite", target.slug] in fake.calls())


def test_join_buttons_shell_out(ready):
    fake, store = ready
    view = RoomsView(store)
    view.personal_card.join_button.click()
    assert wait_for(lambda: ["rooms", "join", "r24ab2", "--format", "json"] in fake.calls())
    assert QApplication.clipboard().text() == store.state.personal_room.url   # join also copies the link

    first = view.list.rows()[0]
    first.join_button.click()
    assert wait_for(lambda: ["rooms", "join", first.room.slug, "--format", "json"] in fake.calls())


def test_room_occupancy_updates_from_rooms_event(ready):
    fake, store = ready
    view = RoomsView(store)
    rooms = {k: [dict(r) for r in v] for k, v in store.state.raw["rooms"].items()}
    rooms["team"][0]["members"] = [{"id": 1, "email": "x@example.com", "full_name": "Ada Lovelace", "short_name": "Ada"}]
    fake.emit("rooms", rooms)
    slug = rooms["team"][0]["slug"]
    assert wait_for(lambda: any(r.room.slug == slug and r.occupancy_label.text() == "Ada Lovelace"
                                for r in view.list.rows()))


def test_links_view_copies_and_joins_by_url(ready):
    fake, store = ready
    view = LinksView(store)
    assert store.state.personal_room.url in view.link_field.text()
    view.copy_button.click()
    assert QApplication.clipboard().text() == store.state.personal_room.url

    view.join_field.setText("https://tuple.app/c/abc123")
    view.join_button.click()
    assert wait_for(lambda: ["call", "join", "https://tuple.app/c/abc123", "--format", "json"] in fake.calls())
    assert view.join_field.text() == ""
