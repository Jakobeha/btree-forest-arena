SCRIPT_PATH=$(dirname "$(realpath "$0")")
cargo flamegraph \
  --bench benches \
  --manifest-path "$SCRIPT_PATH/benches/Cargo.toml" \
  --output "$SCRIPT_PATH/benches/target/criterion/report/flamegraph.svg" \
  --features slab,shareable-slab,concurrent-shareable-slab,shareable-slab-simultaneous-mutation,shareable-slab-arena \
  --root \
  --deterministic \
  -- --bench
chown -R "$USER" "$SCRIPT_PATH/benches/target"