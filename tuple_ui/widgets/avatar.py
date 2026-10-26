"""Round avatar with initials fallback, Gravatar loading and a presence dot."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path

from PyQt6.QtCore import QRectF, QSize, Qt, QUrl
from PyQt6.QtGui import QBrush, QColor, QFont, QPainter, QPainterPath, QPen, QPixmap
from PyQt6.QtNetwork import QNetworkAccessManager, QNetworkDiskCache, QNetworkReply, QNetworkRequest
from PyQt6.QtWidgets import QWidget

from ..model import Person
from ..theme import BG_PANEL, BUSY, ONLINE, TEXT_SUBTLE

PALETTE = ["#7c5cff", "#2f81f7", "#1f9d8b", "#d1731e", "#c23a6c", "#5a8f2d", "#9b59b6", "#2b7a9e"]
PRESENCE_COLOR = {"online": ONLINE, "busy": BUSY, "offline": TEXT_SUBTLE}


class AvatarCache:
    """Process-wide Gravatar cache backed by a disk cache in $XDG_CACHE_HOME."""

    _instance: "AvatarCache | None" = None

    def __init__(self):
        self.manager = QNetworkAccessManager()
        cache_dir = Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache") / "tuple-ui" / "avatars"
        disk = QNetworkDiskCache(self.manager)
        disk.setCacheDirectory(str(cache_dir))
        self.manager.setCache(disk)
        self.pixmaps: dict[str, QPixmap] = {}
        self.waiting: dict[str, list] = {}

    @classmethod
    def shared(cls) -> "AvatarCache":
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    def fetch(self, url: str, callback):
        if not url or os.environ.get("TUPLE_UI_OFFLINE"):
            return
        if url in self.pixmaps:
            callback(self.pixmaps[url])
            return
        if url in self.waiting:
            self.waiting[url].append(callback)
            return
        self.waiting[url] = [callback]
        request = QNetworkRequest(QUrl(url))
        request.setAttribute(QNetworkRequest.Attribute.CacheLoadControlAttribute,
                             QNetworkRequest.CacheLoadControl.PreferCache)
        reply = self.manager.get(request)
        reply.finished.connect(lambda: self._finished(url, reply))

    def _finished(self, url: str, reply: QNetworkReply):
        callbacks = self.waiting.pop(url, [])
        pixmap = QPixmap()
        if reply.error() == QNetworkReply.NetworkError.NoError:
            pixmap.loadFromData(reply.readAll())
        reply.deleteLater()
        if pixmap.isNull() or _is_blank(pixmap):
            return
        self.pixmaps[url] = pixmap
        for callback in callbacks:
            callback(pixmap)


def _is_blank(pixmap: QPixmap) -> bool:
    """Gravatar's `d=blank` fallback is a fully transparent image."""
    image = pixmap.toImage()
    step = max(1, image.width() // 8)
    for x in range(0, image.width(), step):
        for y in range(0, image.height(), step):
            if image.pixelColor(x, y).alpha() > 0:
                return False
    return True


def color_for(name: str) -> QColor:
    digest = hashlib.md5(name.encode("utf-8")).digest()
    return QColor(PALETTE[digest[0] % len(PALETTE)])


class Avatar(QWidget):
    def __init__(self, person: Person | None, size: int = 36, presence: str | None = None, parent=None):
        super().__init__(parent)
        self._size = size
        self.person = person
        self.presence = presence
        self.pixmap: QPixmap | None = None
        self.setFixedSize(QSize(size, size))
        if person and person.avatar_url:
            AvatarCache.shared().fetch(person.avatar_url, self._set_pixmap)

    @property
    def initials(self) -> str:
        return self.person.initials if self.person else "?"

    def set_presence(self, presence: str | None):
        if presence != self.presence:
            self.presence = presence
            self.update()

    def _set_pixmap(self, pixmap: QPixmap):
        self.pixmap = pixmap
        self.update()

    def paintEvent(self, _event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        inset = 1.0
        circle = QRectF(inset, inset, self._size - 2 * inset, self._size - 2 * inset)
        name = self.person.full_name if self.person else "?"
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QBrush(color_for(name)))
        painter.drawEllipse(circle)
        if self.pixmap is not None:
            path = QPainterPath()
            path.addEllipse(circle)
            painter.setClipPath(path)
            scaled = self.pixmap.scaled(self._size, self._size, Qt.AspectRatioMode.KeepAspectRatioByExpanding,
                                        Qt.TransformationMode.SmoothTransformation)
            painter.drawPixmap(0, 0, scaled)
            painter.setClipping(False)
        else:
            font = QFont()
            font.setBold(True)
            font.setPixelSize(max(9, int(self._size * 0.4)))
            painter.setFont(font)
            painter.setPen(QColor("white"))
            painter.drawText(circle, Qt.AlignmentFlag.AlignCenter, self.initials)
        color = PRESENCE_COLOR.get(self.presence or "")
        if color:
            dot = max(8, self._size // 4)
            rect = QRectF(self._size - dot, self._size - dot, dot, dot)
            painter.setBrush(QBrush(QColor(color)))
            painter.setPen(QPen(QColor(BG_PANEL), 2))
            painter.drawEllipse(rect)
        painter.end()
