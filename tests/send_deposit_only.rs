//! Script to submit a TZE deposit to a local regtest zebrad node.

use zcash_eth_bridge::zcash::sender::TzeSender;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use zcash_protocol::value::Zatoshis;

#[tokio::test]
async fn deposit_tze() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let mut sender = TzeSender::new("127.0.0.1:18232").await?;

    // 1st address in anvil, corresponds to pk 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
    let deposit_eth_addr: [u8; 20] = hex::decode("70997970C51812dc3A010C7d01b50e0d17dc79C8")
        .unwrap()
        .try_into()
        .unwrap();
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

    Ok(())
}
