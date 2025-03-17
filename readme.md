[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]

[crates-badge]: https://img.shields.io/crates/v/vsnap.svg
[crates-url]: https://crates.io/crates/vsnap
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/fominv/vsnap/blob/master/LICENSE

# vsnap

**A command line utility to snapshot & restore docker volumes.**

Useful during development for resetting database migrations and other similar use cases.

*Note: This is not intended as a backup solution.*

## Features

```sh
# Help
vsnap --help

# Snapshot volumes
vsnap create source-volume snapshot-a

# Optionally with compression
vsnap create --compression source-volume snapshot-b

# Restore
vsnap restore snapshot-a new-volume

# Optionally overwrite / reset old volume
vsnap restore snapshot-a source-volume

# List snapshot volume with sizes
vsnap list --size
```

## Installation

Make sure to have at least [Rust](https://www.rust-lang.org/learn/get-started) 1.85 installed as 
well as docker running with proper permissions for your local user.

```sh
cargo install vsnap
```

## Demo

![demo](./demo.gif)
