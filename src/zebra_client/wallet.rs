use bip0039::{English, Mnemonic};
use ripemd::{Digest, Ripemd160};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::Sha256;
use zcash_address::ZcashAddress;
use zcash_primitives::transaction::builder::{BuildConfig, Builder};
use zcash_protocol::consensus::{BlockHeight, NetworkType, Parameters};
use zcash_transparent::{address::TransparentAddress, keys::NonHardenedChildIndex};
use zip32::AccountId;

use super::regtest::{REGTEST_DEFAULT_SEED, REGTEST_NETWORK, RegtestNetwork};

/// A wallet for a given network.
pub struct Wallet<P: Parameters> {
    seed: [u8; 64],
    network_params: P,
}

/// Derived key for a given network.
pub struct Key {
    sk: SecretKey,
    network_type: NetworkType,
}

impl<P: Parameters> Wallet<P> {
    pub fn new(seed: [u8; 64], network_params: P) -> Self {
        Self {
            seed,
            network_params,
        }
    }

    pub fn from_mnemonic(mnemonic: &str, network_params: P) -> Self {
        let seed = Mnemonic::<English>::from_phrase(mnemonic)
            .unwrap()
            .to_seed("");
        Self::new(seed, network_params)
    }

    pub fn derive_key(&self, account_id: u32, address_index: u32) -> Key {
        let account = AccountId::try_from(account_id).unwrap();
        let sk = zcash_transparent::keys::AccountPrivKey::from_seed(
            &self.network_params,
            &self.seed,
            account,
        )
        .unwrap()
        .derive_external_secret_key(NonHardenedChildIndex::from_index(address_index).unwrap())
        .unwrap();
        Key::new(sk, self.network_params.network_type())
    }

    pub fn tx_builder<'b>(&'b self, target_height: u32) -> Builder<'b, P, ()> {
        Builder::new(
            self.network_params.clone(),
            BlockHeight::from_u32(target_height),
            BuildConfig::Standard {
                sapling_anchor: None,
                orchard_anchor: None,
            },
        )
    }
}

impl Default for Wallet<RegtestNetwork> {
    fn default() -> Self {
        Self::from_mnemonic(REGTEST_DEFAULT_SEED, REGTEST_NETWORK)
    }
}

/// Returns a default wallet for the Regtest network.
pub fn regtest_default_wallet() -> Wallet<RegtestNetwork> {
    Wallet::<RegtestNetwork>::default()
}

impl Key {
    pub fn new(sk: SecretKey, network_type: NetworkType) -> Self {
        Self { sk, network_type }
    }

    /// Returns the derived secret key.
    pub fn secret_key(&self) -> SecretKey {
        self.sk
    }

    /// Returns the public key for the derived key.
    pub fn public_key(&self) -> PublicKey {
        self.sk.public_key(&Secp256k1::new())
    }

    /// Returns the public key hash for the derived key.
    pub fn pubkey_hash(&self) -> [u8; 20] {
        let pubkey = self.public_key();
        Ripemd160::digest(Sha256::digest(pubkey.serialize())).into()
    }

    /// Returns the transparent address for the derived key.
    pub fn transparent_address(&self) -> TransparentAddress {
        let hash = self.pubkey_hash();
        TransparentAddress::PublicKeyHash(hash)
    }

    /// Returns the Zcash address for the derived key.
    pub fn address(&self) -> ZcashAddress {
        self.transparent_address()
            .to_zcash_address(self.network_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_miner_address() {
        let address = regtest_default_wallet().derive_key(0, 0).address();
        assert_eq!(address.encode(), "tmLTZegcJN5zaufWQBARHkvqC62mTumm3jR");
    }
}
