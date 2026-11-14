## Vaultwarden Desktop — development and build commands.
## Run `make help` to list targets. Requires: cargo, npm, cargo-tauri (installed
## via `make setup` or `make dev-setup`).

SHELL := /bin/bash
export PATH := $(HOME)/.cargo/bin:$(PATH)
UI_DIR := ui
SUDO ?= $(if $(filter 0,$(shell id -u 2>/dev/null)),,sudo)

## --- OS & Architecture Detection ---

ifeq ($(OS),Windows_NT)
    DETECTED_OS := Windows
    HOST_ARCH   := $(or $(PROCESSOR_ARCHITEW6432),$(PROCESSOR_ARCHITECTURE),x86_64)
else
    UNAME_S     := $(shell uname -s 2>/dev/null || echo Linux)
    HOST_ARCH   := $(shell uname -m 2>/dev/null || echo x86_64)
    ifneq (,$(findstring MINGW,$(UNAME_S)))
        DETECTED_OS := Windows
    else ifneq (,$(findstring MSYS,$(UNAME_S)))
        DETECTED_OS := Windows
    else ifneq (,$(findstring CYGWIN,$(UNAME_S)))
        DETECTED_OS := Windows
    else ifeq ($(UNAME_S),Darwin)
        DETECTED_OS := Darwin
    else
        DETECTED_OS := Linux
    endif
endif

OS_TYPE ?= $(DETECTED_OS)

# Normalize ARCH
ifeq ($(HOST_ARCH),AMD64)
    ARCH ?= x86_64
else ifeq ($(HOST_ARCH),arm64)
    ARCH ?= aarch64
else ifeq ($(HOST_ARCH),ARM64)
    ARCH ?= aarch64
else
    ARCH ?= $(HOST_ARCH)
endif

ifeq ($(OS_TYPE),Windows)
    BIN_NAME := vaultwarden-desktop.exe
    WIN_INSTALL_DIR ?= $(if $(LOCALAPPDATA),$(LOCALAPPDATA)/Programs/vaultwarden-desktop,$(USERPROFILE)/AppData/Local/Programs/vaultwarden-desktop)
else
    BIN_NAME := vaultwarden-desktop
    PREFIX ?= $(if $(filter 0,$(shell id -u 2>/dev/null)),/usr/local,$(HOME)/.local)
    BINDIR ?= $(PREFIX)/bin
    DATADIR ?= $(PREFIX)/share
    DESKTOP_DIR ?= $(shell xdg-user-dir DESKTOP 2>/dev/null || echo $(HOME)/Desktop)
    MAC_APP_DIR ?= $(if $(filter 0,$(shell id -u 2>/dev/null)),/Applications,$(HOME)/Applications)
    APP_BUNDLE_NAME := Vaultwarden Desktop.app
endif

ifneq ($(TARGET),)
    CARGO_TARGET_FLAG := --target $(TARGET)
    BIN_PATH := target/$(TARGET)/release/$(BIN_NAME)
else
    CARGO_TARGET_FLAG :=
    BIN_PATH := target/release/$(BIN_NAME)
endif

.PHONY: help dev-setup dev-sys-deps dev-sys-deps-linux dev-sys-deps-macos dev-sys-deps-windows dev-rust dev-node-check \
        setup check test test-core test-e2e fmt lint \
        ui-install ui-build ui-dev \
        build build-release run bundle \
        install install-linux install-windows install-macos install-darwin \
        uninstall uninstall-linux uninstall-windows uninstall-macos uninstall-darwin \
        desktop desktop-linux desktop-windows desktop-entry desktop-shortcut \
        clean clean-target clean-ui distclean

help: ## Show this help
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

## --- Setup & Development Environment ---

dev-setup: dev-node-check dev-sys-deps dev-rust setup ## Prepare complete dev environment (OS libraries, Rust toolchain, Tauri CLI, npm deps)
	@echo "Development setup complete! Run 'make check' or 'make run' to get started."

dev-node-check:
	@if ! command -v npm >/dev/null 2>&1; then \
		echo "Error: 'npm' is not installed or not in PATH."; \
		echo "Please install Node.js 18+ (https://nodejs.org) before proceeding."; \
		exit 1; \
	fi

dev-sys-deps:
ifeq ($(OS_TYPE),Windows)
	@$(MAKE) dev-sys-deps-windows
else ifeq ($(OS_TYPE),Darwin)
	@$(MAKE) dev-sys-deps-macos
else
	@$(MAKE) dev-sys-deps-linux
endif

dev-sys-deps-linux:
	@if command -v apt-get >/dev/null 2>&1; then \
		echo "Installing Linux dependencies via apt-get..."; \
		$(SUDO) apt-get update && $(SUDO) apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config; \
	elif command -v dnf >/dev/null 2>&1; then \
		echo "Installing Linux dependencies via dnf..."; \
		$(SUDO) dnf install -y gcc-c++ webkit2gtk4.1-devel openssl-devel libappindicator-gtk3-devel librsvg2-devel libxdo-devel pkgconf-pkg-config; \
	elif command -v pacman >/dev/null 2>&1; then \
		echo "Installing Linux dependencies via pacman..."; \
		$(SUDO) pacman -S --needed --noconfirm base-devel webkit2gtk-4.1 openssl libappindicator-gtk3 librsvg; \
	elif command -v zypper >/dev/null 2>&1; then \
		echo "Installing Linux dependencies via zypper..."; \
		$(SUDO) zypper install -y patterns-devel-base-devel_basis webkit2gtk4-devel libopenssl-devel libappindicator3-devel librsvg-devel pkg-config; \
	else \
		echo "Notice: Unrecognized Linux package manager. Please install required libraries manually (see README.md)."; \
	fi

dev-sys-deps-macos:
	@if ! xcode-select -p >/dev/null 2>&1; then \
		echo "Prompting to install Xcode Command Line Tools..."; \
		xcode-select --install; \
	else \
		echo "Xcode Command Line Tools already installed."; \
	fi

dev-sys-deps-windows:
	@echo "On Windows, please ensure Microsoft C++ Build Tools or Visual Studio with C++ workload is installed."

dev-rust:
	@if command -v rustup >/dev/null 2>&1; then \
		echo "Ensuring stable Rust toolchain and components..."; \
		rustup default stable; \
		rustup component add rustfmt clippy; \
	elif command -v cargo >/dev/null 2>&1; then \
		echo "Cargo detected without rustup; skipping rustup component configuration."; \
	else \
		echo "Rust not detected. Installing via rustup..."; \
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable; \
		export PATH="$$HOME/.cargo/bin:$$PATH"; \
		rustup component add rustfmt clippy 2>/dev/null || true; \
	fi

setup: ## Install cargo-tauri CLI and frontend npm dependencies
	cargo install tauri-cli --version "^2" --locked
	$(MAKE) ui-install

ui-install: ## Install frontend npm dependencies
	cd $(UI_DIR) && npm install

## --- Verification (run before every commit) ---

check: ## cargo check across the whole workspace (fast compile-only verification)
	cargo check --workspace

test: test-core test-e2e ## Run all Rust tests (core unit tests + e2e IPC flow)

test-core: ## Run vaultwarden-core unit tests (crypto, storage, sync, api)
	cargo test -p vaultwarden-core

test-e2e: ## Run the end-to-end IPC test (tauri::test::mock_builder)
	cargo test -p vaultwarden-desktop --test e2e_vault_flow

fmt: ## Format all Rust code
	cargo fmt --all

lint: ## Run clippy across the workspace
	cargo clippy --workspace --all-targets -- -D warnings

## --- Frontend ---

ui-build: ## Build the Svelte frontend into ui/dist (used by cargo tauri build)
	cd $(UI_DIR) && npm run build

ui-dev: ## Run the Vite dev server standalone (frontend only, no Tauri window)
	cd $(UI_DIR) && npm run dev

## --- Desktop app ---

build: ## Build the Rust workspace in debug mode — compile check ONLY, do not run this binary directly
	cargo build --workspace

build-release: ui-build ## Build the release binary with the frontend embedded (target/release/vaultwarden-desktop)
	cargo build --release -p vaultwarden-desktop --features custom-protocol $(CARGO_TARGET_FLAG)

run: build-release ## Build release and launch the binary directly — the real "double-click" experience, no dev server involved
	./$(BIN_PATH)

bundle: ## Produce a distributable package (.AppImage/.deb on Linux, .app/.dmg on macOS, .msi/.exe on Windows)
	cargo tauri build

## --- Installation & Shortcuts ---

desktop-linux: ## Build the Linux .desktop entry file
	@mkdir -p target
	@echo "[Desktop Entry]" > target/vaultwarden-desktop.desktop
	@echo "Type=Application" >> target/vaultwarden-desktop.desktop
	@echo "Name=Vaultwarden Desktop" >> target/vaultwarden-desktop.desktop
	@echo "GenericName=Password Manager" >> target/vaultwarden-desktop.desktop
	@echo "Comment=Vaultwarden and Bitwarden desktop password manager" >> target/vaultwarden-desktop.desktop
	@echo "Exec=$(BINDIR)/$(BIN_NAME) %U" >> target/vaultwarden-desktop.desktop
	@echo "Icon=vaultwarden-desktop" >> target/vaultwarden-desktop.desktop
	@echo "Terminal=false" >> target/vaultwarden-desktop.desktop
	@echo "Categories=Utility;Security;" >> target/vaultwarden-desktop.desktop
	@echo "Keywords=vaultwarden;bitwarden;password;credentials;vault;security;" >> target/vaultwarden-desktop.desktop
	@echo "StartupWMClass=vaultwarden-desktop" >> target/vaultwarden-desktop.desktop
	@echo "MimeType=x-scheme-handler/bitwarden;x-scheme-handler/vaultwarden;" >> target/vaultwarden-desktop.desktop
	@echo "Generated target/vaultwarden-desktop.desktop"

desktop-windows: ## Create or generate the Windows desktop shortcut script with app logo
	@mkdir -p target
	@echo '$$customDir = "$(WIN_INSTALL_DIR)".Replace("/", "\");' > target/create-shortcut.ps1
	@echo '$$installDir = if ($$customDir) { $$customDir } elseif ($$env:LOCALAPPDATA) { "$$env:LOCALAPPDATA\Programs\vaultwarden-desktop" } else { "$$env:USERPROFILE\AppData\Local\Programs\vaultwarden-desktop" };' >> target/create-shortcut.ps1
	@echo '$$ws = New-Object -ComObject WScript.Shell;' >> target/create-shortcut.ps1
	@echo '$$desk = [Environment]::GetFolderPath("Desktop");' >> target/create-shortcut.ps1
	@echo '$$target = Join-Path $$installDir "vaultwarden-desktop.exe";' >> target/create-shortcut.ps1
	@echo '$$icon = Join-Path $$installDir "vaultwarden.ico";' >> target/create-shortcut.ps1
	@echo 'if (Test-Path $$desk) {' >> target/create-shortcut.ps1
	@echo '    $$s = $$ws.CreateShortcut((Join-Path $$desk "Vaultwarden Desktop.lnk"));' >> target/create-shortcut.ps1
	@echo '    $$s.TargetPath = $$target;' >> target/create-shortcut.ps1
	@echo '    $$s.WorkingDirectory = $$installDir;' >> target/create-shortcut.ps1
	@echo '    $$s.IconLocation = "$$icon,0";' >> target/create-shortcut.ps1
	@echo '    $$s.Description = "Vaultwarden Desktop Password Manager";' >> target/create-shortcut.ps1
	@echo '    $$s.Save();' >> target/create-shortcut.ps1
	@echo '}' >> target/create-shortcut.ps1
	@echo '$$progs = [Environment]::GetFolderPath("Programs");' >> target/create-shortcut.ps1
	@echo 'if (Test-Path $$progs) {' >> target/create-shortcut.ps1
	@echo '    $$s2 = $$ws.CreateShortcut((Join-Path $$progs "Vaultwarden Desktop.lnk"));' >> target/create-shortcut.ps1
	@echo '    $$s2.TargetPath = $$target;' >> target/create-shortcut.ps1
	@echo '    $$s2.WorkingDirectory = $$installDir;' >> target/create-shortcut.ps1
	@echo '    $$s2.IconLocation = "$$icon,0";' >> target/create-shortcut.ps1
	@echo '    $$s2.Description = "Vaultwarden Desktop Password Manager";' >> target/create-shortcut.ps1
	@echo '    $$s2.Save();' >> target/create-shortcut.ps1
	@echo '}' >> target/create-shortcut.ps1
	@if command -v powershell.exe >/dev/null 2>&1; then \
		powershell.exe -NoProfile -ExecutionPolicy Bypass -File target/create-shortcut.ps1 && echo "Created Windows desktop shortcut"; \
	elif command -v powershell >/dev/null 2>&1; then \
		powershell -NoProfile -Command "target/create-shortcut.ps1" && echo "Created Windows desktop shortcut"; \
	else \
		echo "Generated target/create-shortcut.ps1 (run with powershell on Windows to create shortcut)"; \
	fi

desktop: desktop-linux desktop-windows ## Build both Linux .desktop and Windows shortcut script
desktop-entry: desktop-linux ## Alias for desktop-linux
desktop-shortcut: desktop-windows ## Alias for desktop-windows

install-linux: build-release desktop-linux ## Install binary, icons, and .desktop on Linux
	@echo "Installing vaultwarden-desktop for Linux ($(ARCH)) into $(PREFIX)..."
	install -d "$(DESTDIR)$(BINDIR)"
	install -m 755 "$(BIN_PATH)" "$(DESTDIR)$(BINDIR)/$(BIN_NAME)"
	install -d "$(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps"
	install -m 644 vaultwarden.svg "$(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/vaultwarden-desktop.svg"
	install -d "$(DESTDIR)$(DATADIR)/icons/hicolor/512x512/apps"
	install -m 644 vaultwarden.png "$(DESTDIR)$(DATADIR)/icons/hicolor/512x512/apps/vaultwarden-desktop.png"
	install -d "$(DESTDIR)$(DATADIR)/icons/hicolor/128x128/apps"
	install -m 644 src-tauri/icons/128x128.png "$(DESTDIR)$(DATADIR)/icons/hicolor/128x128/apps/vaultwarden-desktop.png"
	install -d "$(DESTDIR)$(DATADIR)/icons/hicolor/32x32/apps"
	install -m 644 src-tauri/icons/32x32.png "$(DESTDIR)$(DATADIR)/icons/hicolor/32x32/apps/vaultwarden-desktop.png"
	install -d "$(DESTDIR)$(DATADIR)/pixmaps"
	install -m 644 vaultwarden.png "$(DESTDIR)$(DATADIR)/pixmaps/vaultwarden-desktop.png"
	install -d "$(DESTDIR)$(DATADIR)/applications"
	install -m 644 target/vaultwarden-desktop.desktop "$(DESTDIR)$(DATADIR)/applications/vaultwarden-desktop.desktop"
	@if [ -z "$(DESTDIR)" ] && [ -d "$(DESKTOP_DIR)" ]; then \
		install -m 755 target/vaultwarden-desktop.desktop "$(DESKTOP_DIR)/vaultwarden-desktop.desktop"; \
		command -v gio >/dev/null 2>&1 && gio set "$(DESKTOP_DIR)/vaultwarden-desktop.desktop" metadata::trusted true 2>/dev/null || true; \
		echo "Installed desktop shortcut to $(DESKTOP_DIR)/vaultwarden-desktop.desktop"; \
	fi
	@command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$(DESTDIR)$(DATADIR)/applications" 2>/dev/null || true
	@command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f -q "$(DESTDIR)$(DATADIR)/icons/hicolor" 2>/dev/null || true
	@echo "Installation complete: $(BINDIR)/$(BIN_NAME)"

install-windows: build-release desktop-windows ## Install binary, icon, and desktop shortcut on Windows
	@echo "Installing vaultwarden-desktop for Windows ($(ARCH)) into $(WIN_INSTALL_DIR)..."
	@if command -v powershell.exe >/dev/null 2>&1; then \
		powershell.exe -NoProfile -Command "New-Item -ItemType Directory -Force -Path '$(WIN_INSTALL_DIR)' | Out-Null; Copy-Item -Force '$(BIN_PATH)' '$(WIN_INSTALL_DIR)/$(BIN_NAME)'; Copy-Item -Force 'vaultwarden.ico' '$(WIN_INSTALL_DIR)/vaultwarden.ico'"; \
	elif command -v powershell >/dev/null 2>&1; then \
		powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path '$(WIN_INSTALL_DIR)' | Out-Null; Copy-Item -Force '$(BIN_PATH)' '$(WIN_INSTALL_DIR)/$(BIN_NAME)'; Copy-Item -Force 'vaultwarden.ico' '$(WIN_INSTALL_DIR)/vaultwarden.ico'"; \
	else \
		mkdir -p "$(WIN_INSTALL_DIR)" && cp -f "$(BIN_PATH)" "$(WIN_INSTALL_DIR)/$(BIN_NAME)" && cp -f vaultwarden.ico "$(WIN_INSTALL_DIR)/vaultwarden.ico"; \
	fi
	@echo "Installation complete: $(WIN_INSTALL_DIR)/$(BIN_NAME)"

install-macos: build-release ## Install binary and .app bundle on macOS
	@echo "Installing vaultwarden-desktop for macOS ($(ARCH))..."
	install -d "$(DESTDIR)$(BINDIR)"
	install -m 755 "$(BIN_PATH)" "$(DESTDIR)$(BINDIR)/$(BIN_NAME)"
	@mkdir -p "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/MacOS"
	@mkdir -p "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Resources"
	cp -f "$(BIN_PATH)" "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/MacOS/$(BIN_NAME)"
	chmod +x "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/MacOS/$(BIN_NAME)"
	cp -f src-tauri/icons/icon.icns "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Resources/icon.icns"
	@echo '<?xml version="1.0" encoding="UTF-8"?>' > "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '<plist version="1.0"><dict>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '  <key>CFBundleExecutable</key><string>$(BIN_NAME)</string>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '  <key>CFBundleIdentifier</key><string>com.vaultwarden.desktop</string>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '  <key>CFBundleName</key><string>Vaultwarden Desktop</string>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '  <key>CFBundlePackageType</key><string>APPL</string>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '  <key>CFBundleIconFile</key><string>icon.icns</string>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '  <key>CFBundleShortVersionString</key><string>0.1.0</string>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '  <key>LSMinimumSystemVersion</key><string>10.15</string>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '  <key>NSHighResolutionCapable</key><true/>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo '</dict></plist>' >> "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)/Contents/Info.plist"
	@echo "Installation complete: $(BINDIR)/$(BIN_NAME)"
	@echo "Application bundle installed: $(MAC_APP_DIR)/$(APP_BUNDLE_NAME)"

install-darwin: install-macos ## Alias for install-macos

install: ## Build and install based on detected OS and arch
ifeq ($(OS_TYPE),Windows)
	@$(MAKE) install-windows
else ifeq ($(OS_TYPE),Darwin)
	@$(MAKE) install-macos
else
	@$(MAKE) install-linux
endif

uninstall: ## Remove installed binary, icons, and shortcuts based on OS
ifeq ($(OS_TYPE),Windows)
	@$(MAKE) uninstall-windows
else ifeq ($(OS_TYPE),Darwin)
	@$(MAKE) uninstall-macos
else
	@$(MAKE) uninstall-linux
endif

uninstall-macos: ## Remove installed binary and .app bundle on macOS
	rm -f "$(DESTDIR)$(BINDIR)/$(BIN_NAME)"
	rm -rf "$(DESTDIR)$(MAC_APP_DIR)/$(APP_BUNDLE_NAME)"
	@echo "Uninstalled vaultwarden-desktop from macOS"

uninstall-darwin: uninstall-macos ## Alias for uninstall-macos

uninstall-linux: ## Remove installed binary, icons, and .desktop on Linux
	rm -f "$(DESTDIR)$(BINDIR)/$(BIN_NAME)"
	rm -f "$(DESTDIR)$(DATADIR)/applications/vaultwarden-desktop.desktop"
	rm -f "$(DESKTOP_DIR)/vaultwarden-desktop.desktop"
	rm -f "$(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/vaultwarden-desktop.svg"
	rm -f "$(DESTDIR)$(DATADIR)/icons/hicolor/512x512/apps/vaultwarden-desktop.png"
	rm -f "$(DESTDIR)$(DATADIR)/icons/hicolor/128x128/apps/vaultwarden-desktop.png"
	rm -f "$(DESTDIR)$(DATADIR)/icons/hicolor/32x32/apps/vaultwarden-desktop.png"
	rm -f "$(DESTDIR)$(DATADIR)/pixmaps/vaultwarden-desktop.png"
	@command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$(DESTDIR)$(DATADIR)/applications" 2>/dev/null || true
	@command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f -q "$(DESTDIR)$(DATADIR)/icons/hicolor" 2>/dev/null || true
	@echo "Uninstalled vaultwarden-desktop"

uninstall-windows: ## Remove installed binary, icon, and shortcuts on Windows
	@if command -v powershell.exe >/dev/null 2>&1; then \
		powershell.exe -NoProfile -Command "Remove-Item -Recurse -Force '$(WIN_INSTALL_DIR)' -ErrorAction SilentlyContinue; Remove-Item -Force (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Vaultwarden Desktop.lnk') -ErrorAction SilentlyContinue; Remove-Item -Force (Join-Path ([Environment]::GetFolderPath('Programs')) 'Vaultwarden Desktop.lnk') -ErrorAction SilentlyContinue"; \
	elif command -v powershell >/dev/null 2>&1; then \
		powershell -NoProfile -Command "Remove-Item -Recurse -Force '$(WIN_INSTALL_DIR)' -ErrorAction SilentlyContinue; Remove-Item -Force (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Vaultwarden Desktop.lnk') -ErrorAction SilentlyContinue; Remove-Item -Force (Join-Path ([Environment]::GetFolderPath('Programs')) 'Vaultwarden Desktop.lnk') -ErrorAction SilentlyContinue"; \
	else \
		rm -rf "$(WIN_INSTALL_DIR)"; \
	fi
	@echo "Uninstalled vaultwarden-desktop from Windows"

## --- Cleanup ---

clean-target: ## Remove Rust build artifacts (target/) — safe, cargo regenerates it
	cargo clean

clean-ui: ## Remove frontend build artifacts (ui/dist, ui/node_modules)
	rm -rf $(UI_DIR)/dist $(UI_DIR)/node_modules

clean: clean-target clean-ui ## Remove all build artifacts (Rust + frontend)

distclean: clean ## Remove build artifacts and local vault data
	@echo "Note: this does NOT touch ~/.config/vaultwarden-desktop."
