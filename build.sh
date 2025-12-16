#/usr/bin/env bash

set -e

cd contracts
forge soldeer install
forge build
cd ..

RUSTFLAGS="--cfg zcash_unstable=\"zfuture\"" cargo build --release
