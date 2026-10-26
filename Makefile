.PHONY: default build release test dev restart install uninstall help

APP_ID := dev.joakimk.TupleUi
JOBS ?= 2

default: build

build:
	nice -n 10 cargo build -j $(JOBS)

release:
	nice -n 10 cargo build --release -j $(JOBS)

test:
	nice -n 10 cargo test -j $(JOBS)

dev: build
	cargo run -q

restart: release
	-pkill -x tuple-ui; sleep 1
	nohup $$(cargo metadata --format-version 1 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/release/tuple-ui --tray > /dev/null 2>&1 &

install: release
	mkdir -p ~/.local/bin ~/.local/share/applications
	install -m 755 $$(cargo metadata --format-version 1 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/release/tuple-ui ~/.local/bin/tuple-ui
	install -m 644 data/$(APP_ID).desktop ~/.local/share/applications/$(APP_ID).desktop
	update-desktop-database ~/.local/share/applications 2>/dev/null || true

uninstall:
	rm -f ~/.local/bin/tuple-ui ~/.local/share/applications/$(APP_ID).desktop
	update-desktop-database ~/.local/share/applications 2>/dev/null || true

help:
	@echo "make          - debug build (JOBS=$(JOBS) parallel jobs, low priority)"
	@echo "make test     - run the test suite"
	@echo "make dev      - run the app from source"
	@echo "make release  - optimized build"
	@echo "make install  - install binary + desktop entry into ~/.local"
	@echo "make restart  - restart a background instance (tray mode)"
