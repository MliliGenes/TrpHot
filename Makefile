.PHONY: all test install clean
all:
	cargo build --release --locked
test:
	cargo test --locked
	cargo build --locked
	python3 tests/integration.py
	./node_modules/.bin/tsc --noEmit
	./node_modules/.bin/bun test ui/app.test.ts
install:
	./install.sh
clean:
	cargo clean
