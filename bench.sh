SCRIPT_PATH=$(dirname "$(realpath "$0")")
cargo bench --all-features --manifest-path "$SCRIPT_PATH/benches/Cargo.toml"
