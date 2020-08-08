build:
  cargo build

serve: build
  RUST_LOG=info,v8_example=trace ./target/debug/v8-example
