.PHONY: all test install clean
all:
	cargo build --release --locked
test:
	cargo test --locked
	cargo build --locked
	python3 tests/integration.py
	python3 tests/web.py
install:
	./install.sh
clean:
	cargo clean
