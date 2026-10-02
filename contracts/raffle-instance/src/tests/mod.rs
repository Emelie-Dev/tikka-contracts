//! Shared fixtures, helpers, and mock contracts for raffle-instance integration tests.
#![cfg(test)]

extern crate std;

use crate::*;
pub(crate) use raffle_shared::RaffleConfigBuilder;

pub(crate) fn init_bounds_config(
	env: &Env,
	payment_token: &Address,
	description: String,
	max_tickets: u32,
	ticket_price: i128,
	prize_amount: i128,
	prizes: soroban_sdk::Vec<u32>,
) -> RaffleConfig {
	RaffleConfig {
		description,
		end_time: 0,
		no_deadline: true,
		max_tickets,
		max_tickets_per_tx: max_tickets,
		min_tickets: 1,
		allow_multiple: true,
		ticket_price,
		payment_token: payment_token.clone(),
		prize_amount,
		prizes,
		randomness_source: RandomnessSource::Internal,
		oracle_address: None,
		oracle_public_key: None,
		protocol_fee_bp: 0,
		treasury_address: None,
		swap_router: None,
		tikka_token: None,
		unique_winners: false,
		metadata_hash: BytesN::from_array(env, &[72u8; 32]),
		claim_lockup_seconds: None,
		claim_expiry_seconds: None,
		swap_deadline_seconds: None,
		early_bird_ticket_percentage: 0,
		early_bird_discount_bp: 0,
		category: None,
		max_tickets_per_address: 0,
		prize_token: None,
		nft_contract: None,
		bundles: soroban_sdk::Vec::new(env),
	}
}
pub(crate) use crate::{RaffleInstance as Contract, RaffleInstanceClient as ContractClient};
use soroban_sdk::{
	contract, contractimpl,
	testutils::{Events, Ledger},
	token::StellarAssetClient,
	Address, BytesN, Env, String,
};

/// Assert the drawing lock was cleared from instance storage.
///
/// The lock is reset by writing `false` (rather than removing the key), so
/// this checks the stored value instead of key presence.
pub(crate) fn assert_drawing_lock_cleared(env: &Env, contract_id: &Address) {
	let is_set: bool = env.as_contract(contract_id, || {
		env.storage()
			.instance()
			.get(&crate::DataKey::DrawingLock)
			.unwrap_or(false)
	});
	assert!(!is_set, "DrawingLock must be cleared");
}

/// Register a fresh SAC and return its address plus its mint client.
pub fn create_token<'a>(env: &'a Env, admin: &Address) -> (Address, StellarAssetClient<'a>) {
	let payment_token = env
		.register_stellar_asset_contract_v2(admin.clone())
		.address();
	(
		payment_token.clone(),
		StellarAssetClient::new(env, &payment_token),
	)
}

/// `sha256` of a 32-byte pre-image — the commitment `submit_commit` stores.
pub fn sha256_bytes32(env: &Env, preimage: &[u8; 32]) -> BytesN<32> {
	env.crypto()
		.sha256(&soroban_sdk::Bytes::from_array(env, preimage))
		.into()
}

/// Assert the raffle's stored metadata hash matches the expected bytes.
pub fn assert_metadata_hash(client: &ContractClient<'_>, expected: &BytesN<32>) {
	let raffle = client.get_raffle();
	assert_eq!(raffle.metadata_hash, *expected);
}

/// Generate an address usable as the calling factory in `client.init`.
pub fn creator_factory_addr(env: &Env) -> Address {
	Address::generate(env)
}

/// Set up a quorum raffle in Drawing state with tickets sold and randomness requested.
pub fn setup_quorum_drawing_raffle<'a>(
	env: &'a Env,
	k: u32,
	oracles: &[Address],
) -> (ContractClient<'a>, Address, Address, u64) {
	// A previous draw in the same test env may have advanced the ledger past
	// `end_time`; rewind so `init` sees a future deadline.
	env.ledger().set_timestamp(0);

	let contract_id = env.register(Contract, ());
	let client = ContractClient::new(env, &contract_id);
	let factory = env.register(MockFactory, ());
	let admin = Address::generate(env);
	let creator = Address::generate(env);

	let token_admin = Address::generate(env);
	let (token_addr, token_mint) = create_token(env, &token_admin);
	token_mint.mint(&creator, &1_000_000);

	let mut oracle_vec = Vec::new(env);
	for oracle in oracles {
		oracle_vec.push_back(oracle.clone());
	}

	let config = RaffleConfig {
		description: String::from_str(env, "quorum raffle"),
		end_time: 1_000,
		no_deadline: false,
		max_tickets: 5,
		max_tickets_per_tx: 5,
		max_tickets_per_address: 0,
		min_tickets: 1,
		allow_multiple: true,
		ticket_price: MIN_TICKET_PRICE,
		payment_token: token_addr,
		prize_amount: MIN_TICKET_PRICE * 5,
		prizes: soroban_sdk::vec![env, 10000u32],
		randomness_source: RandomnessSource::Quorum(QuorumConfig {
			k,
			oracles: oracle_vec,
		}),
		oracle_address: None,
		oracle_public_key: None,
		protocol_fee_bp: 0,
		treasury_address: None,
		swap_router: None,
		tikka_token: None,
		metadata_hash: BytesN::from_array(env, &[55u8; 32]),
		claim_lockup_seconds: None,
		claim_expiry_seconds: None,
		swap_deadline_seconds: None,
		early_bird_ticket_percentage: 0,
		early_bird_discount_bp: 0,
		category: None,
		unique_winners: false,
		bundles: Vec::new(env),
		prize_token: None,
		nft_contract: None,
	};

	client.init(&factory, &admin, &creator, &config);
	client.deposit_prize();
	client.buy_tickets(&creator, &3);
	env.ledger().set_timestamp(1_000);
	client.finalize_raffle();

	let request_id: u64 = env.as_contract(&contract_id, || {
		env.storage()
			.instance()
			.get(&DataKey::RandomnessRequestId)
			.unwrap_or(0)
	});

	(client, contract_id, creator, request_id)
}

#[contract]
pub struct MockFactory;

#[contractimpl]
impl MockFactory {
	pub fn is_global_paused(_env: Env) -> bool {
		false
	}

	pub fn record_volume(_env: Env, _token: Address, _amount: i128) {}

	pub fn track_participant(_env: Env, _participant: Address) {}

	pub fn record_leaderboard_entry(
		_env: Env,
		_raffle_id: Address,
		_tickets: i128,
		_prize_amount: i128,
		_volume: i128,
	) {
	}
}

pub(crate) fn base_config(env: &Env, payment_token: &Address) -> RaffleConfig {
	RaffleConfigBuilder::new(env, payment_token.clone())
		.build()
		.expect("valid base raffle config")
}

pub(crate) fn setup_active_raffle(
	env: &Env,
) -> (
	ContractClient<'_>,
	Address,
	Address,
	Address,
	Address,
	StellarAssetClient<'_>,
) {
	env.mock_all_auths();
	env.ledger().set_timestamp(1_000);
	let contract_id = env.register(Contract, ());
	let client = ContractClient::new(env, &contract_id);
	let factory = env.register(MockFactory, ());
	let admin = Address::generate(env);
	let creator = Address::generate(env);
	let buyer = Address::generate(env);
	let (payment_token, token) = create_token(env, &Address::generate(env));
	token.mint(&creator, &1_000_000);
	token.mint(&buyer, &1_000_000);

	let mut config = base_config(env, &payment_token);
	config.max_tickets = 10;
	config.max_tickets_per_tx = 10;
	config.prize_amount = 10 * raffle_shared::constants::MIN_TICKET_PRICE;
	client.init(&factory, &admin, &creator, &config);
	client.deposit_prize();
	(client, admin, creator, buyer, factory, token)
}

pub(crate) fn init_bounds_env() -> (Env, Address, Address, Address, Address, Address) {
	let env = Env::default();
	env.mock_all_auths();
	env.ledger().set_timestamp(1_000);

	let contract_id = env.register(Contract, ());
	let factory = Address::generate(&env);
	let admin = Address::generate(&env);
	let creator = Address::generate(&env);
	let (payment_token, _) = create_token(&env, &Address::generate(&env));
	(env, contract_id, factory, admin, creator, payment_token)
}

pub mod admin;
pub mod budget;
pub mod claim;
pub mod claim_state;
pub mod draw;
pub mod fairness;
pub mod init;
pub mod invariants;
pub mod reentrancy;
pub mod tickets;
pub mod ttl;
