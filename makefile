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

OUTPUT=target/release/hifi-rs
SRC_FILES=$(shell find src/ -type f -iname \*.rs)
TEST_FILES=$(shell find tests -type f -iname \*.rs)
TEST_SENTINEL=tests/.tests-passed.sentinel

build: $(OUTPUT)
.PHONY: build

clean:
> rm -rf target
> rm -rf downloads
.PHONY: clean

test: $(TEST_SENTINEL)

git-hooks: .git/hooks/commit-msg

clippy: $(SRC_FILES)
> cargo clippy --all-targets -- -D warnings
.PHONY: clippy

format: $(SRC_FILES)
> cargo fmt -- --check
.PHONY: format

tests/.tests-passed.sentinel: $(SRC_FILES) $(TEST_FILES) 
> cargo test
> touch $@

target/release/hifi-rs: $(TEST_SENTINEL)
> cargo build --release
> touch $@

.git/hooks/commit-msg: git-hooks/commit-msg
> cp git-hooks/commit-msg .git/hooks/commit-msg
