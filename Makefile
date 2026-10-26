.PHONY: default build release-build test dev restart install uninstall dist release help

APP_ID := dev.joakimk.TupleUi
JOBS ?= 2
VERSION := $(shell grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
TARGET_DIR := $(shell cargo metadata --format-version 1 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
DIST_NAME := tuple-ui-x86_64-linux
DIST_DIR := dist/$(DIST_NAME)
TARBALL := dist/$(DIST_NAME).tar.gz

default: build

build:
	nice -n 10 cargo build -j $(JOBS)

release-build:
	nice -n 10 cargo build --release -j $(JOBS)

test:
	nice -n 10 cargo test -j $(JOBS)

dev: build
	cargo run -q

restart: release-build
	-pkill -x tuple-ui; sleep 1
	nohup $(TARGET_DIR)/release/tuple-ui --tray > /dev/null 2>&1 &

install: release-build
	mkdir -p ~/.local/bin ~/.local/share/applications
	install -m 755 $(TARGET_DIR)/release/tuple-ui ~/.local/bin/tuple-ui
	install -m 644 data/$(APP_ID).desktop ~/.local/share/applications/$(APP_ID).desktop
	update-desktop-database ~/.local/share/applications 2>/dev/null || true

uninstall:
	rm -f ~/.local/bin/tuple-ui ~/.local/share/applications/$(APP_ID).desktop
	update-desktop-database ~/.local/share/applications 2>/dev/null || true

# Build the binary tarball that releases ship: binary + desktop entry + install.sh.
dist: release-build
	rm -rf dist
	mkdir -p $(DIST_DIR)
	install -m 755 $(TARGET_DIR)/release/tuple-ui $(DIST_DIR)/tuple-ui
	strip $(DIST_DIR)/tuple-ui
	install -m 644 data/$(APP_ID).desktop $(DIST_DIR)/
	install -m 755 scripts/install.sh $(DIST_DIR)/install.sh
	install -m 644 README.md LICENSE $(DIST_DIR)/
	tar -C dist -czf $(TARBALL) $(DIST_NAME)
	@echo "built $(TARBALL) (version $(VERSION))"

# Tag v$(VERSION), push it and publish a GitHub release with the tarball (needs gh auth).
release: test dist
	@git diff --quiet && git diff --cached --quiet || { echo "commit your changes first"; exit 1; }
	@git rev-parse "v$(VERSION)" >/dev/null 2>&1 || git tag -a "v$(VERSION)" -m "tuple-ui v$(VERSION)"
	git push origin HEAD "v$(VERSION)"
	gh release create "v$(VERSION)" $(TARBALL) --title "tuple-ui v$(VERSION)" --generate-notes

help:
	@echo "make          - debug build (JOBS=$(JOBS) parallel jobs, low priority)"
	@echo "make test     - run the test suite"
	@echo "make dev      - run the app from source"
	@echo "make install  - release build installed into ~/.local"
	@echo "make dist     - build the release tarball in dist/"
	@echo "make release  - tag v$(VERSION), push, and publish a GitHub release"
	@echo "make restart  - restart a background instance (tray mode)"
