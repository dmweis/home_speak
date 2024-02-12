TARGET_URL ?= speakerpi
TARGET_USERNAME ?= pi
TARGET_HOST ?= $(TARGET_USERNAME)@$(TARGET_URL)
REMOTE_DIRECTORY ?= /home/pi
ARM_BUILD_PATH ?= target/debian/home_speak_*.deb

VERSION_TAG = $(shell cargo get version)

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

.PHONY: install-dependencies
install-dependencies:
	cargo install cargo-deb cargo-get

.PHONY: build-docker
build-docker:
	rm -rf docker_out
	mkdir docker_out
	DOCKER_BUILDKIT=1 docker build --tag hopper-builder --file Dockerfile --output type=local,dest=docker_out .

.PHONY: push-docker-built
push-docker-built: build-docker
	rsync -avz --delete docker_out/* $(TARGET_HOST):/home/$(TARGET_USERNAME)/home-speak

.PHONY: deploy-with-ez-cd
deploy-with-ez-cd: build-docker
	ez-cd-cli -f docker_out/home_speak_server.deb -d speakerpi
