.PHONY: default build test dev install uninstall restart help

PY := .venv/bin/python
export QT_QPA_PLATFORM ?= wayland;xcb

default: build

build: .venv/.ready

.venv/.ready: pyproject.toml
	python3 -m venv --system-site-packages .venv
	$(PY) -m pip install --quiet -e ".[test]"
	touch .venv/.ready

test: build
	QT_QPA_PLATFORM=offscreen $(PY) -m pytest -q

dev: build
	$(PY) -m tuple_ui

restart: build
	-pkill -f "python.*(tuple_ui|tuple-ui)" ; sleep 1
	nohup $(PY) -m tuple_ui > /dev/null 2>&1 &

install: build
	$(PY) -m pip install --quiet .
	mkdir -p ~/.local/share/applications ~/.local/bin
	cp tuple-ui.desktop ~/.local/share/applications/
	ln -sf $(CURDIR)/.venv/bin/tuple-ui ~/.local/bin/tuple-ui
	update-desktop-database ~/.local/share/applications 2>/dev/null || true

uninstall:
	rm -f ~/.local/share/applications/tuple-ui.desktop ~/.local/bin/tuple-ui
	update-desktop-database ~/.local/share/applications 2>/dev/null || true

help:
	@echo "make          - set up the virtualenv (build)"
	@echo "make test     - run the test suite"
	@echo "make dev      - run the app from source"
	@echo "make restart  - restart a background instance"
	@echo "make install  - install launcher + desktop entry"
