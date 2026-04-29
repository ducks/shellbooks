.PHONY: help version-bump release build test clippy clean

# Extract version from command line if passed as argument
# Supports: make release 20260429.0 OR make release VERSION=20260429.0
ifeq ($(VERSION),)
  VERSION := $(wordlist 2,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS))
  $(eval $(VERSION):;@:)
endif

help:
	@echo "Shellbooks Makefile"
	@echo ""
	@echo "Usage:"
	@echo "  make version-bump 20260429.1       - Bump version in Cargo.toml and commit"
	@echo "  make release 20260429.1            - Bump version and push tag to trigger release"
	@echo "  make build                         - Build release binary"
	@echo "  make test                          - Run tests"
	@echo "  make clippy                        - Run clippy"
	@echo "  make clean                         - Clean build artifacts"

version-bump:
ifeq ($(VERSION),)
	$(error No version specified. Use: make version-bump 20260429.1)
endif
	$(eval VERSION_CLEAN := $(patsubst v%,%,$(VERSION)))
	@echo "Creating release branch for version $(VERSION_CLEAN)..."
	@git checkout -b release/v$(VERSION_CLEAN)
	@echo "Bumping version to $(VERSION_CLEAN)..."
	@sed -i 's/^version = .*/version = "$(VERSION_CLEAN)"/' Cargo.toml
	@git add Cargo.toml
	@git commit -m "chore: bump version to $(VERSION_CLEAN)"

release: version-bump
	$(eval VERSION_CLEAN := $(patsubst v%,%,$(VERSION)))
	@echo "Merging into main..."
	@git checkout main
	@git merge --no-ff release/v$(VERSION_CLEAN)
	@git tag -a v$(VERSION_CLEAN) -m "Release v$(VERSION_CLEAN)"
	@git push origin main
	@git push origin v$(VERSION_CLEAN)

build:
	cargo build --release

test:
	cargo test

clippy:
	cargo clippy -- -D warnings

clean:
	cargo clean
