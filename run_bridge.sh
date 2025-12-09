#!/usr/bin/env bash

RUSTFLAGS="--cfg zcash_unstable=\"zfuture\"" cargo run --release
