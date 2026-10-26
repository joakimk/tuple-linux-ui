"""UI preferences live in the XDG config dir and survive a round trip."""
from tuple_ui.prefs import Prefs


def test_defaults_when_no_file(tmp_path):
    prefs = Prefs(config_dir=tmp_path)
    assert prefs.show_window_on_start is True
    assert prefs.notify_incoming_calls is True
    assert prefs.start_in_tray is False
    assert prefs.window_geometry is None


def test_round_trip_persists_to_xdg_config(tmp_path):
    prefs = Prefs(config_dir=tmp_path)
    prefs.show_window_on_start = False
    prefs.window_geometry = "AAAA"
    prefs.save()

    assert (tmp_path / "tuple-ui" / "prefs.json").exists()
    reloaded = Prefs(config_dir=tmp_path)
    assert reloaded.show_window_on_start is False
    assert reloaded.window_geometry == "AAAA"


def test_corrupt_file_falls_back_to_defaults(tmp_path):
    (tmp_path / "tuple-ui").mkdir()
    (tmp_path / "tuple-ui" / "prefs.json").write_text("{not json")
    prefs = Prefs(config_dir=tmp_path)
    assert prefs.show_window_on_start is True
