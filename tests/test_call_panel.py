"""In-call panel: participants, mute/share/leave controls and reactions."""
import pytest
from PyQt6.QtWidgets import QApplication

from tuple_ui.model import Phase
from tuple_ui.store import Store
from tuple_ui.views.call_panel import CallPanel

from .helpers import FakeTuple, wait_for


@pytest.fixture(scope="module")
def qapp():
    return QApplication.instance() or QApplication([])


@pytest.fixture
def in_call(qapp, tmp_path):
    fake = FakeTuple(tmp_path)
    store = Store(executable=fake.executable, env=fake.env)
    panel = CallPanel(store)
    store.start()
    assert wait_for(lambda: store.state.phase is Phase.READY)
    assert panel.isHidden()
    for event in fake.solo_call_events():
        fake.emit(event["type"], event["data"])
        if event["type"] == "current_call" and event["data"] and event["data"].get("id"):
            break
    assert wait_for(lambda: store.state.call is not None and store.state.call.connection_state == "connected")
    QApplication.processEvents()
    yield fake, store, panel
    store.stop()


def test_panel_appears_with_room_name_and_participants(in_call):
    fake, store, panel = in_call
    assert not panel.isHidden()
    assert panel.title_label.text() == "Your personal room"
    assert panel.participants_label.text() == "Just you"


def test_mute_button_reflects_state_and_toggles(in_call):
    fake, store, panel = in_call
    assert store.state.call.muted                       # fixture joined with audio disabled
    assert panel.mute_button.text() == "Unmute"
    panel.mute_button.click()
    assert wait_for(lambda: ["call", "unmute", "--call", "current"] in fake.calls())

    call = dict(store.state.raw["current_call"])
    call["local"] = dict(call["local"], audio_enabled=True)
    fake.emit("current_call", call)
    assert wait_for(lambda: panel.mute_button.text() == "Mute")
    panel.mute_button.click()
    assert wait_for(lambda: ["call", "mute", "--call", "current"] in fake.calls())


def test_share_leave_and_reactions(in_call):
    fake, store, panel = in_call
    assert panel.share_button.text() == "Share screen"
    panel.share_button.click()
    assert wait_for(lambda: ["share"] in fake.calls())

    call = dict(store.state.raw["current_call"])
    call["local"] = dict(call["local"], share_state="screen")
    fake.emit("current_call", call)
    assert wait_for(lambda: panel.share_button.text() == "Stop sharing")

    panel.react("airhorn")
    assert wait_for(lambda: ["call", "react", "airhorn"] in fake.calls())

    panel.leave_button.click()
    assert wait_for(lambda: ["call", "leave", "--call", "current"] in fake.calls())


def test_participants_listed_with_other_people(in_call):
    fake, store, panel = in_call
    call = dict(store.state.raw["current_call"])
    call["participants"] = [
        {"id": 2, "email": "ada@example.com", "full_name": "Ada Lovelace", "short_name": "Ada",
         "audio_enabled": True, "share_state": "screen"},
        {"id": 3, "email": "grace@example.com", "full_name": "Grace Hopper", "short_name": "Grace",
         "audio_enabled": False},
    ]
    fake.emit("current_call", call)
    assert wait_for(lambda: panel.participants_label.text() == "Ada Lovelace, Grace Hopper")
    rows = panel.participant_rows()
    assert [r.name_label.text() for r in rows] == ["Ada Lovelace", "Grace Hopper"]
    assert rows[0].detail_label.text() == "Sharing screen"
    assert rows[1].detail_label.text() == "Muted"


def test_panel_hides_when_call_ends(in_call):
    fake, store, panel = in_call
    fake.emit("current_call", None)
    assert wait_for(lambda: panel.isHidden())
