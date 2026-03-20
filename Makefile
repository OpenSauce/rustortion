.PHONY: all lint clippy test check fmt bench cover changelog plugin plugin-install

all: lint clippy test

lint: fmt clippy

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --all-features -- \
		-D warnings \
		-D clippy::all \
		-D clippy::pedantic \
		-D clippy::nursery

test:
	cargo test --workspace --all-targets --all-features

bench:
	cargo bench --workspace

cover:
	cargo tarpaulin --workspace

plugin:
	cargo xtask bundle rustortion-plugin --release

plugin-install:
	mkdir -p ~/.clap ~/.vst3
	cp target/bundled/Rustortion.clap ~/.clap/
	cp -r target/bundled/Rustortion.vst3 ~/.vst3/

changelog:
	git-cliff -o CHANGELOG.md
