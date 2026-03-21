# filter out target and keep the rest as args
PRIMARY_TARGET := $(firstword $(MAKECMDGOALS))
ARGS := $(filter-out $(PRIMARY_TARGET), $(MAKECMDGOALS))

.PHONY: git-hooks
git-hooks:
	git config core.hooksPath ./git-hooks;

.PHONY: init
init: git-hooks

.PHONY: fmt
fmt: init
	cargo fmt

.PHONY: test
test: test-tokio test-smol
	cargo test --all-features -- --nocapture --test-threads=1

.PHONY: test-tokio
test-tokio: init
	cargo check -p orb-tokio
	cargo test -p orb-tokio ${ARGS} --all-features -- --nocapture --test-threads=1

.PHONY: test-smol
test-smol: init
	cargo check -p orb-smol
	echo "run smol"
	cargo test -p orb-smol ${ARGS} -- --nocapture --test-threads=1
	#echo "run smol with -F global"
	cargo test -p orb-smol ${ARGS} --all-features -- --nocapture --test-threads=1

.PHONY: build
build: init
	cargo build -p orb-tokio
	cargo build -p orb-smol
	cargo build -p orb-smol --all-features
	cargo build --all-features

.DEFAULT_GOAL = build

# Target name % means that it is a rule that matches anything, @: is a recipe;
# the : means do nothing
%:
	@:
