run:
    cargo run
test:
    cargo run popular_now.scel
benchmark:
    hyperfine -m 100 --warmup 3 "$PWD/target/release/scel2rime ./popular_now.scel" "scel2rime ./popular.scel"
