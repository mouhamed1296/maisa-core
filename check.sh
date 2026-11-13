#!/bin/sh
# Full verification of maisa-core. Exits non-zero on the first failure.
#   ./check.sh          everything except mutation testing (about 20 s)
#   ./check.sh --full   also plants 55 defects and checks the tests catch each (about 4 min)
set -e
cd "$(dirname "$0")"
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test            # debug build: integer overflow checks on
cargo test --release
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
if [ "$1" = "--full" ]; then python3 tools/mutation_test.py; fi
echo "ALL CHECKS PASSED"
