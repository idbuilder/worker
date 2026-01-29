.PHONY: build test lint build-docker clean check fmt typos

# Project configuration
PROJECT_NAME := idbuilder-worker
DOCKER_IMAGE := $(PROJECT_NAME)
DOCKER_TAG := latest

# Build the release binary
build:
	cargo build --release

# Run all tests
test:
	cargo test

# Run clippy, check formatting, and check typos
lint:
	cargo clippy
	cargo fmt --check
	typos

# Build Docker image
build-docker:
	docker build -t $(DOCKER_IMAGE):$(DOCKER_TAG) .

# Clean build artifacts
clean:
	cargo clean

# Quick check without building
check:
	cargo check

# Format code
fmt:
	cargo fmt

# Check for typos in source code
typos:
	typos
