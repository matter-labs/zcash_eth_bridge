cd zebra
RUSTFLAGS="--cfg zcash_unstable=\"zfuture\"" cargo build --release --features tx_v6,internal-miner

cd ../zcash_regtest
../zebra/target/release/zebrad --config ./ethbridge.toml
