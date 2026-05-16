run file="popular_now.scel":
    cargo run -- {{file}}

batch config="scel2rime.conf":
    cargo run -- --config {{config}}

check:
    cargo fmt -- --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cargo check

benchmark file="popular_now.scel":
    cargo build --release
    hyperfine -m 100 --warmup 3 "$PWD/target/release/scel2rime {{file}}"
