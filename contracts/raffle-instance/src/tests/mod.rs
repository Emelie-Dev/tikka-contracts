#![cfg(test)]

extern crate std;
use std::vec;

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
	RaffleConfigBuilder::new(env, payment_token.clone())
		.description(description)
		.max_tickets(max_tickets)
		.max_tickets_per_tx(max_tickets)
		.ticket_price(ticket_price)
		.prize_amount(prize_amount)
		.prizes(prizes)
		.metadata_hash(BytesN::from_array(env, &[72u8; 32]))
		.build()
		.unwrap()
}
pub(crate) use crate::{RaffleInstance as Contract, RaffleInstanceClient as ContractClient};
use soroban_sdk::{
	contract, contractimpl,
	testutils::{budget::Budget, Address as _, Events, Ledger, Register},
	token::StellarAssetClient,
	Address, BytesN, Env, String,
};
use crate::events;

#[contract]
pub struct MockFactory;

#[contractimpl]
impl MockFactory {
	pub fn is_global_paused(_env: Env) -> bool {
		false
	}

	pub fn record_volume(_env: Env, _token: Address, _amount: i128) {}

	pub fn track_participant(_env: Env, _participant: Address) {}
}

pub(crate) fn create_token<'a>(
	env: &'a Env,
	admin: &Address,
) -> (Address, StellarAssetClient<'a>) {
	let payment_token = env
		.register_stellar_asset_contract_v2(admin.clone())
		.address();
	(
		payment_token.clone(),
		StellarAssetClient::new(env, &payment_token),
	)
}

pub(crate) fn base_config(env: &Env, payment_token: &Address) -> RaffleConfig {
	RaffleConfigBuilder::new(env, payment_token.clone())
		.build()
		.expect("valid base raffle config")
}

pub(crate) fn setup_instance(
	env: &Env,
) -> (
	ContractClient<'_>,
	Address,
	Address,
	Address,
	Address,
	Address,
) {
	let contract_id = env.register(Contract, ());
	let client = ContractClient::new(env, &contract_id);
	let factory = Address::generate(env);
	let admin = Address::generate(env);
	let creator = Address::generate(env);
	let (payment_token, _) = create_token(env, &Address::generate(env));
	(client, contract_id, factory, admin, creator, payment_token)
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

pub(crate) fn creator_factory_addr(env: &Env) -> Address {
	Address::generate(env)
}

pub(crate) fn assert_drawing_lock_cleared(env: &Env, contract_id: &Address) {
	env.as_contract(contract_id, || {
		assert!(!env.storage().instance().has(&DataKey::DrawingLock));
	});
}

pub(crate) fn assert_metadata_hash(client: &ContractClient<'_>, expected: &BytesN<32>) {
	assert_eq!(client.get_raffle().metadata_hash, *expected);
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



pub mod budget;
pub mod claim;
pub mod claim_state;
pub mod draw;
pub mod fairness;
pub mod init;
pub mod invariants;
pub mod tickets;
pub mod ttl;
pub mod claim_state;
pub mod claim;
pub mod init;
pub mod admin;
pub mod tickets;
pub mod reentrancy;
