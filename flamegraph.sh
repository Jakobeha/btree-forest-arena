SCRIPT_PATH=$(dirname "$(realpath "$0")")
CARGO_PROFILE_BENCH_DEBUG=true sudo cargo flamegraph \
  --bench benches \
  --manifest-path "$SCRIPT_PATH/benches/Cargo.toml" \
  --output "$SCRIPT_PATH/benches/target/criterion/report/flamegraph.svg" \
  --features slab,shareable-slab,concurrent-shareable-slab,shareable-slab-simultaneous-mutation,shareable-slab-arena \
  --root \
  --deterministic \
  -- --bench
sudo chmod -R 775 "$SCRIPT_PATH/benches/target"