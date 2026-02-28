.PHONY: all lint clippy test check fmt

all: lint clippy test

lint: fmt clippy

fmt:
	cargo fmt -- --check

clippy:
	cargo clippy --all-targets --all-features -- \
		-D warnings \
		-D clippy::all \
		-D clippy::pedantic \
		-D clippy::nursery

test:
	cargo test --all-targets --all-features

bench:
	cargo bench

cover:
	cargo tarpaulin

changelog:
	git-cliff -o CHANGELOG.md
