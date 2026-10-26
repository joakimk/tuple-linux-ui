"""Shared helpers for driving the app against the fake `tuple` executable."""
import json
import os
import shutil
import time
from pathlib import Path

from PyQt6.QtCore import QCoreApplication

FAKE_BIN = Path(__file__).parent / "fake_tuple"
FIXTURES = Path(__file__).parent / "fixtures"


def wait_for(predicate, timeout=3.0, step=0.01):
    """Pump the Qt event loop until `predicate()` is truthy or the timeout hits."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        QCoreApplication.processEvents()
        if predicate():
            return True
        time.sleep(step)
    QCoreApplication.processEvents()
    return bool(predicate())


class FakeTuple:
    """A scenario directory the fake `tuple` executable reads from."""

    def __init__(self, tmp_path: Path):
        self.dir = tmp_path / "scenario"
        self.dir.mkdir()
        self.log = tmp_path / "tuple.log"
        self.env = dict(os.environ, FAKE_TUPLE_DIR=str(self.dir), FAKE_TUPLE_LOG=str(self.log),
                        PATH=f"{FAKE_BIN}:{os.environ.get('PATH', '')}")
        shutil.copy(FIXTURES / "state_raw_idle.json", self.dir / "state_raw.json")
        first = (FIXTURES / "follow_raw_solo_call.jsonl").read_text().splitlines()[0]
        (self.dir / "follow.jsonl").write_text(first + "\n")

    @property
    def executable(self) -> str:
        return str(FAKE_BIN / "tuple")

    def emit(self, event_type, data):
        with open(self.dir / "events.jsonl", "a") as f:
            f.write(json.dumps({"type": event_type, "data": data}) + "\n")

    def fail_with(self, message):
        (self.dir / "error").write_text(message)

    def calls(self):
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text().splitlines() if line.strip()]

    def solo_call_events(self):
        lines = (FIXTURES / "follow_raw_solo_call.jsonl").read_text().splitlines()[1:]
        return [json.loads(line) for line in lines if line.strip()]
