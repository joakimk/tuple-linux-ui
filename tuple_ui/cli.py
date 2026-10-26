"""Asynchronous wrapper around the `tuple` executable (QProcess based).

Every call returns a `CliResult`; nothing here blocks the UI thread.
"""
from __future__ import annotations

import json
import shutil
from dataclasses import dataclass, field
from typing import Any, Callable

from PyQt6.QtCore import QObject, QProcess, QProcessEnvironment, QTimer

DEFAULT_TIMEOUT_MS = 30_000


@dataclass
class CliResult:
    args: list[str]
    exit_code: int
    stdout: str = ""
    stderr: str = ""
    failed_to_start: bool = False
    timed_out: bool = False
    extra: dict = field(default_factory=dict)

    @property
    def ok(self) -> bool:
        return self.exit_code == 0 and not self.failed_to_start and not self.timed_out

    @property
    def json(self) -> Any:
        try:
            return json.loads(self.stdout)
        except (ValueError, TypeError):
            return None

    @property
    def error(self) -> str:
        """Best human-readable error text for a failed command."""
        if self.failed_to_start:
            return f"Could not start `tuple` ({' '.join(self.args)}): executable not found"
        if self.timed_out:
            return f"`tuple {' '.join(self.args)}` timed out"
        payload = self.json
        if isinstance(payload, dict) and payload.get("error"):
            return str(payload["error"])
        text = (self.stderr or self.stdout).strip()
        return text.splitlines()[-1] if text else f"`tuple {' '.join(self.args)}` failed ({self.exit_code})"

    @property
    def command(self) -> str:
        return "tuple " + " ".join(self.args)


class TupleCli(QObject):
    """Spawns `tuple` commands and long-running follow streams."""

    def __init__(self, executable: str = "tuple", env: dict[str, str] | None = None, parent=None):
        super().__init__(parent)
        self.executable = executable
        self._env = env
        self._live: set[QProcess] = set()

    def available(self) -> bool:
        return shutil.which(self.executable) is not None

    def _make_process(self) -> QProcess:
        proc = QProcess(self)
        if self._env is not None:
            qenv = QProcessEnvironment()
            for key, value in self._env.items():
                qenv.insert(key, value)
            proc.setProcessEnvironment(qenv)
        return proc

    def run(self, args: list[str], callback: Callable[[CliResult], None] | None = None,
            timeout_ms: int = DEFAULT_TIMEOUT_MS) -> QProcess:
        """Run `tuple <args>` asynchronously; `callback` receives the CliResult."""
        proc = self._make_process()
        result = CliResult(args=list(args), exit_code=-1)
        self._live.add(proc)

        def finish():
            if proc not in self._live:
                return
            self._live.discard(proc)
            result.stdout = bytes(proc.readAllStandardOutput()).decode("utf-8", "replace")
            result.stderr = bytes(proc.readAllStandardError()).decode("utf-8", "replace")
            if callback:
                callback(result)
            proc.deleteLater()

        def on_finished(code, _status):
            result.exit_code = code
            finish()

        def on_error(err):
            if err == QProcess.ProcessError.FailedToStart:
                result.failed_to_start = True
                finish()

        def on_timeout():
            if proc in self._live and proc.state() != QProcess.ProcessState.NotRunning:
                result.timed_out = True
                proc.kill()

        proc.finished.connect(on_finished)
        proc.errorOccurred.connect(on_error)
        timer = QTimer(proc)
        timer.setSingleShot(True)
        timer.timeout.connect(on_timeout)
        timer.start(timeout_ms)
        proc.start(self.executable, list(args))
        return proc

    def spawn_stream(self, args: list[str]) -> QProcess:
        """Start a long-running `tuple <args>`; the caller reads stdout lines itself."""
        proc = self._make_process()
        proc.start(self.executable, list(args))
        return proc

    def shutdown(self):
        for proc in list(self._live):
            self._live.discard(proc)
            if proc.state() != QProcess.ProcessState.NotRunning:
                proc.kill()
