"""UI-only preferences, stored under $XDG_CONFIG_HOME/tuple-ui/prefs.json.

These are separate from Tuple's own settings (`tuple settings` / `tuple set`).
"""
from __future__ import annotations

import json
import os
from pathlib import Path

DEFAULTS = {
    "show_window_on_start": True,
    "start_in_tray": False,
    "notify_incoming_calls": True,
    "notify_missed_calls": True,
    "window_geometry": None,
    "last_tab": "people",
}


def default_config_dir() -> Path:
    return Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")


class Prefs:
    def __init__(self, config_dir: Path | None = None):
        self.path = (config_dir or default_config_dir()) / "tuple-ui" / "prefs.json"
        self._data = dict(DEFAULTS)
        self.load()

    def load(self):
        try:
            loaded = json.loads(self.path.read_text())
        except (OSError, ValueError):
            return
        if isinstance(loaded, dict):
            self._data.update({k: v for k, v in loaded.items() if k in DEFAULTS})

    def save(self):
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            self.path.write_text(json.dumps(self._data, indent=2) + "\n")
        except OSError as e:
            print(f"tuple-ui: could not save prefs to {self.path}: {e}")

    def get(self, key):
        return self._data.get(key, DEFAULTS.get(key))

    def set(self, key, value):
        self._data[key] = value

    show_window_on_start = property(lambda s: bool(s.get("show_window_on_start")),
                                    lambda s, v: s.set("show_window_on_start", bool(v)))
    start_in_tray = property(lambda s: bool(s.get("start_in_tray")),
                             lambda s, v: s.set("start_in_tray", bool(v)))
    notify_incoming_calls = property(lambda s: bool(s.get("notify_incoming_calls")),
                                     lambda s, v: s.set("notify_incoming_calls", bool(v)))
    notify_missed_calls = property(lambda s: bool(s.get("notify_missed_calls")),
                                   lambda s, v: s.set("notify_missed_calls", bool(v)))
    window_geometry = property(lambda s: s.get("window_geometry"),
                               lambda s, v: s.set("window_geometry", v))
    last_tab = property(lambda s: s.get("last_tab") or "people",
                        lambda s, v: s.set("last_tab", v))
