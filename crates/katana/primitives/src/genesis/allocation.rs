use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

use alloy_primitives::U256;
use derive_more::{Deref, DerefMut};
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use starknet::core::serde::unsigned_field_element::{UfeHex, UfeHexOption};
use starknet::core::utils::get_contract_address;
use starknet::signers::SigningKey;

use super::constant::DEFAULT_ACCOUNT_CLASS_HASH;
use crate::class::ClassHash;
use crate::contract::{ContractAddress, StorageKey, StorageValue};
use crate::{felt, Felt};

/// Represents a contract allocation in the genesis block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum GenesisAllocation {
    /// Account contract
    Account(GenesisAccountAlloc),
    /// Generic non-account contract
    Contract(GenesisContractAlloc),
}

impl GenesisAllocation {
    /// Get the public key of the account contract if it's an account contract, otherwise `None`.
    pub fn public_key(&self) -> Option<Felt> {
        match self {
            Self::Contract(_) => None,
            Self::Account(account) => Some(account.public_key()),
        }
    }

    /// Get the contract class hash.
    pub fn class_hash(&self) -> Option<ClassHash> {
        match self {
            Self::Contract(contract) => contract.class_hash,
            Self::Account(account) => Some(account.class_hash()),
        }
    }

    /// Get the balance to be allocated to this contract.
    pub fn balance(&self) -> Option<U256> {
        match self {
            Self::Contract(contract) => contract.balance,
            Self::Account(account) => account.balance(),
        }
    }

    /// Get the nonce.
    pub fn nonce(&self) -> Option<Felt> {
        match self {
            Self::Contract(contract) => contract.nonce,
            Self::Account(account) => account.nonce(),
        }
    }

    /// Get the storage values for this contract allocation.
    pub fn storage(&self) -> Option<&BTreeMap<StorageKey, StorageValue>> {
        match self {
            Self::Contract(contract) => contract.storage.as_ref(),
            Self::Account(account) => account.storage(),
        }
    }
}

/// Genesis allocation for account contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum GenesisAccountAlloc {
    /// Account contract with hidden private key.
    Account(GenesisAccount),
    /// Account contract with exposed private key.
    /// Suitable for printing to the console for development purposes.
    DevAccount(DevGenesisAccount),
}

impl GenesisAccountAlloc {
    pub fn public_key(&self) -> Felt {
        match self {
            Self::Account(account) => account.public_key,
            Self::DevAccount(account) => account.public_key,
        }
    }

    pub fn class_hash(&self) -> ClassHash {
        match self {
            Self::Account(account) => account.class_hash,
            Self::DevAccount(account) => account.class_hash,
        }
    }

    pub fn balance(&self) -> Option<U256> {
        match self {
            Self::Account(account) => account.balance,
            Self::DevAccount(account) => account.balance,
        }
    }

    pub fn nonce(&self) -> Option<Felt> {
        match self {
            Self::Account(account) => account.nonce,
            Self::DevAccount(account) => account.nonce,
        }
    }

    pub fn storage(&self) -> Option<&BTreeMap<StorageKey, StorageValue>> {
        match self {
            Self::Account(account) => account.storage.as_ref(),
            Self::DevAccount(account) => account.storage.as_ref(),
        }
    }

    pub fn private_key(&self) -> Option<Felt> {
        match self {
            Self::Account(_) => None,
            Self::DevAccount(account) => Some(account.private_key),
        }
    }
}

/// A generic non-account contract.
#[serde_with::serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenesisContractAlloc {
    /// The class hash of the contract.
    #[serde_as(as = "UfeHexOption")]
    pub class_hash: Option<ClassHash>,
    /// The amount of the fee token allocated to the contract.
    pub balance: Option<U256>,
    /// The initial nonce of the contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Felt>,
    /// The initial storage values of the contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<BTreeMap<StorageKey, StorageValue>>,
}

/// Used mainly for development purposes where the account info including the
/// private key is printed to the console.
#[serde_with::serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Deref, DerefMut)]
pub struct DevGenesisAccount {
    /// The private key associated with the public key of the account.
    #[serde_as(as = "UfeHex")]
    pub private_key: Felt,
    #[deref]
    #[deref_mut]
    #[serde(flatten)]
    /// The inner account contract.
    pub inner: GenesisAccount,
}

impl DevGenesisAccount {
    /// Creates a new dev account with the given `private_key` and `class_hash`.
    pub fn new(private_key: Felt, class_hash: ClassHash) -> Self {
        let public_key = public_key_from_private_key(private_key);
        Self { private_key, inner: GenesisAccount::new(public_key, class_hash) }
    }

    /// Creates a new dev account with the allocated `balance`.
    pub fn new_with_balance(private_key: Felt, class_hash: ClassHash, balance: U256) -> Self {
        let mut account = Self::new(private_key, class_hash);
        account.balance = Some(balance);
        account
    }

    pub fn address(&self) -> ContractAddress {
        self.inner.address()
    }
}

/// Account contract allocated in the genesis block.
#[serde_with::serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenesisAccount {
    /// The public key associated with the account for validation.
    #[serde_as(as = "UfeHex")]
    pub public_key: Felt,
    /// The class hash of the account contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: ClassHash,
    /// The amount of the fee token allocated to the account.
    pub balance: Option<U256>,
    /// The initial nonce of the account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Felt>,
    /// The initial storage values of the account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<BTreeMap<StorageKey, StorageValue>>,
    /// The salt that will be used to deploy this account.
    pub salt: Felt,
}

impl GenesisAccount {
    // Backward compatible reason
    pub const DEFAULT_SALT: Felt = felt!("666");

    pub fn new(public_key: Felt, class_hash: ClassHash) -> Self {
        Self::new_with_salt(public_key, class_hash, Self::DEFAULT_SALT)
    }

    pub fn new_with_salt(public_key: Felt, class_hash: ClassHash, salt: Felt) -> Self {
        Self { public_key, class_hash, salt, balance: None, nonce: None, storage: None }
    }

    pub fn new_with_balance(public_key: Felt, class_hash: ClassHash, balance: U256) -> Self {
        let mut account = Self::new(public_key, class_hash);
        account.balance = Some(balance);
        account
    }

    pub fn new_with_salt_and_balance(
        public_key: Felt,
        class_hash: ClassHash,
        salt: Felt,
        balance: U256,
    ) -> Self {
        let mut account = Self::new_with_salt(public_key, class_hash, salt);
        account.balance = Some(balance);
        account
    }

    /// Returns the address of this account.
    pub fn address(&self) -> ContractAddress {
        get_contract_address(self.salt, self.class_hash, &[self.public_key], Felt::ZERO).into()
    }
}

impl From<DevGenesisAccount> for GenesisAllocation {
    fn from(value: DevGenesisAccount) -> Self {
        Self::Account(GenesisAccountAlloc::DevAccount(value))
    }
}

/// A helper type for allocating dev accounts in the genesis block.
#[must_use]
#[derive(Debug)]
pub struct DevAllocationsGenerator {
    total: u16,
    seed: [u8; 32],
    balance: Option<U256>,
    class_hash: Felt,
}

impl DevAllocationsGenerator {
    /// Create a new dev account generator for `total` number of accounts.
    ///
    /// This will return a [DevAllocationsGenerator] with the default parameters.
    pub fn new(total: u16) -> Self {
        Self { total, seed: [0u8; 32], balance: None, class_hash: DEFAULT_ACCOUNT_CLASS_HASH }
    }

    pub fn with_class(self, class_hash: ClassHash) -> Self {
        Self { class_hash, ..self }
    }

    pub fn with_seed<T: Into<[u8; 32]>>(self, seed: T) -> Self {
        Self { seed: seed.into(), ..self }
    }

    pub fn with_balance<T: Into<U256>>(self, balance: T) -> Self {
        Self { balance: Some(balance.into()), ..self }
    }

    /// Generate `total` number of accounts based on the `seed`.
    #[must_use]
    pub fn generate(&self) -> HashMap<ContractAddress, DevGenesisAccount> {
        let mut seed = self.seed;
        (0..self.total)
            .map(|_| {
                let mut rng = SmallRng::from_seed(seed);
                let mut private_key_bytes = [0u8; 32];

                rng.fill_bytes(&mut private_key_bytes);
                private_key_bytes[0] %= 0x8;
                seed = private_key_bytes;

                let private_key = Felt::from_bytes_be(&private_key_bytes);

                let account = if let Some(amount) = self.balance {
                    DevGenesisAccount::new_with_balance(private_key, self.class_hash, amount)
                } else {
                    DevGenesisAccount::new(private_key, self.class_hash)
                };

                (account.address(), account)
            })
            .collect()
    }
}

/// Helper function for generating the public key from the `private_key` using
/// the Stark curve.
fn public_key_from_private_key(private_key: Felt) -> Felt {
    SigningKey::from_secret_scalar(private_key).verifying_key().scalar()
}
