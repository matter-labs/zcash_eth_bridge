use crate::{types::ZecToEthTransfer, zebra_client::client::RpcClient as _};
use zcash_extensions::{consensus::transparent::EXTENSION_ETH_BRIDGE, transparent::eth_bridge};
use zcash_primitives::transaction::components::{TzeOut, tze};
use zcash_primitives::{block::BlockHash, extensions::transparent::FromPayload};
use zcash_protocol::TxId;
use zcash_protocol::value::Zatoshis;
use zebra_chain::{
    block::Block, serialization::ZcashDeserialize as _, transparent::ExtendedScript,
};
use zebra_node_services::rpc_client::RpcRequestClient;
use zebra_rpc::methods::GetBlockResponse;

pub struct ZcashWatcher {
    client: RpcRequestClient,
}

impl ZcashWatcher {
    pub fn new(rpc_url: &str) -> Self {
        let client = RpcRequestClient::new(rpc_url.parse().unwrap());
        Self { client }
    }

    pub async fn get_block_count(&self) -> anyhow::Result<u32> {
        let count = self.client.get_block_count().await?;
        Ok(count)
    }

    pub async fn extract_zec_to_eth_transfers(
        &self,
        blocks: &[Block],
    ) -> anyhow::Result<(Vec<ZecToEthTransfer>, Vec<(tze::OutPoint, TzeOut)>)> {
        let mut transfers = Vec::new();
        let mut outpoints = Vec::new();

        for block in blocks {
            for tx in &block.transactions {
                for (n, output) in tx.outputs().iter().enumerate() {
                    let ExtendedScript::Extension(tze) = &output.lock_script else {
                        // Not a TZE
                        continue;
                    };

                    if tze.extension_id != EXTENSION_ETH_BRIDGE {
                        // Not an EthBridge deposit
                        continue;
                    }

                    let Ok(eth_bridge::Precondition::Deposit(deposit_data)) =
                        eth_bridge::Precondition::from_payload(tze.mode, &tze.payload)
                    else {
                        // Not a (valid, at least) deposit
                        continue;
                    };

                    let transfer = ZecToEthTransfer {
                        eth_address: deposit_data.to,
                        amount: output.value.zatoshis() as u64,
                    };
                    transfers.push(transfer);

                    let outpoint = tze::OutPoint::new(TxId::from_bytes(tx.hash().0), n as u32);
                    let tze_out = TzeOut {
                        value: Zatoshis::from_nonnegative_i64(output.value.zatoshis()).unwrap(),
                        precondition: zcash_primitives::extensions::transparent::Precondition {
                            extension_id: tze.extension_id,
                            mode: tze.mode,
                            payload: tze.payload.clone(),
                        },
                    };
                    outpoints.push((outpoint, tze_out));
                }
            }
        }

        Ok((transfers, outpoints))
    }

    pub async fn get_block(&self, height: u32) -> anyhow::Result<Block> {
        let block_hash = self.client.get_block_hash(height).await?;
        let block = self
            .client
            .get_block(&BlockHash(block_hash.hash().0))
            .await?;
        let block = match block {
            GetBlockResponse::Raw(raw) => Block::zcash_deserialize(raw.as_ref())?,
            GetBlockResponse::Object(_obj) => todo!("Only raw blocks are supported for now"),
        };
        Ok(block)
    }
}
