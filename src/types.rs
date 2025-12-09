#[derive(Debug, Clone)]
pub struct EthToZecTransfer {
    pub amount: u64, // TODO: use U256?
    pub pubkey_hash: [u8; 20],
}

#[derive(Debug, Clone)]
pub struct ZecToEthTransfer {
    pub amount: u64, // TODO: use U256?
    pub eth_address: [u8; 20],
}

#[derive(Debug, Clone)]
pub struct StateUpdate {
    pub old_eth_block: u64,
    pub new_eth_block: u64,
    pub old_eth_hash: [u8; 32],
    pub new_eth_hash: [u8; 32],
    pub old_zcash_block: u64,
    pub new_zcash_block: u64,
    pub old_zcash_hash: [u8; 32],
    pub new_zcash_hash: [u8; 32],
    pub eth_to_zec_transfers: Vec<EthToZecTransfer>,
    pub zec_to_eth_transfers: Vec<ZecToEthTransfer>,
}
