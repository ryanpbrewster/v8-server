build:
  cargo build

serve: build
  RUST_LOG=info ./target/debug/v8-example
