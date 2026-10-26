"""Store drives the real app layers against the fake `tuple` subprocess."""
import pytest
from PyQt6.QtWidgets import QApplication

from tuple_ui.model import Phase
from tuple_ui.store import Store

from .helpers import FakeTuple, wait_for


@pytest.fixture(scope="module")
def qapp():
    return QApplication.instance() or QApplication([])


@pytest.fixture
def fake(tmp_path):
    return FakeTuple(tmp_path)


@pytest.fixture
def store(qapp, fake):
    store = Store(executable=fake.executable, env=fake.env)
    yield store
    store.stop()


def test_store_loads_initial_state_from_follow_stream(store, fake):
    seen = []
    store.state_changed.connect(seen.append)
    store.start()

    assert wait_for(lambda: seen and seen[-1].phase is Phase.READY)
    assert store.state.user.full_name == "Jo Example"
    assert len(store.state.contacts) == 24


def test_store_applies_live_events(store, fake):
    store.start()
    assert wait_for(lambda: store.state.phase is Phase.READY)

    for event in fake.solo_call_events():
        fake.emit(event["type"], event["data"])
        if event["type"] == "current_call" and event["data"] and event["data"].get("id"):
            break
    assert wait_for(lambda: store.state.call is not None and store.state.call.connection_state == "connected")

    fake.emit("current_call", None)
    assert wait_for(lambda: store.state.call is None)


def test_store_actions_shell_out_to_tuple(store, fake):
    store.start()
    assert wait_for(lambda: store.state.phase is Phase.READY)

    results = []
    store.command_finished.connect(lambda label, result: results.append((label, result)))
    store.mute()
    store.join_room("r24ab2")
    store.call_person("Ada Lovelace")
    assert wait_for(lambda: len(results) == 3)

    calls = fake.calls()
    assert ["call", "mute", "--call", "current"] in calls
    assert ["rooms", "join", "r24ab2", "--format", "json"] in calls
    assert ["call", "start", "Ada Lovelace", "--format", "json"] in calls
    assert all(result.ok for _, result in results)
    assert dict(results)["mute"].stdout.strip() == "Muted"


def test_store_reports_cli_failure_as_unavailable(store, fake):
    fake.fail_with("dial unix /run/user/1000/tuple.sock: connect: no such file")
    store.start()

    assert wait_for(lambda: store.state.phase is Phase.UNAVAILABLE)
    assert "tuple.sock" in store.state.error


def test_store_reports_missing_executable(qapp, fake):
    store = Store(executable="/nonexistent/tuple", env=fake.env)
    store.start()
    try:
        assert wait_for(lambda: store.state.phase is Phase.UNAVAILABLE)
        assert "not found" in store.state.error.lower() or "nonexistent" in store.state.error
    finally:
        store.stop()
