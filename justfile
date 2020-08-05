build:
  cargo build --release

serve: build
  RUST_LOG=info ./target/release/v8-example
