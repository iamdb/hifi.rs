SHELL := bash
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

ifeq ($(origin .RECIPEPREFIX), undefined)
  $(error This Make does not support .RECIPEPREFIX. Please use GNU Make 4.0 or later)
endif
.RECIPEPREFIX = >

build: target/release/hifi-rs
.PHONY: build

clean:
> rm -rf target
> rm -rf downloads
.PHONY: clean

test: tests/.tests-passed.sentinel

git-hooks: .git/hooks/commit-msg

clippy: $(shell find src -type f)
> cargo clippy --all-targets -- -D warnings
.PHONY: clippy

format: $(shell find src -type f)
> cargo fmt -- --check
.PHONY: format

tests/.tests-passed.sentinel: $(shell find src -type f)
> cargo test
> touch $@

target/release/hifi-rs: tests/.tests-passed.sentinel
> cargo build --release
> touch $@

.git/hooks/commit-msg: git-hooks/commit-msg
> cp git-hooks/commit-msg .git/hooks/commit-msg
