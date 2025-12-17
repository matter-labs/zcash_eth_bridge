use alloy::{
    primitives::{Address, B256, U256},
    providers::{DynProvider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};

use crate::eth::contract::{
    WZec::{self, WZecInstance},
    ZcashBridge::{self, ZcashBridgeInstance},
};
use crate::types::StateUpdate;

pub struct EthSender {
    _provider: DynProvider,
    pub bridge_contract: ZcashBridgeInstance<DynProvider>,
    pub wzec_contract: WZecInstance<DynProvider>,
}

impl EthSender {
    pub fn new(rpc_url: &str, pk: &str, bridge_address: &str, wzec_address: &str) -> Self {
        let wallet: PrivateKeySigner = pk.parse().expect("Invalid private key");
        let provider = DynProvider::new(
            ProviderBuilder::new()
                .wallet(wallet)
                .connect_http(rpc_url.parse().unwrap()),
        );
        let bridge_contract = ZcashBridge::new(bridge_address.parse().unwrap(), provider.clone());
        let wzec_contract = WZec::new(wzec_address.parse().unwrap(), provider.clone());
        Self {
            _provider: provider,
            bridge_contract,
            wzec_contract,
        }
    }

    pub async fn update_bridge(&self, state_update: StateUpdate) -> anyhow::Result<()> {
        let state_update = super::contract::ZcashBridge::StateUpdate {
            previousEthRoot: B256::new(state_update.old_eth_hash),
            previousEthBlockNumber: state_update.old_eth_block,
            newEthRoot: B256::new(state_update.new_eth_hash),
            newEthBlockNumber: state_update.new_eth_block,
            previousZecRoot: B256::new(state_update.old_zcash_hash),
            previousZecBlockNumber: state_update.old_zcash_block,
            newZecRoot: B256::new(state_update.new_zcash_hash),
            newZecBlockNumber: state_update.new_zcash_block,
            zecToEthTransfers: state_update
                .zec_to_eth_transfers
                .iter()
                .map(
                    |transfer| super::contract::ZcashBridge::ProcessedZecToEthTransfer {
                        to: Address::from_slice(&transfer.eth_address),
                        amount: U256::from(transfer.amount),
                    },
                )
                .collect(),
            ethToZecTransfers: state_update
                .eth_to_zec_transfers
                .iter()
                .map(
                    |transfer| super::contract::ZcashBridge::ProcessedEthToZecTransfer {
                        pubkeyHash: transfer.pubkey_hash.into(),
                        amount: U256::from(transfer.amount),
                    },
                )
                .collect(),
        };

        let tx = self.bridge_contract.submitStateUpdate(state_update);
        let pending_tx = tx.send().await?;
        let receipt = pending_tx.get_receipt().await?;
        tracing::debug!("[ETH] Submitted state update, receipt: {receipt:?}");

        Ok(())
    }
}
