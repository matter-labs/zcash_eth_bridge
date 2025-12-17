use zcash_primitives::block::BlockHash;
use zcash_protocol::{TxId, consensus::BranchId};
use zebra_chain::{
    block::Block,
    serialization::{ZcashDeserialize, ZcashSerialize},
    transparent::MIN_TRANSPARENT_COINBASE_MATURITY,
};
use zebra_node_services::rpc_client::RpcRequestClient;
use zebra_rpc::methods::GetBlockResponse;

use super::client::RpcClient;

/// Converts a transaction hash in RPC format (reversed) into byte format.
pub fn txid_from_rpc_string(hex_string: &str) -> Result<TxId, anyhow::Error> {
    let bytes_rev = hex::decode(hex_string)
        .unwrap()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    TxId::read(&bytes_rev[..]).map_err(|e| anyhow::anyhow!("failed to parse txid: {}", e))
}

/// Finds the first spendable coinbase transaction ID given the current block count.
pub async fn spendable_coinbase_txid(
    client: &RpcRequestClient,
    target_height: u32,
) -> Result<TxId, anyhow::Error> {
    if target_height < MIN_TRANSPARENT_COINBASE_MATURITY {
        panic!(
            "At height {target_height} there are no spendable coinbase transactions, minimum maturity is {MIN_TRANSPARENT_COINBASE_MATURITY}. \
            Please wait a bit until zebra mines more blocks and run the binary again."
        );
    }

    let get_block_hash_res = client
        .get_block_hash(target_height - MIN_TRANSPARENT_COINBASE_MATURITY)
        .await?;
    let block_hash = BlockHash::from_slice(&get_block_hash_res.hash().0);
    let block = client.get_block(&block_hash).await?;
    match block {
        GetBlockResponse::Raw(block_bytes) => {
            let block = Block::zcash_deserialize(&mut block_bytes.as_ref())?;
            let coinbase_txid = TxId::from_bytes(block.transactions[0].hash().0);
            Ok(coinbase_txid)
        }
        _ => anyhow::bail!("expected raw block"),
    }
}

pub fn tx_convert_librustzcash_to_zebra(
    tx: &zcash_primitives::transaction::Transaction,
) -> zebra_chain::transaction::Transaction {
    let mut tx_data = Vec::new();
    tx.write(&mut tx_data).unwrap();
    zebra_chain::transaction::Transaction::zcash_deserialize(&mut tx_data.as_slice()).unwrap()
}

pub fn tx_convert_zebra_to_librustzcash(
    tx: &zebra_chain::transaction::Transaction,
) -> zcash_primitives::transaction::Transaction {
    let tx_data = tx.zcash_serialize_to_vec().unwrap();
    let mut reader = std::io::Cursor::new(tx_data);
    zcash_primitives::transaction::Transaction::read(&mut reader, BranchId::ZFuture).unwrap()
}
