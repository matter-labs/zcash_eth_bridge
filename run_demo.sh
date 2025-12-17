#!/usr/bin/env bash

RUSTFLAGS="--cfg zcash_unstable=\"zfuture\"" cargo run --example e2e_demo
