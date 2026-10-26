import json
import os
from pathlib import Path

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

FIXTURES = Path(__file__).parent / "fixtures"


@pytest.fixture
def fixtures():
    return FIXTURES


@pytest.fixture
def raw_idle():
    return json.loads((FIXTURES / "state_raw_idle.json").read_text())


@pytest.fixture
def follow_events():
    lines = (FIXTURES / "follow_raw_solo_call.jsonl").read_text().splitlines()
    return [json.loads(line) for line in lines if line.strip()]
