
on: [push, pull_request]

jobs:
  rust-ci:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        toolchain: [stable]

    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Install tools
        run: |
          cargo install cargo-deny
          cargo install cargo-audit

      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - run: cargo test --workspace --all-features
      - run: cargo deny check
      - run: cargo audit
      - run: cargo doc --no-deps --workspace