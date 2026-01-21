.PHONY: build release link unlink clean

build:
	cargo build

release:
	cargo build --release

link: release
	sudo ln -sf "$$(pwd)/target/release/repo" /usr/local/bin/repo
	@echo "Linked: repo -> /usr/local/bin/repo"

unlink:
	sudo rm -f /usr/local/bin/repo
	@echo "Unlinked /usr/local/bin/repo"

clean:
	cargo clean
