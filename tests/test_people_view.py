"""The People tab lists contacts like the Mac popover and calls through the store."""
import pytest
from PyQt6.QtWidgets import QApplication

from tuple_ui.model import Phase
from tuple_ui.store import Store
from tuple_ui.views.people import PeopleView

from .helpers import FakeTuple, wait_for


@pytest.fixture(scope="module")
def qapp():
    return QApplication.instance() or QApplication([])


@pytest.fixture
def ready(qapp, tmp_path):
    fake = FakeTuple(tmp_path)
    store = Store(executable=fake.executable, env=fake.env)
    view = PeopleView(store)
    store.start()
    assert wait_for(lambda: store.state.phase is Phase.READY)
    yield fake, store, view
    store.stop()


def test_people_view_groups_favorites_online_and_everyone(ready):
    fake, store, view = ready
    # Every online contact in the fixture is a favorite, so no "Online" section yet.
    assert view.list.section_titles() == ["Favorites", "Everyone"]
    assert view.list.row_count() == 24
    names = [row.contact.full_name for row in view.list.rows()]
    assert names[:6] == sorted(names[:6])                      # online favorites, alphabetical
    assert names[6] == "Vint Cerf"                              # offline favorite still in Favorites
    assert len(view.list.section_rows("Favorites")) == 7


def test_people_search_filters_by_name_or_email(ready):
    fake, store, view = ready
    view.search.setText("lovel")
    assert [row.contact.full_name for row in view.list.rows()] == ["Ada Lovelace"]

    view.search.setText("grace.hopper@")
    assert [row.contact.full_name for row in view.list.rows()] == ["Grace Hopper"]

    view.search.setText("")
    assert view.list.row_count() == 24


def test_call_button_starts_a_call_via_tuple(ready):
    fake, store, view = ready
    row = view.list.rows()[0]
    row.call_button.click()
    assert wait_for(lambda: ["call", "start", row.contact.email, "--format", "json"] in fake.calls())


def test_presence_change_event_updates_rows(ready):
    fake, store, view = ready
    contacts = [dict(c) for c in store.state.raw["contacts"]]
    target = next(c for c in contacts if c["full_name"] == "Ada Lovelace")
    target["status"] = "online"
    fake.emit("contacts", contacts)

    assert wait_for(lambda: any(r.contact.full_name == "Ada Lovelace" and r.contact.status == "online"
                                for r in view.list.rows()))
    assert view.list.section_titles() == ["Favorites", "Online", "Everyone"]
    assert [r.contact.full_name for r in view.list.section_rows("Online")] == ["Ada Lovelace"]
