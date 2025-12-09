#/usr/bin/env bash

anvil --dump-state anvil-state.json &
ANVIL_PID=$!

cd contracts
forge build
# This is fine to have PK hardcoded here since this script is only for local anvil deployment
PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 forge script ./script/Deploy.s.sol --rpc-url http://127.0.0.1:8545 --broadcast

cd ..

kill $ANVIL_PID
