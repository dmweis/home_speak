TARGET_URL ?= speakerpi.local
TARGET_HOST ?= pi@$(TARGET_URL)
REMOTE_DIRECTORY ?= /home/pi
ARM_BUILD_PATH ?= target/debian/home_speak_*.deb


.PHONY: build
build:
	cargo build --release --bin home_speak_server
	cargo deb --no-build

.PHONE: install
install: build
	sudo dpkg -i target/debian/home_speak_*.deb

.PHONY: deploy
deploy: build
	@echo "Sending $(ARM_BUILD_PATH) to $(TARGET_HOST):$(REMOTE_DIRECTORY)"
	rsync -avz --delete $(ARM_BUILD_PATH) $(TARGET_HOST):$(REMOTE_DIRECTORY)

.PHONY: debug
debug:
	cargo run
