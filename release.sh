#!/usr/bin/env bash

cargo check
cargo test
cargo fmt -- --check

cargo release version --execute $1

docker build . --tag fominv/vsnap:latest --tag fominv/vsnap:$1
cargo release --package vsnap $1

docker push fominv/vsnap:latest
cargo release --execute --package vsnap $1 
