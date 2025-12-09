#!/usr/bin/env bash

RUSTFLAGS="--cfg zcash_unstable=\"zfuture\"" cargo test -- e2e_demo
