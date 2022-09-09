mod event;

use near_contract_standards::fungible_token::core::FungibleTokenCore;
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::resolver::FungibleTokenResolver;
use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, LookupMap, UnorderedSet};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, log, near_bindgen, sys, AccountId, Balance, Gas, PanicOnDefault, PromiseOrValue,
};

use std::convert::TryFrom;

#[derive(
    BorshDeserialize, BorshSerialize, Clone, Copy, Eq, PartialEq, Debug, Serialize, Deserialize,
)]
#[serde(crate = "near_sdk::serde")]
pub enum BlackListStatus {
    // An address might be using
    Allowable,
    // All acts with an address have to be banned
    Banned,
}

#[derive(
    BorshDeserialize, BorshSerialize, Clone, Copy, Eq, PartialEq, Debug, Serialize, Deserialize,
)]
#[serde(crate = "near_sdk::serde")]
pub enum ContractStatus {
    Working,
    Paused,
}

impl std::fmt::Display for ContractStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractStatus::Working => write!(f, "working"),
            ContractStatus::Paused => write!(f, "paused"),
        }
    }
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    owner_id: AccountId,
    proposed_owner_id: AccountId,
    token: FungibleToken,
    metadata: LazyOption<FungibleTokenMetadata>,
    guardians: UnorderedSet<AccountId>,
    black_list: LookupMap<AccountId, BlackListStatus>,
    status: ContractStatus,
}

const DATA_IMAGE_SVG_NEAR_ICON: &str =
    "data:image/svg+xml,%3Csvg width='111' height='90' viewBox='0 0 111 90' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath fill-rule='evenodd' clip-rule='evenodd' d='M24.4825 0.862305H88.0496C89.5663 0.862305 90.9675 1.64827 91.7239 2.92338L110.244 34.1419C111.204 35.7609 110.919 37.8043 109.549 39.1171L58.5729 87.9703C56.9216 89.5528 54.2652 89.5528 52.6139 87.9703L1.70699 39.1831C0.305262 37.8398 0.0427812 35.7367 1.07354 34.1077L20.8696 2.82322C21.6406 1.60483 23.0087 0.862305 24.4825 0.862305ZM79.8419 14.8003V23.5597H61.7343V29.6329C74.4518 30.2819 83.9934 32.9475 84.0642 36.1425L84.0638 42.803C83.993 45.998 74.4518 48.6635 61.7343 49.3125V64.2168H49.7105V49.3125C36.9929 48.6635 27.4513 45.998 27.3805 42.803L27.381 36.1425C27.4517 32.9475 36.9929 30.2819 49.7105 29.6329V23.5597H31.6028V14.8003H79.8419ZM55.7224 44.7367C69.2943 44.7367 80.6382 42.4827 83.4143 39.4727C81.0601 36.9202 72.5448 34.9114 61.7343 34.3597V40.7183C59.7966 40.8172 57.7852 40.8693 55.7224 40.8693C53.6595 40.8693 51.6481 40.8172 49.7105 40.7183V34.3597C38.8999 34.9114 30.3846 36.9202 28.0304 39.4727C30.8066 42.4827 42.1504 44.7367 55.7224 44.7367Z' fill='%23009393'/%3E%3C/svg%3E";

#[near_bindgen]
impl Contract {
    // Initializes the contract with the given total supply owned by the given `owner_id` with
    // default metadata (for example purposes only).
    #[init]
    pub fn new_default_meta(owner_id: AccountId, total_supply: U128) -> Self {
        Self::new(
            owner_id,
            total_supply,
            FungibleTokenMetadata {
                spec: FT_METADATA_SPEC.to_string(),
                name: "Tether USD".to_string(),
                symbol: "USDt".to_string(),
                icon: Some(DATA_IMAGE_SVG_NEAR_ICON.to_string()),
                reference: None,
                reference_hash: None,
                decimals: 6,
            },
        )
    }

    // Initializes the contract with the given total supply owned by the given `owner_id` with
    // the given fungible token metadata.
    #[init]
    pub fn new(owner_id: AccountId, total_supply: U128, metadata: FungibleTokenMetadata) -> Self {
        metadata.assert_valid();
        let mut this = Self {
            owner_id: owner_id.clone(),
            proposed_owner_id: owner_id.clone(),
            token: FungibleToken::new(b"a".to_vec()),
            guardians: UnorderedSet::new(b"c".to_vec()),
            metadata: LazyOption::new(b"m".to_vec(), Some(&metadata)),
            black_list: LookupMap::new(b"b".to_vec()),
            status: ContractStatus::Working,
        };
        this.token.internal_register_account(&owner_id);
        this.token.internal_deposit(&owner_id, total_supply.into());
        event::emit::ft_mint(&owner_id, total_supply.into(), Some("Initial supply"));
        this
    }

    pub fn upgrade_name_symbol(&mut self, name: String, symbol: String) {
        self.abort_if_not_owner();
        let metadata = self.metadata.get();
        if let Some(mut metadata) = metadata {
            metadata.name = name;
            metadata.symbol = symbol;
            self.metadata.replace(&metadata);
        }
    }

    pub fn propose_new_owner(&mut self, proposed_owner_id: AccountId) {
        self.abort_if_not_owner();
        self.proposed_owner_id = proposed_owner_id;
    }

    pub fn accept_ownership(&mut self) {
        assert_ne!(self.owner_id, self.proposed_owner_id);
        assert_eq!(env::predecessor_account_id(), self.proposed_owner_id);
        self.owner_id = self.proposed_owner_id.clone();
    }

    /// Extend guardians. Only can be called by owner.
    pub fn extend_guardians(&mut self, guardians: Vec<AccountId>) {
        self.abort_if_not_owner();
        for guardian in guardians {
            if !self.guardians.insert(&guardian) {
                env::panic_str(&format!("The guardian '{}' already exists", guardian));
            }
        }
    }

    /// Remove guardians. Only can be called by owner.
    pub fn remove_guardians(&mut self, guardians: Vec<AccountId>) {
        self.abort_if_not_owner();
        for guardian in guardians {
            if !self.guardians.remove(&guardian) {
                env::panic_str(&format!("The guardian '{}' doesn't exist", guardian));
            }
        }
    }

    pub fn guardians(&self) -> Vec<AccountId> {
        self.guardians.to_vec()
    }

    pub fn upgrade_icon(&mut self, data: String) {
        self.abort_if_not_owner();
        let metadata = self.metadata.get();
        if let Some(mut metadata) = metadata {
            metadata.icon = Some(data);
            self.metadata.replace(&metadata);
        }
    }

    pub fn get_blacklist_status(&self, account_id: &AccountId) -> BlackListStatus {
        self.abort_if_pause();
        return match self.black_list.get(account_id) {
            Some(x) => x.clone(),
            None => BlackListStatus::Allowable,
        };
    }

    pub fn add_to_blacklist(&mut self, account_id: &AccountId) {
        self.abort_if_not_owner();
        self.abort_if_pause();
        self.black_list.insert(account_id, &BlackListStatus::Banned);
    }

    pub fn remove_from_blacklist(&mut self, account_id: &AccountId) {
        self.abort_if_not_owner();
        self.abort_if_pause();
        self.black_list
            .insert(account_id, &BlackListStatus::Allowable);
    }

    pub fn destroy_black_funds(&mut self, account_id: &AccountId) {
        self.abort_if_not_owner();
        self.abort_if_pause();

        assert_eq!(
            self.get_blacklist_status(&account_id),
            BlackListStatus::Banned
        );

        let black_balance = self.ft_balance_of(account_id.clone());

        self.burn(account_id, black_balance);
    }

    // Issue a new amount of tokens
    // these tokens are deposited into the owner address
    pub fn issue(&mut self, amount: U128) {
        self.abort_if_not_owner();
        self.mint(&self.owner_id.clone(), amount)
    }

    // Creates `amount` tokens and assigns them to `account`, increasing
    // the total supply.
    pub fn mint(&mut self, account_id: &AccountId, amount: U128) {
        self.abort_if_not_owner();
        self.abort_if_pause();

        self.token.internal_deposit(account_id, amount.into());
        event::emit::ft_mint(account_id, amount.into(), None);
    }

    // Redeem tokens (burn).
    // These tokens are withdrawn from the owner address
    // if the balance must be enough to cover the redeem
    // or the call will fail.
    pub fn redeem(&mut self, amount: U128) {
        self.abort_if_not_owner();
        self.burn(&self.owner_id.clone(), amount)
    }

    // Redeem tokens (burn).
    // These tokens are withdrawn from the owner address
    // if the balance must be enough to cover the redeem
    // or the call will fail.
    pub fn burn(&mut self, account_id: &AccountId, amount: U128) {
        self.abort_if_not_owner();
        self.abort_if_pause();

        self.token.internal_withdraw(account_id, amount.into());
        event::emit::ft_burn(account_id, amount.into(), None);
    }

    // If we have to pause contract
    pub fn pause(&mut self) {
        assert_eq!(self.status, ContractStatus::Working);
        self.abort_if_not_owner_or_guardian();
        self.status = ContractStatus::Paused;
    }

    // If we have to resume contract
    pub fn resume(&mut self) {
        assert_eq!(self.status, ContractStatus::Paused);
        self.abort_if_not_owner_or_guardian();
        self.status = ContractStatus::Working;
    }

    pub fn contract_status(&self) -> ContractStatus {
        self.status
    }

    /**
     * @dev Returns the name of the token.
     */
    pub fn name(&mut self) -> String {
        self.abort_if_pause();
        let metadata = self.metadata.get();
        metadata.expect("Unable to get name").name
    }

    /**
     * Returns the symbol of the token.
     */
    pub fn symbol(&mut self) -> String {
        self.abort_if_pause();
        let metadata = self.metadata.get();
        metadata.expect("Unable to get symbol").symbol
    }

    /**
     * Returns the decimals places of the token.
     */
    pub fn decimals(&mut self) -> u8 {
        self.abort_if_pause();
        let metadata = self.metadata.get();
        metadata.expect("Unable to get decimals").decimals
    }

    pub fn version(&self) -> String {
        format!("{}:{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
    }

    pub fn owner(&self) -> AccountId {
        self.owner_id.clone()
    }

    /// Should only be called by this contract on migration.
    /// This is NOOP implementation. KEEP IT if you haven't changed contract state.
    /// This method is called from `upgrade()` method.
    /// For next version upgrades, change this function.
    #[init(ignore_state)]
    #[private]
    pub fn migrate() -> Self {
        let this: Contract = env::state_read().expect("Contract is not initialized.");

        // Remove before the next upgrade. Log $1 from another transaction.
        event::emit::ft_mint(
            &this.owner_id,
            1000000,
            Some("From transaction: AMc2tZXY8kR8DAu9Z4hod1UD8vpp9mmgvukoUjDUycj2"),
        );

        this
    }

    fn abort_if_pause(&self) {
        if self.status == ContractStatus::Paused {
            env::panic_str("Operation aborted because the contract under maintenance")
        }
    }

    fn abort_if_not_owner(&self) {
        if env::predecessor_account_id() != self.owner_id {
            env::panic_str("This method might be called only by owner account")
        }
    }

    fn abort_if_not_owner_or_guardian(&self) {
        if env::predecessor_account_id() != self.owner_id
            && !self.guardians.contains(&env::predecessor_account_id())
        {
            env::panic_str("This method can be called only by owner or guardian")
        }
    }

    fn abort_if_blacklisted(&self, account_id: &AccountId) {
        if self.get_blacklist_status(account_id) != BlackListStatus::Allowable {
            env::panic_str(&format!("Account '{}' is banned", account_id));
        }
    }

    fn on_account_closed(&mut self, account_id: AccountId, balance: Balance) {
        log!("Closed @{} with {}", account_id, balance);
    }

    fn on_tokens_burned(&mut self, account_id: &AccountId, amount: Balance) {
        event::emit::ft_burn(&account_id, amount, None);
    }
}

#[no_mangle]
pub fn upgrade() {
    env::setup_panic_hook();

    let contract: Contract = env::state_read().expect("Contract is not initialized");
    contract.abort_if_not_owner();

    const MIGRATE_METHOD_NAME: &[u8; 7] = b"migrate";
    const UPGRADE_GAS_LEFTOVER: Gas = Gas(5_000_000_000_000);

    unsafe {
        // Load code into register 0 result from the input argument if factory call or from promise if callback.
        sys::input(0);
        // Create a promise batch to upgrade current contract with code from register 0.
        let promise_id = sys::promise_batch_create(
            env::current_account_id().as_bytes().len() as u64,
            env::current_account_id().as_bytes().as_ptr() as u64,
        );
        // Deploy the contract code from register 0.
        sys::promise_batch_action_deploy_contract(promise_id, u64::MAX, 0);
        // Call promise to migrate the state.
        // Batched together to fail upgrade if migration fails.
        sys::promise_batch_action_function_call(
            promise_id,
            MIGRATE_METHOD_NAME.len() as u64,
            MIGRATE_METHOD_NAME.as_ptr() as u64,
            0,
            0,
            0,
            (env::prepaid_gas() - env::used_gas() - UPGRADE_GAS_LEFTOVER).0,
        );
        sys::promise_return(promise_id);
    }
}

/// The core methods for a basic fungible token. Extension standards may be
/// added in addition to this macro.

#[near_bindgen]
impl FungibleTokenCore for Contract {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        self.abort_if_pause();
        self.abort_if_blacklisted(&env::predecessor_account_id());
        self.token.ft_transfer(receiver_id, amount, memo);
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.abort_if_pause();
        self.abort_if_blacklisted(&env::predecessor_account_id());
        self.token
            .ft_transfer_call(receiver_id.clone(), amount, memo, msg)
    }

    fn ft_total_supply(&self) -> U128 {
        self.abort_if_pause();
        self.token.ft_total_supply()
    }

    fn ft_balance_of(&self, account_id: AccountId) -> U128 {
        self.abort_if_pause();
        self.token.ft_balance_of(account_id)
    }
}

#[near_bindgen]
impl FungibleTokenResolver for Contract {
    #[private]
    fn ft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> U128 {
        let sender_id: AccountId = sender_id.into();
        let (used_amount, burned_amount) =
            self.token
                .internal_ft_resolve_transfer(&sender_id, receiver_id, amount);
        if burned_amount > 0 {
            self.on_tokens_burned(
                &AccountId::try_from(sender_id).expect("Couldn't validate sender address"),
                burned_amount,
            );
        }
        used_amount.into()
    }
}

near_contract_standards::impl_fungible_token_storage!(Contract, token, on_account_closed);

#[near_bindgen]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        self.metadata.get().unwrap()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, Balance};

    use super::*;

    const TOTAL_SUPPLY: Balance = 1_000_000_000_000_000;

    fn get_context(predecessor_account_id: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder
    }

    #[test]
    fn test_new() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let contract = Contract::new_default_meta(accounts(1).into(), TOTAL_SUPPLY.into());
        testing_env!(context.is_view(true).build());
        assert_eq!(contract.ft_total_supply().0, TOTAL_SUPPLY);
        assert_eq!(contract.ft_balance_of(accounts(1)).0, TOTAL_SUPPLY);
    }

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn test_default() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let _contract = Contract::default();
    }

    #[test]
    fn test_contract_status() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let contract = Contract::new_default_meta(accounts(2).into(), TOTAL_SUPPLY.into());
        assert_eq!(contract.contract_status(), ContractStatus::Working);
    }

    #[test]
    fn test_ownership() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(1).into(), TOTAL_SUPPLY.into());
        contract.propose_new_owner(accounts(2));
        assert_eq!(contract.owner_id, accounts(1));
        testing_env!(context.predecessor_account_id(accounts(2)).build());
        contract.accept_ownership();
        assert_eq!(contract.owner_id, accounts(2));
    }

    #[test]
    #[should_panic]
    fn test_extend_guardians_by_user() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(1).into(), TOTAL_SUPPLY.into());
        testing_env!(context.predecessor_account_id(accounts(2)).build());
        contract.extend_guardians(vec![accounts(3)]);
    }

    #[test]
    fn test_guardians() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(1).into(), TOTAL_SUPPLY.into());
        testing_env!(context.predecessor_account_id(accounts(1)).build());
        contract.extend_guardians(vec![accounts(2)]);
        assert!(contract.guardians.contains(&accounts(2)));
        contract.remove_guardians(vec![accounts(2)]);
        assert!(!contract.guardians.contains(&accounts(2)));
    }

    #[test]
    fn test_view_guardians() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(1).into(), TOTAL_SUPPLY.into());
        testing_env!(context.predecessor_account_id(accounts(1)).build());
        contract.extend_guardians(vec![accounts(2)]);
        assert_eq!(contract.guardians()[0], accounts(2));
        contract.remove_guardians(vec![accounts(2)]);
        assert_eq!(contract.guardians().len(), 0);
    }

    #[test]
    fn test_transfer() {
        let mut context = get_context(accounts(2));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(2).into(), TOTAL_SUPPLY.into());
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(contract.storage_balance_bounds().min.into())
            .predecessor_account_id(accounts(1))
            .build());
        // Paying for account registration, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(1)
            .predecessor_account_id(accounts(2))
            .build());
        let transfer_amount = TOTAL_SUPPLY / 3;
        contract.ft_transfer(accounts(1), transfer_amount.into(), None);

        testing_env!(context
            .storage_usage(env::storage_usage())
            .account_balance(env::account_balance())
            .is_view(true)
            .attached_deposit(0)
            .build());
        assert_eq!(
            contract.ft_balance_of(accounts(2)).0,
            (TOTAL_SUPPLY - transfer_amount)
        );
        assert_eq!(contract.ft_balance_of(accounts(1)).0, transfer_amount);
    }

    #[test]
    fn test_blacklist() {
        let mut context = get_context(accounts(2));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(2).into(), TOTAL_SUPPLY.into());
        testing_env!(context
            .storage_usage(env::storage_usage())
            .predecessor_account_id(accounts(2))
            .build());

        assert_eq!(
            contract.get_blacklist_status(&accounts(1)),
            BlackListStatus::Allowable
        );

        contract.add_to_blacklist(&accounts(1));
        assert_eq!(
            contract.get_blacklist_status(&accounts(1)),
            BlackListStatus::Banned
        );

        contract.remove_from_blacklist(&accounts(1));
        assert_eq!(
            contract.get_blacklist_status(&accounts(1)),
            BlackListStatus::Allowable
        );

        contract.token.internal_register_account(&accounts(1));
        contract
            .token
            .internal_deposit(&accounts(1), TOTAL_SUPPLY / 3);

        contract.add_to_blacklist(&accounts(1));
        let total_supply_before = contract.token.total_supply;

        contract.destroy_black_funds(&accounts(1));
        assert_ne!(total_supply_before, contract.token.total_supply);
    }

    #[test]
    #[should_panic]
    fn test_destroy_black_funds_panic() {
        let mut context = get_context(accounts(2));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(2).into(), TOTAL_SUPPLY.into());
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(contract.storage_balance_bounds().min.into())
            .predecessor_account_id(accounts(1))
            .current_account_id(accounts(1))
            .signer_account_id(accounts(1))
            .build());

        contract.issue(U128::from(1000));
        contract.add_to_blacklist(&accounts(1));
        contract.destroy_black_funds(&accounts(1));

        contract.issue(U128::from(1000));
        contract.remove_from_blacklist(&accounts(1));
        contract.destroy_black_funds(&accounts(1));
    }

    #[test]
    fn test_issuance() {
        let mut context = get_context(accounts(2));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(2).into(), TOTAL_SUPPLY.into());
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(contract.storage_balance_bounds().min.into())
            .predecessor_account_id(accounts(2))
            .current_account_id(accounts(1))
            .signer_account_id(accounts(2))
            .build());

        let previous_total_supply = contract.ft_total_supply().0;
        let previous_balance = contract.ft_balance_of(accounts(2)).0;
        let reissuance_balance: Balance = 1_234_567_891;
        contract.issue(U128::from(reissuance_balance));
        assert_eq!(
            previous_total_supply + reissuance_balance,
            contract.ft_total_supply().0
        );
        assert_eq!(
            previous_balance + reissuance_balance,
            contract.ft_balance_of(accounts(2)).0
        );
    }

    #[test]
    fn test_redeem() {
        let mut context = get_context(accounts(2));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(2).into(), TOTAL_SUPPLY.into());
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(contract.storage_balance_bounds().min.into())
            .predecessor_account_id(accounts(2))
            .build());

        let previous_total_supply = contract.ft_total_supply();
        let previous_balance = contract.ft_balance_of(accounts(1));
        let reissuance_balance: Balance = 1_234_567_891;
        contract.issue(U128::from(reissuance_balance));
        contract.redeem(U128::from(reissuance_balance));
        assert_eq!(previous_total_supply, contract.ft_total_supply());
        assert_eq!(previous_balance, contract.ft_balance_of(accounts(1)));
    }

    #[test]
    fn test_maintenance() {
        let mut context = get_context(accounts(2));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(2).into(), TOTAL_SUPPLY.into());
        contract.extend_guardians(vec![accounts(3)]);
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(contract.storage_balance_bounds().min.into())
            .predecessor_account_id(accounts(3))
            .current_account_id(accounts(1))
            .signer_account_id(accounts(3))
            .build());
        assert_eq!(contract.contract_status(), ContractStatus::Working);
        contract.pause();
        assert_eq!(contract.contract_status(), ContractStatus::Paused);
        contract.resume();
        assert_eq!(contract.contract_status(), ContractStatus::Working);
        contract.pause();
        let result = std::panic::catch_unwind(move || {
            let _ = contract.ft_total_supply();
        });
        assert!(result.is_err());
    }

    #[test]
    #[should_panic]
    fn test_contract_status_pause() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(1).into(), TOTAL_SUPPLY.into());
        contract.pause();
        assert_eq!(contract.contract_status(), ContractStatus::Paused);
        contract.pause();
    }

    #[test]
    #[should_panic]
    fn test_contract_status_resume() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(1).into(), TOTAL_SUPPLY.into());
        contract.resume();
    }
}
