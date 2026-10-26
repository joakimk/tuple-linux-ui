"""A scrollable list of titled sections, each holding row widgets."""
from __future__ import annotations

from PyQt6.QtCore import Qt
from PyQt6.QtWidgets import QLabel, QScrollArea, QVBoxLayout, QWidget


class SectionList(QScrollArea):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWidgetResizable(True)
        self.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._container = QWidget()
        self._layout = QVBoxLayout(self._container)
        self._layout.setContentsMargins(4, 0, 4, 8)
        self._layout.setSpacing(0)
        self._layout.addStretch(1)
        self.setWidget(self._container)
        self._sections: list[tuple[str, list[QWidget]]] = []
        self.empty_label = QLabel("")
        self.empty_label.setObjectName("muted")
        self.empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.empty_label.setWordWrap(True)
        self.empty_label.hide()
        self._layout.insertWidget(0, self.empty_label)

    def set_sections(self, sections: list[tuple[str, list[QWidget]]], empty_text: str = ""):
        self.clear()
        self._sections = [(title, list(rows)) for title, rows in sections if rows]
        position = 1  # after the empty label
        for title, rows in self._sections:
            if title:
                label = QLabel(title.upper())
                label.setObjectName("sectionTitle")
                self._layout.insertWidget(position, label)
                position += 1
            for row in rows:
                self._layout.insertWidget(position, row)
                position += 1
        self.empty_label.setText(empty_text)
        self.empty_label.setVisible(not self._sections and bool(empty_text))

    def clear(self):
        while self._layout.count() > 2:  # keep the empty label and trailing stretch
            item = self._layout.takeAt(1)
            widget = item.widget()
            if widget is not None:
                widget.setParent(None)
                widget.deleteLater()
        self._sections = []

    def section_titles(self) -> list[str]:
        return [title for title, _ in self._sections]

    def rows(self) -> list[QWidget]:
        return [row for _, rows in self._sections for row in rows]

    def section_rows(self, title: str) -> list[QWidget]:
        return next((rows for t, rows in self._sections if t == title), [])

    def row_count(self) -> int:
        return len(self.rows())
