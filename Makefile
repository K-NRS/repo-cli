.PHONY: build release-build link unlink clean changelog release

# AI CLI for changelog generation (claude, codex, gemini)
AI_CLI ?= claude

build:
	cargo build

release-build:
	cargo build --release

link: release-build
	sudo ln -sf "$$(pwd)/target/release/repo" /usr/local/bin/repo
	@echo "Linked: repo -> /usr/local/bin/repo"

unlink:
	sudo rm -f /usr/local/bin/repo
	@echo "Unlinked /usr/local/bin/repo"

clean:
	cargo clean

# Generate changelog from git commits since last tag
changelog:
	@if [ -z "$(VERSION)" ]; then \
		LAST_TAG=$$(git describe --tags --abbrev=0 2>/dev/null || echo ""); \
		if [ -z "$$LAST_TAG" ]; then \
			COMMITS=$$(git log --oneline --no-decorate); \
		else \
			COMMITS=$$(git log --oneline --no-decorate $$LAST_TAG..HEAD); \
		fi; \
	else \
		LAST_TAG=$$(git describe --tags --abbrev=0 2>/dev/null || echo ""); \
		if [ -z "$$LAST_TAG" ]; then \
			COMMITS=$$(git log --oneline --no-decorate); \
		else \
			COMMITS=$$(git log --oneline --no-decorate $$LAST_TAG..HEAD); \
		fi; \
	fi; \
	if [ -z "$$COMMITS" ]; then \
		echo "No commits since last tag"; \
		exit 1; \
	fi; \
	echo "$$COMMITS" | $(AI_CLI) -p "Generate a concise changelog for a GitHub release from these git commits. Group by category (Features, Fixes, etc). Output markdown only, no explanations:"

# Create a new release: make release VERSION=0.2.0
release:
	@if [ -z "$(VERSION)" ]; then \
		echo "Usage: make release VERSION=x.y.z"; \
		exit 1; \
	fi
	@echo "Creating release v$(VERSION)..."
	@# Update version in Cargo.toml
	@sed -i '' 's/^version = ".*"/version = "$(VERSION)"/' Cargo.toml
	@# Generate changelog
	@echo "Generating changelog with $(AI_CLI)..."
	@LAST_TAG=$$(git describe --tags --abbrev=0 2>/dev/null || echo ""); \
	if [ -z "$$LAST_TAG" ]; then \
		COMMITS=$$(git log --oneline --no-decorate); \
	else \
		COMMITS=$$(git log --oneline --no-decorate $$LAST_TAG..HEAD); \
	fi; \
	CHANGELOG=$$(echo "$$COMMITS" | $(AI_CLI) -p "Generate a concise changelog for GitHub release v$(VERSION) from these commits. Group by category (Features, Fixes, etc). Markdown format, no intro text:"); \
	echo "$$CHANGELOG" > .changelog.tmp
	@# Commit version bump
	@git add Cargo.toml Cargo.lock 2>/dev/null || git add Cargo.toml
	@git commit -m "chore: bump version to $(VERSION)"
	@git push origin master
	@# Create and push tag
	@git tag -a "v$(VERSION)" -m "Release v$(VERSION)"
	@git push origin "v$(VERSION)"
	@echo ""
	@echo "Release v$(VERSION) triggered!"
	@echo "GitHub Actions will build and publish: https://github.com/K-NRS/repo-cli/actions"
	@rm -f .changelog.tmp
