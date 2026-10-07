#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python -m pip install -r conformance/requirements.txt
python scripts/build-bindings.py --python
cargo build -p rust --example conformance
export CGO_LDFLAGS="-L$(pwd)/target/debug -Wl,-rpath,$(pwd)/target/debug"
(cd packages/go && go test ./... && go build -o ../../target/debug/go-conformance ./conformance)
STARGATE_TEST_PYTHON="$(command -v python)" python conformance/run.py
python -m pytest packages/python/tests
(cd packages/node && npm install --no-package-lock --no-audit --no-fund && npm test && npm run types)
