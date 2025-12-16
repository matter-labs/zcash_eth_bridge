# ZCash <-> Ethereum trustless bridge prototype

This project presents a prototype of a trustless bridge between ZCash and Ethereum.
Additionally, it provides a demo flow for bridging funds from ZCash to Ethereum and back.

## Prerequisites

- [`zebra` general requirements](https://github.com/ZcashFoundation/zebra?tab=readme-ov-file#manual-install)
- [Foundry](https://getfoundry.sh/)
- Submodules initialized via `git submodule update --init`. 

## Running the demo


First, build the project using `./build.sh`
Then, in 3 different terminals, run:

```sh
# 1st terminal
./run_zcash.sh
# 2nd terminal
./run_anvil.sh
# 3rd terminal
./run_zcash.sh
```

After everything is started, you can run the demo that would test whether bridging works as intended.

```sh
./run_demo.sh
```

## Workflow

The best way to learn the application logic would be to check the `main` function in [`main.rs`](./src/main.rs), it is pretty basic.

The application connects to both ZCash and Ethereum nodes, and watches for the new blocks generated.
As soon as at least 1 block is generated on both chains, a state update is prepared:
- Deposit requests are extracted from the new ZCash blocks.
- Withdrawal requests are extracted from the new Ethereum blocks.
- A single state update object is prepared, containing information about both chains.
- Update transaction is sent to Zcash.
- Update transaction is sent to Ethereum.
- Proceed to the next loop iteration.

## TZE implementation details

In order to make this project possible, a new TZE is created. Definition of the TZE can be found [here](https://github.com/matter-labs/librustzcash/tree/popzxc-prototype/zcash_extensions/src).

TZE ID: 2 (number chosen to not collide with previously proposed TZE implementation)

Modes:

| Mode ID | Mode name | Description |
|---------|-----------|-------------|
| 0       | Create    | Instantiates a new STF object, with a unique identifier. |
| 1       | STF       | Progresses STF from state A to state B, claims deposits, processes withdrawals. |
| 2       | Deposit   | Locks the funds for depositing, to be claimed by the STF UTXO on the next state update. |


## Caveats

This prototype serves a single purpose only: to demonstrate the feasibility of a cross-chain bridge between ZCash and Ethereum.
It is intentionally incomplete, since the critical implementation details are yet to be decided during a discussion with the
ZCash community.

Examples of incompleteness:
- TZE preconditions/witness only specify a single root hash. Extending it to two root hashes for both chains is trivial.
- TZE Create mode does not enforce the uniquieness of the STF identifier. This can be implemented e.g. by using a signature made with private key that only STF creator posesses.
- Continuity of the STF is not enforced (e.g. making sure that the whole sequence matches a single ID). This can be done by exposing previous tx contents in the TZE context.
- No ZK verification is implemented. This can be done once there is agreement w.r.t. ZK backend to be used.
- Consensus-level verification for deposits/withdrawals is not sufficient. This can be implemented, if access to previous tx contents is added in the TZE context.
- Ethereum contracts are very basic and missing common implementation best practices.

This implementation is not meant and not suitable for any kind of production use, and serves a demonstration purpose only.
Use at your own risk.

## Acknowledgements

Parts of this repository, mainly `zebra_client` module, are based on the
[`zebra` fork](https://github.com/Ztarknet/zebra) by Starkware, licensed
under [MIT](https://github.com/Ztarknet/zebra/blob/zfuture/LICENSE-MIT)/[Apache 2.0](https://github.com/Ztarknet/zebra/blob/zfuture/LICENSE-APACHE).

Forks of `zebra` and `librustzcash` used in this repository are also partially
based on the corresponding forks ([1](https://github.com/Ztarknet/librustzcash), [2](https://github.com/Ztarknet/zebra)), both licensed
under MIT/Apache 2.0.

Thank you!

## License

Licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.
