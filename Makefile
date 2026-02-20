.PHONY: all build download

VERSION := v2.2
# Detect OS
ifeq ($(OS),Windows_NT)
    DETECTED_OS := windows
    GODOTPCK := godotpcktool.exe
else
    UNAME_S := $(shell uname -s)
    ifeq ($(UNAME_S),Linux)
        DETECTED_OS := linux
        GODOTPCK := godotpcktool
    else ifeq ($(UNAME_S),Darwin)
        DETECTED_OS := macos
        GODOTPCK := godotpcktool
    else
        DETECTED_OS := linux
        GODOTPCK := godotpcktool
    endif
endif

RELEASE_DIR := target/release
GODOTPCK_URL := https://github.com/Sekai-Engine/GodotPckTool/releases/download/$(VERSION)/$(GODOTPCK)

all: build download

build:
	cargo build --release

download:
	@mkdir -p $(RELEASE_DIR)
	@echo "Downloading $(GODOTPCK) for $(DETECTED_OS)..."
	curl -L -o $(RELEASE_DIR)/$(GODOTPCK) $(GODOTPCK_URL)
	@chmod +x $(RELEASE_DIR)/$(GODOTPCK)

clean:
	cargo clean
	rm -f $(RELEASE_DIR)/$(GODOTPCK)
