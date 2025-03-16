#!/usr/bin/env bash

set -ue -o pipefail

(
    cd rust
    cargo fmt -- --check
    cargo check
    cargo test

    cargo release --package vsnap $1
)

docker build . --tag fominv/vsnap:latest --tag fominv/vsnap:$1
docker push fominv/vsnap:latest

(
    cd rust
    cargo release --execute --package vsnap $1
)
