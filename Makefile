TARGET_URL ?= speakerpi.local
TARGET_HOST ?= pi@$(TARGET_URL)
REMOTE_DIRECTORY ?= /home/pi
ARM_BUILD_PATH ?= target/debian/home_speak_*.deb

VERSION_TAG = $(shell cargo get version)

MENDER_ARTIFACT_NAME ?= home-speak-$(VERSION_TAG)
MENDER_ARTIFACT_FILE ?= $(MENDER_ARTIFACT_NAME).mender
MENDER_ARTIFACT_OUTPUT_PATH := target/mender

HOSTNAME = $(shell hostname).local

.PHONY: build
build:
	cargo build --release --bin home_speak_server
	cargo deb --no-build

.PHONE: install
install: build
	sudo dpkg -i $(ARM_BUILD_PATH)

.PHONY: deploy
deploy: build
	@echo "Sending $(ARM_BUILD_PATH) to $(TARGET_HOST):$(REMOTE_DIRECTORY)"
	rsync -avz --delete $(ARM_BUILD_PATH) $(TARGET_HOST):$(REMOTE_DIRECTORY)

.PHONY: debug
debug:
	cargo run

.PHONY: build-artifact
build-artifact: build
	mkdir -p $(MENDER_ARTIFACT_OUTPUT_PATH)
	rm -f $(MENDER_ARTIFACT_OUTPUT_PATH)/*
	mender-artifact write module-image --type deb \
		--artifact-name $(MENDER_ARTIFACT_NAME) \
		--device-type raspberrypi4 \
		--device-type raspberrypi3 \
		--output-path $(MENDER_ARTIFACT_OUTPUT_PATH)/$(MENDER_ARTIFACT_FILE) \
		--file $(ARM_BUILD_PATH)

.PHONY: publish-mender-artifact
publish-mender-artifact: build-artifact
	mender-cli artifacts --server https://hosted.mender.io upload $(MENDER_ARTIFACT_OUTPUT_PATH)/$(MENDER_ARTIFACT_FILE)

.PHONY: serve-artifact
serve-artifact: build-artifact
	@echo http://$(HOSTNAME):8000
	python3 -m http.server 8000 --directory $(MENDER_ARTIFACT_OUTPUT_PATH)

.PHONY: install-dependencies
install-dependencies:
	cargo install cargo-deb cargo-get
