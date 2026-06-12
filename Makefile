.PHONY: build run test bench fmt lint check clean

build:
	cargo build --release

run:
	cargo run --release -- $(FILE)

test:
	cargo test

bench:
	cargo bench

fmt:
	cargo fmt


# Run fmt + lint + tests together before committing
check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test

clean:
	cargo clean
