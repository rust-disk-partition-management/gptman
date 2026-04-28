# Contributing

## Code style

`cargo fmt` and `cargo clippy -- -D warnings` must pass. Using `#[allow(...)]`
locally to suppress a specific lint is fine.

## Tests

Run the test suite with:

```sh
cargo test --workspace
cargo test --workspace --no-default-features
```

The test fixtures in `tests/fixtures/` are binary disk images created by hand
using `fdisk`. If you need a new fixture, create a disk image the same way and
commit it.

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).
