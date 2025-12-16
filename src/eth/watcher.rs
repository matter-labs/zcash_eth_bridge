use alloy::{
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::Filter,
    sol_types::SolEvent,
};
use anyhow::Result;

use crate::{
    eth::contract::{
        WZec::{self, WZecInstance},
        ZcashBridge::{self, ZcashBridgeInstance},
    },
    types::EthToZecTransfer,
};

pub struct EthWatcher {
    provider: DynProvider,
    pub bridge_contract: ZcashBridgeInstance<DynProvider>,
    pub wzec_contract: WZecInstance<DynProvider>,
}

impl EthWatcher {
    pub fn new(rpc_url: &str, bridge_address: &str, wzec_address: &str) -> Self {
        let provider =
            DynProvider::new(ProviderBuilder::new().connect_http(rpc_url.parse().unwrap()));
        let bridge_contract = ZcashBridge::new(bridge_address.parse().unwrap(), provider.clone());
        let wzec_contract = WZec::new(wzec_address.parse().unwrap(), provider.clone());
        Self {
            provider,
            bridge_contract,
            wzec_contract,
        }
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        let block_number = self.provider.get_block_number().await?;
        Ok(block_number)
    }

    pub async fn extract_eth_to_zec_transfers(
        &self,
        blocks: &[alloy::rpc::types::Block],
    ) -> Result<Vec<EthToZecTransfer>> {
        let mut transfers = Vec::new();

        let first_block = blocks.first().unwrap().number();
        let last_block = blocks.last().unwrap().number();

        let filter = Filter::new()
            .address(*self.bridge_contract.address())
            .from_block(first_block)
            .to_block(last_block)
            .event_signature(super::contract::ZcashBridge::WithdrawalRequested::SIGNATURE_HASH);
        let logs = self.provider.get_logs(&filter).await?;

        for log in logs {
            let event = super::contract::ZcashBridge::WithdrawalRequested::decode_log(&log.into())?;
            let transfer = EthToZecTransfer {
                pubkey_hash: event.pubkeyHash.0,
                amount: u64::try_from(event.amount).expect("Amount exceeds u64"),
            };
            transfers.push(transfer);
        }

        Ok(transfers)
    }

    pub async fn get_block(&self, block_number: u64) -> Result<alloy::rpc::types::Block> {
        let block = self
            .provider
            .get_block(block_number.into())
            .await?
            .expect("Block not found");
        Ok(block)
    }
}
