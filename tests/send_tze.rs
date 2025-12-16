//! Script to mine some blocks on a local regtest zebrad node.

use zcash_eth_bridge::zcash::sender::TzeSender;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use zcash_extensions::transparent::eth_bridge;
use zcash_protocol::value::Zatoshis;

#[tokio::test]
async fn send_tze() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let mut sender = TzeSender::new("127.0.0.1:18232").await?;
    // sender.send_simple_tx().await?;
    let (create_outpoint, create_tze_output) = sender.send_tze_create(50_000).await?;
    tracing::info!(
        "[tze create] hash: {}, output: {:?}",
        create_outpoint.txid(),
        create_tze_output
    );
    sender.wait_for_tx(create_outpoint.txid()).await?;

    let deposit_eth_addr = [0xAB; 20];
    let deposit_amount = 90_000;
    let (deposit_outpoint, deposit_tze_output) = sender
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
    sender.wait_for_tx(deposit_outpoint.txid()).await?;

    let (stf_init_outpoint, stf_tze_output) = sender
        .initialize_tze_stf(50_000, (create_outpoint, create_tze_output))
        .await?;
    tracing::info!(
        "[tze stf init] hash: {}, output: {:?}",
        stf_init_outpoint.txid(),
        stf_tze_output
    );
    sender.wait_for_tx(stf_init_outpoint.txid()).await?;

    let processed_deposit = eth_bridge::modes::stf::ProcessedDeposit {
        to: deposit_eth_addr,
        amount: Zatoshis::const_from_u64(deposit_amount),
    };
    let (stf_progress_outpoint, _stf_tze_output) = sender
        .progress_tze_stf(
            50_000,
            (stf_init_outpoint, stf_tze_output),
            vec![(deposit_outpoint, deposit_tze_output)],
            vec![processed_deposit],
            Vec::new(),
        )
        .await?;
    tracing::info!(
        "[tze stf progress] hash: {}, output: {:?}",
        stf_progress_outpoint.txid(),
        _stf_tze_output
    );
    sender.wait_for_tx(stf_progress_outpoint.txid()).await?;

    Ok(())
}
