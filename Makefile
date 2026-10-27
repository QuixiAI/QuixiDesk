# QuixiDesk — native Rust desktop workspace (tao + wry / WKWebView)
#
#   make            # release build of the whole workspace
#   make app        # build + package dist/QuixiDesk.app
#   make run        # run the desktop app straight from cargo
#   make test       # cargo test --workspace
#   make install    # copy dist/QuixiDesk.app to /Applications
#
SHELL := /bin/bash
ROOT  := $(shell cd "$(dir $(lastword $(MAKEFILE_LIST)))" && pwd)
CARGO := $(shell command -v cargo 2>/dev/null || echo $(HOME)/.cargo/bin/cargo)
APP   := $(ROOT)/dist/QuixiDesk.app

.PHONY: all app run test check fmt clippy clean install uninstall open

all:
	$(CARGO) build --release --workspace

app:
	./scripts/package-macos-app.sh release

run:
	$(CARGO) run --release -p quixi-app

test:
	$(CARGO) test --workspace

check:
	$(CARGO) check --workspace --all-targets

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

open:
	open "$(APP)"

install: app
	rm -rf /Applications/QuixiDesk.app
	cp -R "$(APP)" /Applications/QuixiDesk.app
	@echo "Installed /Applications/QuixiDesk.app"

uninstall:
	rm -rf /Applications/QuixiDesk.app

clean:
	$(CARGO) clean
	rm -rf "$(ROOT)/dist"
