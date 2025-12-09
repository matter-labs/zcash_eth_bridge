use async_trait::async_trait;
use std::collections::HashSet;
use tracing::{debug, info, warn};
use zcash_primitives::{
    block::BlockHash,
    transaction::{Transaction, TxId},
};
use zcash_protocol::consensus::BranchId;
use zebra_node_services::rpc_client::RpcRequestClient;
use zebra_rpc::methods::{
    GetAddressUtxosRequest, GetAddressUtxosResponse, GetBlockHashResponse, GetBlockResponse,
    GetRawTransactionResponse, SendRawTransactionResponse, Utxo,
};

#[async_trait]
pub trait RpcClient {
    async fn send_raw_transaction(
        &self,
        transaction: &Transaction,
    ) -> Result<SendRawTransactionResponse, anyhow::Error>;

    async fn get_raw_transaction(
        &self,
        txid: &TxId,
        verbose: bool,
    ) -> Result<GetRawTransactionResponse, anyhow::Error>;

    async fn get_transaction(
        &self,
        txid: &TxId,
        branch_id: BranchId,
    ) -> Result<Transaction, anyhow::Error> {
        let tx = self.get_raw_transaction(txid, true).await?;
        match tx {
            GetRawTransactionResponse::Raw(tx) => Ok(Transaction::read(tx.as_ref(), branch_id)?),
            GetRawTransactionResponse::Object(tx) => {
                Ok(Transaction::read(tx.hex().as_ref(), branch_id)?)
            }
        }
    }

    async fn get_block_count(&self) -> Result<u32, anyhow::Error>;
    async fn get_block_hash(&self, height: u32) -> Result<GetBlockHashResponse, anyhow::Error>;
    async fn get_block(&self, hash: &BlockHash) -> Result<GetBlockResponse, anyhow::Error>;
    async fn get_address_utxos(&self, address: String) -> Result<Vec<Utxo>, anyhow::Error>;

    /// Get up-to-date UTXOs for an address, including mempool transactions.
    ///
    /// This method combines data from getaddressutxos and getrawmempool to provide
    /// a current view of UTXOs that accounts for unconfirmed transactions.
    ///
    /// Returns the same type as `get_address_utxos` but with mempool data incorporated:
    /// - Confirmed UTXOs spent by mempool transactions are excluded
    /// - New UTXOs created by mempool transactions are included (with height = 0 as marker)
    ///
    /// Uses `BranchId::ZFuture` for transaction parsing (suitable for custom testnets with experimental features).
    async fn get_address_utxos_with_mempool(
        &self,
        address: String,
    ) -> Result<Vec<Utxo>, anyhow::Error>;
}

#[async_trait]
impl RpcClient for RpcRequestClient {
    async fn send_raw_transaction(
        &self,
        transaction: &Transaction,
    ) -> Result<zebra_rpc::methods::SendRawTransactionResponse, anyhow::Error> {
        let mut tx_data = Vec::new();
        transaction.write(&mut tx_data)?;
        let tx_data_hex = hex::encode(tx_data);
        self.json_result_from_call("sendrawtransaction", format!(r#"["{tx_data_hex}"]"#))
            .await
            .map_err(|e| anyhow::anyhow!("failed to send transaction: {:?}", e))
    }

    async fn get_raw_transaction(
        &self,
        txid: &TxId,
        verbose: bool,
    ) -> Result<zebra_rpc::methods::GetRawTransactionResponse, anyhow::Error> {
        let verbose = if verbose { 1 } else { 0 };
        let txid_hex = txid.to_string();
        self.json_result_from_call("getrawtransaction", format!(r#"["{txid_hex}", {verbose}]"#))
            .await
            .map_err(|e| anyhow::anyhow!("failed to get raw transaction: {:?}", e))
    }

    async fn get_block_count(&self) -> Result<u32, anyhow::Error> {
        self.json_result_from_call("getblockcount", "[]".to_string())
            .await
            .map_err(|e| anyhow::anyhow!("failed to get block count: {}", e))
    }

    async fn get_block_hash(&self, height: u32) -> Result<GetBlockHashResponse, anyhow::Error> {
        self.json_result_from_call("getblockhash", format!(r#"[{height}]"#))
            .await
            .map_err(|e| anyhow::anyhow!("failed to get block hash: {}", e))
    }

    async fn get_block(&self, hash: &BlockHash) -> Result<GetBlockResponse, anyhow::Error> {
        let block_hash_hex = hash.to_string();
        self.json_result_from_call("getblock", format!(r#"["{block_hash_hex}", 0]"#))
            .await
            .map_err(|e| anyhow::anyhow!("failed to get block: {}", e))
    }

    async fn get_address_utxos(&self, address: String) -> Result<Vec<Utxo>, anyhow::Error> {
        let request = GetAddressUtxosRequest::new(vec![address], false);
        let request_json = serde_json::to_string(&request)
            .map_err(|e| anyhow::anyhow!("failed to serialize request: {}", e))?;
        let params = format!("[{}]", request_json);
        let response: GetAddressUtxosResponse = self
            .json_result_from_call("getaddressutxos", params)
            .await
            .map_err(|e| anyhow::anyhow!("failed to get address utxos: {}", e))?;

        let utxos = match response {
            GetAddressUtxosResponse::Utxos(utxos) => utxos,
            GetAddressUtxosResponse::UtxosAndChainInfo(response) => response.utxos().clone(),
        };

        Ok(utxos)
    }

    async fn get_address_utxos_with_mempool(
        &self,
        address: String,
    ) -> Result<Vec<Utxo>, anyhow::Error> {
        use zebra_chain::block::Height;
        use zebra_chain::transparent;

        // Use ZFuture branch ID for experimental features (TZE, etc.)
        let branch_id = BranchId::ZFuture;

        // Step 1: Get confirmed UTXOs for the address
        let mut confirmed_utxos = self.get_address_utxos(address.clone()).await?;

        // Step 2: Get all transaction IDs in the mempool
        let mempool_tx_ids: Vec<String> = self
            .json_result_from_call("getrawmempool", "[false]".to_string())
            .await
            .map_err(|e| anyhow::anyhow!("failed to get raw mempool: {}", e))?;

        info!(
            "Found {} confirmed UTXOs, {} mempool transactions",
            confirmed_utxos.len(),
            mempool_tx_ids.len()
        );

        // Step 3: Track which confirmed UTXOs are spent by mempool transactions
        let mut spent_outpoints = HashSet::new();
        let mut mempool_utxos = Vec::new();

        // Parse the target address once
        let target_address: transparent::Address = address
            .parse()
            .map_err(|e| anyhow::anyhow!("failed to parse address: {:?}", e))?;

        // Process each mempool transaction
        for tx_id_hex in mempool_tx_ids {
            // Fetch the transaction details directly using the hex string from getrawmempool
            // Call getrawtransaction with verbose=1 for mempool compatibility
            let response: Result<zebra_rpc::methods::GetRawTransactionResponse, _> = self
                .json_result_from_call("getrawtransaction", format!(r#"["{}", 1]"#, tx_id_hex))
                .await;

            let tx = match response {
                Ok(GetRawTransactionResponse::Object(tx_obj)) => {
                    // Parse the hex into a transaction
                    match Transaction::read(tx_obj.hex().as_ref(), branch_id) {
                        Ok(tx) => tx,
                        Err(e) => {
                            warn!("Failed to parse transaction {}: {:?}", tx_id_hex, e);
                            continue;
                        }
                    }
                }
                Ok(GetRawTransactionResponse::Raw(_)) => {
                    warn!(" Unexpected raw response for transaction {}", tx_id_hex);
                    continue;
                }
                Err(e) => {
                    warn!("Failed to fetch mempool transaction {}: {:?}", tx_id_hex, e);
                    continue;
                }
            };

            // Check if this transaction spends any of the confirmed UTXOs
            for input in tx.transparent_bundle().iter().flat_map(|b| b.vin.iter()) {
                let outpoint = input.prevout();
                // Note: outpoint.hash() returns the bytes in internal order
                // We need to compare with utxo.txid() which is also in internal order
                let outpoint_hash_bytes = outpoint.hash();
                let outpoint_index = outpoint.n();

                // Check if this input spends any of our confirmed UTXOs
                for utxo in &confirmed_utxos {
                    // Compare the raw bytes directly
                    let utxo_hash_bytes = &utxo.txid().0;
                    let utxo_index = utxo.output_index().index();

                    if outpoint_hash_bytes == utxo_hash_bytes && outpoint_index == utxo_index {
                        let utxo_key = format!(
                            "{}:{}",
                            hex::encode(utxo.txid().0),
                            utxo.output_index().index()
                        );
                        debug!("Mempool tx {} spends UTXO {}", tx_id_hex, utxo_key);
                        spent_outpoints.insert(utxo_key);
                    }
                }
            }

            // Check if this transaction creates outputs to the address
            if let Some(bundle) = tx.transparent_bundle() {
                for (index, output) in bundle.vout.iter().enumerate() {
                    // Check if the output is to our target address
                    if let Some(addr) = output.recipient_address() {
                        // Convert TransparentAddress to zebra_chain::transparent::Address for comparison
                        let addr_zebra: transparent::Address = match addr {
                            zcash_transparent::address::TransparentAddress::PublicKeyHash(hash) => {
                                transparent::Address::from_pub_key_hash(
                                    zebra_chain::parameters::Network::new_default_testnet().kind(),
                                    hash,
                                )
                            }
                            zcash_transparent::address::TransparentAddress::ScriptHash(hash) => {
                                transparent::Address::from_script_hash(
                                    zebra_chain::parameters::Network::new_default_testnet().kind(),
                                    hash,
                                )
                            }
                        };

                        if addr_zebra == target_address {
                            // Parse transaction hash from hex
                            let tx_hash_bytes = hex::decode(&tx_id_hex)
                                .map_err(|e| anyhow::anyhow!("failed to decode txid hex: {}", e))?;
                            let mut tx_hash_array = [0u8; 32];
                            tx_hash_array.copy_from_slice(&tx_hash_bytes);
                            let tx_hash = zebra_chain::transaction::Hash::from(tx_hash_array);

                            // Convert script from zcash_primitives to zebra_chain
                            let zebra_script =
                                transparent::Script::from(output.script_pubkey().clone());

                            // Create a Utxo with height 0 to indicate mempool transaction
                            mempool_utxos.push(Utxo::new(
                                target_address.clone(),
                                tx_hash,
                                zebra_chain::transparent::OutputIndex::from_usize(index),
                                zebra_script,
                                output.value().into(),
                                Height(0), // Height 0 indicates unconfirmed/mempool
                            ));
                        }
                    }
                }
            }
        }

        // Step 4: Filter out spent UTXOs from confirmed UTXOs
        let original_count = confirmed_utxos.len();
        confirmed_utxos.retain(|utxo| {
            let utxo_key = format!(
                "{}:{}",
                hex::encode(utxo.txid().0),
                utxo.output_index().index()
            );
            let is_spent = spent_outpoints.contains(&utxo_key);
            if is_spent {
                debug!("Filtering out spent UTXO: {}", utxo_key);
            }
            !is_spent
        });
        let filtered_count = original_count - confirmed_utxos.len();
        info!(
            "Filtered {} spent UTXOs, {} mempool UTXOs created, {} total UTXOs",
            filtered_count,
            mempool_utxos.len(),
            confirmed_utxos.len() + mempool_utxos.len()
        );

        // Step 5: Combine confirmed and mempool UTXOs
        confirmed_utxos.extend(mempool_utxos);

        Ok(confirmed_utxos)
    }
}
