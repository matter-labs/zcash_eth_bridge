use zcash_protocol::consensus::{BlockHeight, NetworkType, NetworkUpgrade, Parameters};

/// Marker struct for the regtest network.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub struct RegtestNetwork;

pub const REGTEST_NETWORK: RegtestNetwork = RegtestNetwork;

/// The default seed for the Regtest network.
/// Zcash address for (account_id=0, address_index=0) is "tmLTZegcJN5zaufWQBARHkvqC62mTumm3jR".
pub const REGTEST_DEFAULT_SEED: &str = "fabric dilemma shift time border road fork license among uniform early laundry caution deer stamp";

impl Parameters for RegtestNetwork {
    fn network_type(&self) -> NetworkType {
        NetworkType::Regtest
    }

    fn activation_height(&self, nu: NetworkUpgrade) -> Option<BlockHeight> {
        match nu {
            NetworkUpgrade::Overwinter => Some(BlockHeight::from(1)),
            NetworkUpgrade::Sapling => Some(BlockHeight::from(1)),
            NetworkUpgrade::Blossom => Some(BlockHeight::from(1)),
            NetworkUpgrade::Heartwood => Some(BlockHeight::from(1)),
            NetworkUpgrade::Canopy => Some(BlockHeight::from(1)),
            NetworkUpgrade::Nu5 => Some(BlockHeight::from(1)),
            NetworkUpgrade::Nu6 => Some(BlockHeight::from(1)),
            NetworkUpgrade::Nu6_1 => Some(BlockHeight::from(1)),
            #[cfg(zcash_unstable = "nu7")]
            NetworkUpgrade::Nu7 => Some(BlockHeight::from(1)),
            #[cfg(zcash_unstable = "zfuture")]
            NetworkUpgrade::ZFuture => Some(BlockHeight::from(3)),
        }
    }
}
