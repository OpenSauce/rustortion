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

# TAG is required: without --tag, git-cliff files the commits you are about to
# release under "[unreleased]" instead of the version being cut.
#   make changelog TAG=v0.3.0
changelog:
ifndef TAG
	$(error TAG is required, e.g. `make changelog TAG=v0.3.0`)
endif
	git-cliff --tag $(TAG) -o CHANGELOG.md
