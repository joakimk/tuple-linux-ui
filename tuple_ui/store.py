"""Store: owns the live `tuple state follow --raw` stream and dispatches CLI actions.

Views subscribe to `state_changed(AppState)` and call the action methods;
each action shells out to `tuple` and reports back through
`command_finished(label, CliResult)`.
"""
from __future__ import annotations

import json
import shlex

from PyQt6.QtCore import QObject, QProcess, QTimer, pyqtSignal

from .cli import CliResult, TupleCli
from .model import AppState, Phase

FOLLOW_ARGS = ["state", "follow", "--raw", "--format", "json"]
RESTART_DELAYS_MS = [1000, 2000, 4000, 8000, 10000]


class Store(QObject):
    state_changed = pyqtSignal(object)            # AppState
    command_finished = pyqtSignal(str, object)    # label, CliResult
    notice = pyqtSignal(str, str)                 # title, body (surfaced to the user)

    def __init__(self, executable: str = "tuple", env: dict[str, str] | None = None, parent=None):
        super().__init__(parent)
        self.cli = TupleCli(executable, env, self)
        self.state = AppState(phase=Phase.CONNECTING)
        self._follow: QProcess | None = None
        self._buffer = b""
        self._restarts = 0
        self._stopping = False
        self._saw_state = False

    # ------------------------------------------------------------ lifecycle
    def start(self):
        self._stopping = False
        self._start_follow()

    def stop(self):
        self._stopping = True
        proc, self._follow = self._follow, None
        if proc is not None and proc.state() != QProcess.ProcessState.NotRunning:
            proc.kill()
            proc.waitForFinished(1000)
        self.cli.shutdown()

    def _start_follow(self):
        if self._stopping:
            return
        self._buffer = b""
        self._saw_state = False
        proc = self.cli.spawn_stream(FOLLOW_ARGS)
        self._follow = proc
        proc.readyReadStandardOutput.connect(lambda: self._on_follow_output(proc))
        proc.finished.connect(lambda code, _s: self._on_follow_finished(proc, code))
        proc.errorOccurred.connect(lambda err: self._on_follow_error(proc, err))

    def _on_follow_output(self, proc: QProcess):
        if proc is not self._follow:
            return
        self._buffer += bytes(proc.readAllStandardOutput())
        while b"\n" in self._buffer:
            line, self._buffer = self._buffer.split(b"\n", 1)
            self._handle_line(line.decode("utf-8", "replace"))

    def _handle_line(self, line: str):
        line = line.strip()
        if not line:
            return
        try:
            event = json.loads(line)
        except ValueError:
            return
        if not isinstance(event, dict) or "type" not in event:
            return
        self._saw_state = True
        self._restarts = 0
        self._set_state(self.state.apply_event(event["type"], event.get("data")))

    def _on_follow_error(self, proc: QProcess, err):
        if proc is not self._follow or err != QProcess.ProcessError.FailedToStart:
            return
        self._follow = None
        self._set_state(AppState.unavailable(f"`{self.cli.executable}` not found on PATH"))
        self._schedule_restart()

    def _on_follow_finished(self, proc: QProcess, code: int):
        if proc is not self._follow:
            return
        self._follow = None
        if self._stopping:
            return
        stderr = bytes(proc.readAllStandardError()).decode("utf-8", "replace").strip()
        if not self._saw_state:
            message = stderr.splitlines()[-1] if stderr else f"`tuple state follow` exited with {code}"
            self._set_state(AppState.unavailable(message))
        else:
            self._set_state(AppState(phase=Phase.CONNECTING, error=stderr))
        self._schedule_restart()

    def _schedule_restart(self):
        delay = RESTART_DELAYS_MS[min(self._restarts, len(RESTART_DELAYS_MS) - 1)]
        self._restarts += 1
        QTimer.singleShot(delay, self._start_follow)

    def _set_state(self, state: AppState):
        self.state = state
        self.state_changed.emit(state)

    # -------------------------------------------------------------- actions
    def _run(self, label: str, args: list[str], as_json: bool = False):
        full = list(args) + (["--format", "json"] if as_json else [])

        def done(result: CliResult):
            self.command_finished.emit(label, result)
            if not result.ok:
                self.notice.emit(f"{label.replace('_', ' ').capitalize()} failed", result.error)

        return self.cli.run(full, done)

    # calls
    def call_person(self, name_or_email: str):
        return self._run("call", ["call", "start", name_or_email], as_json=True)

    def join_room(self, slug_or_url: str, switch: bool = False):
        args = ["rooms", "join", slug_or_url] + (["--switch"] if switch else [])
        return self._run("join_room", args, as_json=True)

    def join(self, person_or_url: str, switch: bool = False):
        args = ["call", "join", person_or_url] + (["--switch"] if switch else [])
        return self._run("join", args, as_json=True)

    def accept(self, call_id: str = "current"):
        return self._run("accept", ["call", "accept", "--call", call_id])

    def leave(self, call_id: str = "current"):
        return self._run("leave", ["call", "leave", "--call", call_id])

    def mute(self):
        return self._run("mute", ["call", "mute", "--call", "current"])

    def unmute(self):
        return self._run("unmute", ["call", "unmute", "--call", "current"])

    def toggle_mute(self):
        if self.state.call is None:
            return None
        return self.unmute() if self.state.call.muted else self.mute()

    def share_screen(self):
        return self._run("share_screen", ["share"])

    def stop_sharing(self):
        return self._run("stop_sharing", ["unshare"])

    def toggle_share(self):
        if self.state.call is None:
            return None
        return self.stop_sharing() if self.state.call.sharing else self.share_screen()

    def react(self, sound: str):
        return self._run("react", ["call", "react", sound])

    def add_participant(self, person: str):
        return self._run("add_participant", ["call", "participants", "add", person])

    def remove_participant(self, person: str):
        return self._run("remove_participant", ["call", "participants", "remove", person])

    def edit_call(self, title: str | None = None, summary: str | None = None):
        args = ["call", "edit"]
        if title is not None:
            args += ["--title", title]
        if summary is not None:
            args += ["--summary", summary]
        return self._run("edit_call", args)

    def capture_start(self):
        return self._run("capture_start", ["capture", "start"])

    def capture_stop(self):
        return self._run("capture_stop", ["capture", "stop"])

    # contacts & rooms
    def favorite_contact(self, person: str, favorited: bool):
        verb = "favorite" if favorited else "unfavorite"
        return self._run("favorite_contact", ["contacts", verb, person])

    def remove_contact(self, person: str):
        return self._run("remove_contact", ["contacts", "remove", person])

    def invite(self, email: str):
        return self._run("invite", ["contacts", "invite", email])

    def favorite_room(self, slug: str, favorited: bool):
        verb = "favorite" if favorited else "unfavorite"
        self._set_state(self.state.with_room_favorited(slug, favorited))
        return self._run("favorite_room", ["rooms", verb, slug])

    def create_room(self, name: str):
        return self._run("create_room", ["rooms", "create", name], as_json=True)

    # session
    def login(self):
        return self._run("login", ["login"])

    def auth(self, code: str):
        return self._run("auth", ["auth", code])

    def logout(self):
        return self._run("logout", ["logout"])

    def start_daemon(self):
        return self._run("start_daemon", ["start"])

    def stop_daemon(self):
        return self._run("stop_daemon", ["stop"])

    def open_native_ui(self):
        return self._run("native_ui", ["ui"])

    @staticmethod
    def describe(args: list[str]) -> str:
        return "tuple " + " ".join(shlex.quote(a) for a in args)
