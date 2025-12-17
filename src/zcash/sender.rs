use crate::{
    types::StateUpdate,
    zebra_client::{
        client::RpcClient as _,
        helpers::spendable_coinbase_txid,
        regtest::RegtestNetwork,
        wallet::{Key, Wallet, regtest_default_wallet},
    },
};
use rand_core::OsRng;
use zcash_extensions::transparent::eth_bridge::{self};
use zcash_primitives::transaction::{
    builder::{BuildResult, Builder},
    components::{TzeOut, tze},
    fees::fixed::FeeRule,
};
use zcash_proofs::prover::LocalTxProver;
use zcash_protocol::{TxId, consensus::BranchId, value::Zatoshis};
use zcash_transparent::{
    address::TransparentAddress,
    builder::TransparentSigningSet,
    bundle::{OutPoint, TxOut},
};
use zebra_chain::transaction;
use zebra_node_services::rpc_client::RpcRequestClient;
use zebra_rpc::methods::GetRawTransaction;

/// The amount to lock in the TZE STF output for it to not be considered dust.
const LOCK_IN_VALUE: Zatoshis = Zatoshis::const_from_u64(100_000);

pub struct TzeSender {
    pub client: RpcRequestClient,
    wallet: Wallet<RegtestNetwork>,
    miner_key: Key,
    stf_identifier: [u8; 32],
    root_hash: [u8; 32],
    // For now we expect that we can always pay for a tx with a single input.
    fee_txid: TxId,
    // Tracks the amount of deposited funds
    deposited: Zatoshis,
}

impl TzeSender {
    pub async fn new(rpc_address: &str) -> anyhow::Result<Self> {
        let client = RpcRequestClient::new(rpc_address.parse().unwrap());
        let wallet = regtest_default_wallet();
        let miner_key = wallet.derive_key(0, 0);

        let target_height = client.get_block_count().await? + 1;
        let fee_txid = spendable_coinbase_txid(&client, target_height).await?;
        Ok(Self {
            client,
            wallet,
            miner_key,
            stf_identifier: [0xAB; 32],
            root_hash: [0xCD; 32],
            fee_txid,
            deposited: Zatoshis::ZERO,
        })
    }

    pub async fn send_tze_create(&mut self, fee: u64) -> anyhow::Result<(tze::OutPoint, TzeOut)> {
        let target_height = self.target_height().await?;

        let mut builder = eth_bridge::builder::EthBridgeTzeBuilder {
            txn_builder: self.wallet.tx_builder(target_height),
            extension_id: zcash_extensions::consensus::transparent::EXTENSION_ETH_BRIDGE,
        };
        let coin = self.add_fee_input(&mut builder.txn_builder).await?;

        let value = (coin.value() - Zatoshis::const_from_u64(fee)).unwrap();
        let value = (value - LOCK_IN_VALUE).unwrap();
        builder.add_create_output(LOCK_IN_VALUE, self.stf_identifier, self.root_hash)?;
        assert_eq!(
            self.deposited,
            Zatoshis::ZERO,
            "Create called on a dirty state"
        );
        self.deposited = LOCK_IN_VALUE;

        self.add_fee_output(&mut builder.txn_builder, value).await?;

        let res = self.finish_tx(builder.txn_builder, fee).await?;
        let tx = res.transaction();

        let tze_output = tx.tze_bundle().unwrap().vout[0].clone();
        let hash = self.client.send_raw_transaction(tx).await.unwrap().hash();
        tracing::debug!("[tze create] Tx: {tx:?}");

        // TZE outpoints come after transparent outputs, so index 1.
        let outpoint = Self::outpoint(&hash, 1);
        self.fee_txid = TxId::from_bytes(hash.0);

        Ok((outpoint, tze_output))
    }

    pub async fn send_tze_deposit(
        &mut self,
        to_eth_addr: [u8; 20],
        amount: Zatoshis,
        fee: u64,
    ) -> anyhow::Result<(tze::OutPoint, TzeOut)> {
        let target_height = self.target_height().await?;

        let mut builder = eth_bridge::builder::EthBridgeTzeBuilder {
            txn_builder: self.wallet.tx_builder(target_height),
            extension_id: zcash_extensions::consensus::transparent::EXTENSION_ETH_BRIDGE,
        };
        let coin = self.add_fee_input(&mut builder.txn_builder).await?;

        builder.add_deposit_output(amount, self.stf_identifier, to_eth_addr)?;

        let value = (coin.value() - Zatoshis::const_from_u64(fee)).unwrap();
        let value = (value - amount).unwrap();
        self.add_fee_output(&mut builder.txn_builder, value).await?;

        let res = self.finish_tx(builder.txn_builder, fee).await?;
        let tx = res.transaction();
        tracing::debug!("[tze deposit] Tx: {tx:?}");

        let tze_output = tx.tze_bundle().unwrap().vout[0].clone();
        let hash = self.client.send_raw_transaction(tx).await.unwrap().hash();

        // TZE outpoints come after transparent outputs, so index 1.
        let outpoint = Self::outpoint(&hash, 1);
        self.fee_txid = TxId::from_bytes(hash.0);

        Ok((outpoint, tze_output))
    }

    pub async fn initialize_tze_stf(
        &mut self,
        fee: u64,
        prevout: (tze::OutPoint, TzeOut),
    ) -> anyhow::Result<(tze::OutPoint, TzeOut)> {
        let target_height = self.target_height().await?;

        let mut builder = eth_bridge::builder::EthBridgeTzeBuilder {
            txn_builder: self.wallet.tx_builder(target_height),
            extension_id: zcash_extensions::consensus::transparent::EXTENSION_ETH_BRIDGE,
        };

        let coin = self.add_fee_input(&mut builder.txn_builder).await?;
        builder.add_create_input(prevout)?;

        builder.add_stf_output(LOCK_IN_VALUE, self.stf_identifier, self.root_hash)?;
        let value = (coin.value() - Zatoshis::const_from_u64(fee)).unwrap();
        self.add_fee_output(&mut builder.txn_builder, value).await?;

        let res = self.finish_tx(builder.txn_builder, fee).await?;
        let tx = res.transaction();
        tracing::debug!("[tze init stf] Tx: {tx:?}");

        let tze_output = tx.tze_bundle().unwrap().vout[0].clone();
        let hash = self.client.send_raw_transaction(tx).await.unwrap().hash();

        // TZE outpoints come after transparent outputs, so index 1.
        let outpoint = Self::outpoint(&hash, 1);
        self.fee_txid = TxId::from_bytes(hash.0);

        Ok((outpoint, tze_output))
    }

    pub async fn progress_tze_stf(
        &mut self,
        fee: u64,
        prevout: (tze::OutPoint, TzeOut),
        deposit_outpoints: Vec<(tze::OutPoint, TzeOut)>,
        processed_deposits: Vec<eth_bridge::modes::stf::ProcessedDeposit>,
        processed_withdrawals: Vec<eth_bridge::modes::stf::ProcessedWithdrawal>,
    ) -> anyhow::Result<(tze::OutPoint, TzeOut)> {
        let target_height = self.target_height().await?;

        let mut builder = eth_bridge::builder::EthBridgeTzeBuilder {
            txn_builder: self.wallet.tx_builder(target_height),
            extension_id: zcash_extensions::consensus::transparent::EXTENSION_ETH_BRIDGE,
        };

        let coin = self.add_fee_input(&mut builder.txn_builder).await?;
        // TZE outpoints come after transparent outputs, so index 1.
        builder.add_stf_input(
            prevout,
            self.stf_identifier,
            self.root_hash,
            processed_deposits,
            processed_withdrawals.clone(),
        )?;

        for deposit_outpoint in deposit_outpoints {
            self.deposited = (self.deposited + deposit_outpoint.1.value).unwrap();
            builder.add_deposit_input(deposit_outpoint)?;
        }

        // TZE outpoints come after transparent outputs, so index 1 + number of withdrawal outputs.
        let stf_output_number = 1 + processed_withdrawals.len() as u32;

        // 1. Transparent inputs (they go first in vout)
        let value = (coin.value() - Zatoshis::const_from_u64(fee)).unwrap();
        self.add_fee_output(&mut builder.txn_builder, value).await?;

        // 2. Withdrawal outputs (still transparent).
        for withdrawal in processed_withdrawals {
            builder
                .txn_builder
                .add_transparent_output(
                    &TransparentAddress::PublicKeyHash(withdrawal.pubkey_hash),
                    withdrawal.amount,
                )
                .map_err(wrap_anyhow)?;
            self.deposited = (self.deposited - withdrawal.amount).unwrap();
        }

        // 3. TZE STF output
        builder.add_stf_output(self.deposited, self.stf_identifier, self.root_hash)?;

        let res = self.finish_tx(builder.txn_builder, fee).await?;
        let tx = res.transaction();
        tracing::debug!("[tze progress stf] Tx: {tx:?}");

        let tze_output = tx.tze_bundle().unwrap().vout[0].clone();
        let hash = self.client.send_raw_transaction(tx).await.unwrap().hash();

        let outpoint = Self::outpoint(&hash, stf_output_number);
        self.fee_txid = TxId::from_bytes(hash.0);

        Ok((outpoint, tze_output))
    }

    pub async fn update_zcash(
        &mut self,
        prevout: (tze::OutPoint, TzeOut),
        zcash_deposit_outpoints: Vec<(tze::OutPoint, TzeOut)>,
        state_update: StateUpdate,
    ) -> anyhow::Result<(tze::OutPoint, TzeOut)> {
        let zec_to_eth_transfers = state_update
            .zec_to_eth_transfers
            .into_iter()
            .map(
                |t| zcash_extensions::transparent::eth_bridge::modes::stf::ProcessedDeposit {
                    to: t.eth_address,
                    amount: zcash_protocol::value::Zatoshis::from_u64(t.amount).unwrap(),
                },
            )
            .collect();
        let eth_to_zec_transers = state_update
            .eth_to_zec_transfers
            .into_iter()
            .map(
                |t| zcash_extensions::transparent::eth_bridge::modes::stf::ProcessedWithdrawal {
                    pubkey_hash: t.pubkey_hash,
                    amount: zcash_protocol::value::Zatoshis::from_u64(t.amount).unwrap(),
                },
            )
            .collect();

        self.progress_tze_stf(
            50_000,
            prevout,
            zcash_deposit_outpoints,
            zec_to_eth_transfers,
            eth_to_zec_transers,
        )
        .await
    }

    pub async fn deploy(&mut self) -> anyhow::Result<(tze::OutPoint, TzeOut)> {
        let (create_outpoint, create_tze_output) = self.send_tze_create(50_000).await?;
        tracing::debug!(
            "[tze create] hash: {}, output: {:?}",
            create_outpoint.txid(),
            create_tze_output
        );
        self.wait_for_tx(create_outpoint.txid()).await?;

        let (stf_tze_outpoint, stf_tze_output) = self
            .initialize_tze_stf(50_000, (create_outpoint, create_tze_output))
            .await?;
        tracing::debug!(
            "[tze stf init] hash: {}, output: {:?}",
            stf_tze_outpoint.txid(),
            stf_tze_output
        );
        self.wait_for_tx(stf_tze_outpoint.txid()).await?;

        Ok((stf_tze_outpoint, stf_tze_output))
    }

    pub async fn wait_for_tx(&self, txid: &TxId) -> anyhow::Result<u64> {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            match self.client.get_raw_transaction(txid, true).await {
                Ok(tx) => match tx {
                    GetRawTransaction::Object(tx) => {
                        // `None` = mempool, `Some(-1)` = side chain, `Some(height >= 0)` = main chain
                        if tx.height() > Some(0) {
                            return Ok(tx.height().unwrap() as u64);
                        }
                    }
                    _ => panic!("Unexpected response"),
                },
                Err(_) => continue,
            }
        }
    }

    async fn target_height(&self) -> anyhow::Result<u32> {
        let block_count = self.client.get_block_count().await?;
        Ok(block_count + 1)
    }

    async fn add_fee_input<'a>(
        &self,
        builder: &mut Builder<'a, RegtestNetwork, ()>,
    ) -> anyhow::Result<TxOut> {
        let (txid, coin) = self.spendable_tx().await?;

        builder
            .add_transparent_input(
                self.miner_key.public_key(),
                OutPoint::new(txid.into(), 0),
                coin.clone(),
            )
            .map_err(wrap_anyhow)?;

        Ok(coin)
    }

    async fn add_fee_output<'a>(
        &self,
        builder: &mut Builder<'a, RegtestNetwork, ()>,
        value: Zatoshis,
    ) -> anyhow::Result<()> {
        let to = self.wallet.derive_key(0, 0).transparent_address();
        builder
            .add_transparent_output(&to, value)
            .map_err(wrap_anyhow)?;
        Ok(())
    }

    async fn finish_tx<'a>(
        &self,
        builder: Builder<'a, RegtestNetwork, ()>,
        fee: u64,
    ) -> anyhow::Result<BuildResult> {
        let mut transparent_signing_set = TransparentSigningSet::new();
        transparent_signing_set.add_key(self.miner_key.secret_key());

        let fee_rule = FeeRule::non_standard(Zatoshis::const_from_u64(fee));
        let prover = LocalTxProver::bundled();

        let res = builder
            .build_zfuture(
                &transparent_signing_set,
                &[],
                &[],
                OsRng,
                &prover,
                &prover,
                &fee_rule,
            )
            .map_err(|e| format!("build failure: {:?}", e))
            .unwrap();

        Ok(res)
    }

    async fn spendable_tx(&self) -> anyhow::Result<(TxId, TxOut)> {
        let tx = self
            .client
            .get_transaction(&self.fee_txid, BranchId::ZFuture)
            .await?;
        let coin = tx.transparent_bundle().unwrap().vout[0].clone();
        Ok((self.fee_txid, coin))
    }

    fn outpoint(hash: &transaction::Hash, vout: u32) -> tze::OutPoint {
        tze::OutPoint::new(TxId::from_bytes(hash.0), vout)
    }
}

fn wrap_anyhow<T: std::fmt::Display>(err: T) -> anyhow::Error {
    anyhow::anyhow!(err.to_string())
}
