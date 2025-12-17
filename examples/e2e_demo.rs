//! Script to display full flow of transfering funds from Zcash to Ethereum and back.

use alloy::primitives::U256;
use zcash_eth_bridge::{
    eth::{sender::EthSender, watcher::EthWatcher},
    zcash::sender::TzeSender,
    zebra_client::{client::RpcClient as _, regtest::RegtestNetwork, wallet::Wallet},
};

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use zcash_protocol::value::Zatoshis;

const DEPOSIT_AMOUNT: u64 = 90_000;

const ETH_PK: &str = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
const ETH_ADDR: &str = "70997970C51812dc3A010C7d01b50e0d17dc79C8";

const ETH_BRIDGE_ADDR: &str = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512";
const WZEC_TOKEN_ADDR: &str = "0x5FbDB2315678afecb367f032d93F642f64180aa3";

const ZCASH_RECEIVER_SEED: [u8; 64] = [0x42; 64];

struct Demo {
    tze_sender: TzeSender,
    eth_watcher: EthWatcher,
    eth_sender: EthSender,
}

enum BlockId {
    Ethereum(u64),
    Zcash(u64),
}

impl Demo {
    async fn new(zebrad_addr: &str, anvil_addr: &str) -> anyhow::Result<Self> {
        let tze_sender = TzeSender::new(zebrad_addr).await?;
        let eth_watcher = EthWatcher::new(anvil_addr, ETH_BRIDGE_ADDR, WZEC_TOKEN_ADDR);
        let eth_sender = EthSender::new(anvil_addr, ETH_PK, ETH_BRIDGE_ADDR, WZEC_TOKEN_ADDR);
        Ok(Self {
            tze_sender,
            eth_watcher,
            eth_sender,
        })
    }

    fn zcash_receiver_wallet(&self) -> Wallet<RegtestNetwork> {
        Wallet::new(ZCASH_RECEIVER_SEED, RegtestNetwork)
    }

    async fn deposit_zec(&mut self, to: &str, deposit_amount: u64) -> anyhow::Result<u64> {
        let deposit_eth_addr: [u8; 20] = hex::decode(to).unwrap().try_into().unwrap();
        let (deposit_outpoint, deposit_tze_output) = self
            .tze_sender
            .send_tze_deposit(
                deposit_eth_addr,
                Zatoshis::const_from_u64(deposit_amount),
                50_000,
            )
            .await?;
        tracing::info!(
            "[tze deposit] hash: {}, output: {:?}",
            deposit_outpoint.txid(),
            deposit_tze_output
        );
        let tx_height = self.tze_sender.wait_for_tx(deposit_outpoint.txid()).await?;
        Ok(tx_height)
    }

    async fn wait_for_bridge(&mut self, height: BlockId) -> anyhow::Result<()> {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let current_state = self
                .eth_watcher
                .bridge_contract
                .latestState()
                .call()
                .await?;

            let reached = match &height {
                BlockId::Ethereum(h) => current_state.ethBlockNumber >= *h,
                BlockId::Zcash(h) => current_state.zecBlockNumber >= *h,
            };

            if reached {
                break;
            }
        }
        Ok(())
    }

    async fn withdraw_zec(&mut self, to: [u8; 20], amount: u64) -> anyhow::Result<u64> {
        // Allow the specified amount to be withdrawn
        let approve_tx = self
            .eth_sender
            .wzec_contract
            .approve(ETH_BRIDGE_ADDR.parse().unwrap(), U256::from(amount));
        let pending_approve_tx = approve_tx.send().await?;
        let _approve_receipt = pending_approve_tx.get_receipt().await?;

        // Perform the withdrawal
        let tx = self
            .eth_sender
            .bridge_contract
            .requestWithdrawal(U256::from(amount), to.into());
        let pending_tx = tx.send().await?;
        let receipt = pending_tx.get_receipt().await?;
        tracing::info!(
            "[ETH] Submitted withdrawal request, tx hash: {:?}",
            receipt.transaction_hash
        );
        Ok(receipt.block_number.unwrap())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let mut demo = Demo::new("127.0.0.1:18232", "http://127.0.0.1:8545").await?;

    let initial_balance = demo
        .eth_watcher
        .wzec_contract
        .balanceOf(ETH_ADDR.parse().unwrap())
        .call()
        .await?;
    tracing::info!("Initial WZEC balance: {initial_balance}");

    tracing::info!("Submitting Zcash->Ethereum deposit");
    let tx_height = demo.deposit_zec(ETH_ADDR, DEPOSIT_AMOUNT).await?;
    tracing::info!(
        "Deposit submitted on Zcash and included into block {tx_height}, waiting for it to be processed on the bridge"
    );

    demo.wait_for_bridge(BlockId::Zcash(tx_height)).await?;
    tracing::info!("Deposit processed on the bridge");

    let balance_after_bridging = demo
        .eth_watcher
        .wzec_contract
        .balanceOf(ETH_ADDR.parse().unwrap())
        .call()
        .await?;
    tracing::info!("WZEC balance after bridging: {balance_after_bridging}");

    let zcash_receiver_wallet = demo.zcash_receiver_wallet();
    let zcash_pk = zcash_receiver_wallet.derive_key(0, 0);
    tracing::info!("Withdrawing funds to Zcash address: {}", zcash_pk.address());

    let start_utxos = demo
        .tze_sender
        .client
        .get_address_utxos(zcash_pk.address().to_string())
        .await?;
    tracing::info!("Existing address UTXOs: {start_utxos:?}");

    tracing::info!("Submitting Ethereum->Zcash withdrawal");
    let withdraw_block = demo
        .withdraw_zec(zcash_pk.pubkey_hash(), DEPOSIT_AMOUNT)
        .await?;
    tracing::info!(
        "Withdrawal requested on Ethereum in block {withdraw_block}, waiting for it to be processed on Zcash"
    );
    demo.wait_for_bridge(BlockId::Ethereum(withdraw_block))
        .await?;

    tracing::info!("Withdrawal processed on the bridge");
    let final_balance = demo
        .eth_watcher
        .wzec_contract
        .balanceOf(ETH_ADDR.parse().unwrap())
        .call()
        .await?;
    tracing::info!("Final WZEC balance: {final_balance}");

    let mut final_utxos = demo
        .tze_sender
        .client
        .get_address_utxos(zcash_pk.address().to_string())
        .await?;
    // Looks like `zebra` requires some time to actually index the new UTXOs
    while final_utxos.len() == start_utxos.len() {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        final_utxos = demo
            .tze_sender
            .client
            .get_address_utxos(zcash_pk.address().to_string())
            .await?;
    }
    tracing::info!("Zcash address UTXOs after withdrawal: {final_utxos:?}");

    Ok(())
}
