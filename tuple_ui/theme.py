"""Tuple-flavoured dark theme: palette constants and the global QSS."""

BG = "#1b1c20"
BG_PANEL = "#232429"
BG_ELEVATED = "#2c2e34"
BG_HOVER = "#34363d"
BORDER = "#3a3c44"
TEXT = "#ececf1"
TEXT_MUTED = "#9a9fab"
TEXT_SUBTLE = "#6b707b"

ACCENT = "#7c5cff"       # Tuple purple
ACCENT_HOVER = "#8f74ff"
DANGER = "#e5484d"
DANGER_HOVER = "#f06468"
SUCCESS = "#3dd68c"
WARN = "#f5a524"
ONLINE = SUCCESS
BUSY = WARN

FONT = '"Inter", "Noto Sans", "Cantarell", "Segoe UI", sans-serif'

QSS = f"""
* {{ font-family: {FONT}; font-size: 10pt; }}
QWidget {{ background-color: {BG}; color: {TEXT}; }}
QMainWindow, QDialog {{ background-color: {BG}; }}
QToolTip {{ background-color: {BG_ELEVATED}; color: {TEXT}; border: 1px solid {BORDER}; padding: 4px 6px; }}

QFrame#sidebar {{ background-color: {BG_PANEL}; border-right: 1px solid {BORDER}; }}
QFrame#bottomBar {{ background-color: {BG_PANEL}; border-top: 1px solid {BORDER}; }}
QFrame#callPanel {{ background-color: {BG_PANEL}; border-top: 1px solid {BORDER}; }}
QFrame#banner {{ background-color: {BG_ELEVATED}; border: 1px solid {BORDER}; border-radius: 8px; }}
QFrame#card {{ background-color: {BG_PANEL}; border: 1px solid {BORDER}; border-radius: 8px; }}
QFrame#card:hover {{ background-color: {BG_ELEVATED}; }}

QLabel#sectionTitle {{ color: {TEXT_SUBTLE}; font-size: 8pt; font-weight: 700; letter-spacing: 1px;
                       padding: 10px 12px 4px 12px; }}
QLabel#title {{ font-size: 13pt; font-weight: 600; }}
QLabel#muted {{ color: {TEXT_MUTED}; }}
QLabel#subtle {{ color: {TEXT_SUBTLE}; font-size: 9pt; }}
QLabel#warn {{ color: {WARN}; }}
QLabel#danger {{ color: {DANGER}; }}

QPushButton {{ background-color: {BG_ELEVATED}; color: {TEXT}; border: 1px solid {BORDER};
               border-radius: 6px; padding: 6px 12px; }}
QPushButton:hover {{ background-color: {BG_HOVER}; }}
QPushButton:pressed {{ background-color: {BORDER}; }}
QPushButton:disabled {{ color: {TEXT_SUBTLE}; background-color: {BG_PANEL}; }}
QPushButton:checked {{ background-color: {ACCENT}; border-color: {ACCENT}; color: white; }}
QPushButton[kind="primary"] {{ background-color: {ACCENT}; border-color: {ACCENT}; color: white; font-weight: 600; }}
QPushButton[kind="primary"]:hover {{ background-color: {ACCENT_HOVER}; }}
QPushButton[kind="danger"] {{ background-color: {DANGER}; border-color: {DANGER}; color: white; font-weight: 600; }}
QPushButton[kind="danger"]:hover {{ background-color: {DANGER_HOVER}; }}
QPushButton[kind="flat"] {{ background-color: transparent; border: none; padding: 4px 8px; }}
QPushButton[kind="flat"]:hover {{ background-color: {BG_HOVER}; }}
QPushButton[kind="nav"] {{ background-color: transparent; border: none; border-radius: 6px; text-align: left;
                           padding: 8px 12px; color: {TEXT_MUTED}; }}
QPushButton[kind="nav"]:hover {{ background-color: {BG_HOVER}; color: {TEXT}; }}
QPushButton[kind="nav"]:checked {{ background-color: {BG_ELEVATED}; color: {TEXT}; font-weight: 600; }}
QPushButton[kind="control"] {{ min-width: 64px; padding: 8px 10px; border-radius: 8px; }}
QPushButton[kind="control"][active="true"] {{ background-color: {DANGER}; border-color: {DANGER}; color: white; }}

QLineEdit {{ background-color: {BG_ELEVATED}; border: 1px solid {BORDER}; border-radius: 6px; padding: 6px 10px;
             selection-background-color: {ACCENT}; }}
QLineEdit:focus {{ border-color: {ACCENT}; }}
QTextEdit, QPlainTextEdit {{ background-color: {BG_ELEVATED}; border: 1px solid {BORDER}; border-radius: 6px; }}
QComboBox {{ background-color: {BG_ELEVATED}; border: 1px solid {BORDER}; border-radius: 6px; padding: 4px 8px; }}
QComboBox QAbstractItemView {{ background-color: {BG_ELEVATED}; selection-background-color: {ACCENT}; }}
QCheckBox {{ spacing: 8px; }}

QScrollArea {{ border: none; background: transparent; }}
QScrollArea > QWidget > QWidget {{ background: transparent; }}
QScrollBar:vertical {{ background: transparent; width: 8px; margin: 0; }}
QScrollBar::handle:vertical {{ background: {BORDER}; min-height: 24px; border-radius: 4px; }}
QScrollBar::handle:vertical:hover {{ background: {TEXT_SUBTLE}; }}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{ height: 0; }}

QMenu {{ background-color: {BG_ELEVATED}; border: 1px solid {BORDER}; padding: 4px 0; }}
QMenu::item {{ padding: 6px 20px; }}
QMenu::item:selected {{ background-color: {ACCENT}; color: white; }}
QMenu::item:disabled {{ color: {TEXT_SUBTLE}; }}
QMenu::separator {{ height: 1px; background: {BORDER}; margin: 4px 8px; }}

QListWidget {{ background: transparent; border: none; }}
QTabWidget::pane {{ border: 1px solid {BORDER}; border-radius: 6px; }}
QTabBar::tab {{ background: {BG_PANEL}; padding: 6px 12px; border: 1px solid {BORDER}; border-bottom: none; }}
QTabBar::tab:selected {{ background: {BG_ELEVATED}; }}
"""


def apply_theme(app):
    app.setStyle("Fusion")
    app.setStyleSheet(QSS)
