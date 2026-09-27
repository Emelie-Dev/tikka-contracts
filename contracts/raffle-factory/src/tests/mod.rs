#![cfg(test)]

use crate::*;
use raffle_shared::{
    constants::MAX_INTERNAL_RANDOMNESS_PRIZE_AMOUNT, DEFAULT_PAGE_LIMIT,
    MAX_PAGE_LIMIT, PaginationParams, RandomnessSource, RaffleConfigBuilder,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger, MockAuth, MockAuthInvoke},
    Address, BytesN, Env, FromVal, IntoVal, String, Symbol, Val, Vec as SdkVec,
};

pub fn assert_event(
    env: &Env,
    expected_contract: &Address,
    expected_topic: &str,
) {
    let events = env.events().all();
    let last = events.last().unwrap();
    assert_eq!(&last.0, expected_contract);
    assert_eq!(Symbol::from_val(env, &last.1.get(0).unwrap()), Symbol::new(env, "tikka"));
    assert_eq!(Symbol::from_val(env, &last.1.get(1).unwrap()), Symbol::new(env, expected_topic));
}

pub fn setup_factory(env: &Env) -> (RaffleFactoryClient<'_>, Address, Address) {
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let wasm_hash = BytesN::from_array(env, &[0u8; 32]);

    let contract_id = env.register(RaffleFactory, ());
    let client = RaffleFactoryClient::new(env, &contract_id);
    env.mock_all_auths();
    client.init_factory(&admin, &wasm_hash, &0u32, &treasury);
    client.set_creation_delay(&0u64);

    (client, admin, treasury)
}

pub fn test_raffle_config(env: &Env, payment_token: &Address) -> RaffleConfig {
    RaffleConfigBuilder::new(env, payment_token.clone())
        .description(String::from_str(env, "Test Raffle"))
        .max_tickets(10)
        .max_tickets_per_tx(10)
        .ticket_price(10_000)
        .prize_amount(10_000)
        .prizes(SdkVec::from_array(env, [10_000u32]))
        .metadata_hash(BytesN::from_array(env, &[1u8; 32]))
        .claim_lockup_seconds(0)
        .swap_deadline_seconds(0)
        .build()
        .expect("valid test raffle config")
}

pub fn create_raffles_via_factory(
    env: &Env,
    client: &RaffleFactoryClient<'_>,
    admin: &Address,
    treasury: &Address,
    creator: &Address,
    count: u32,
) -> SdkVec<Address> {
    use raffle_instance::RaffleInstanceClient;

    let factory_address = client.address.clone();
    let token_admin = Address::generate(env);
    let payment_token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let protocol_fee_bp: u32 = env.as_contract(&factory_address, || {
        env.storage()
            .persistent()
            .get(&DataKey::ProtocolFeeBP)
            .unwrap_or(0)
    });

    let mut addrs = SdkVec::new(env);
    for _ in 0..count {
        let mut config = test_raffle_config(env, &payment_token);
        config.protocol_fee_bp = protocol_fee_bp;
        config.treasury_address = Some(treasury.clone());

        let raffle_address = env.register(raffle_instance::RaffleInstance, ());
        RaffleInstanceClient::new(env, &raffle_address).init(
            &factory_address,
            admin,
            creator,
            &config,
        );

        env.as_contract(&factory_address, || {
            let stable_id: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::NextRaffleId)
                .unwrap_or(0u32);
            env.storage()
                .persistent()
                .set(&DataKey::RaffleById(stable_id), &raffle_address);
            env.storage()
                .persistent()
                .set(&DataKey::NextRaffleId, &(stable_id.saturating_add(1)));
            let live_count: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::RaffleCount)
                .unwrap_or(0u32)
                .saturating_add(1);
            env.storage()
                .persistent()
                .set(&DataKey::RaffleCount, &live_count);
        });

        addrs.push_back(raffle_address);
    }
    addrs
}

pub const ZERO_CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";

pub fn zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(env, ZERO_CONTRACT))
}

pub fn seed_raffles(env: &Env, factory_id: &Address, n: u32) -> SdkVec<Address> {
    let mut addrs = SdkVec::new(env);
    env.as_contract(factory_id, || {
        for i in 0..n {
            let addr = Address::generate(env);
            env.storage()
                .persistent()
                .set(&DataKey::RaffleById(i), &addr);
            addrs.push_back(addr);
        }
        env.storage().persistent().set(&DataKey::NextRaffleId, &n);
        env.storage().persistent().set(&DataKey::RaffleCount, &n);
    });
    addrs
}

pub fn seed_creator_index(env: &Env, factory_id: &Address, creator: &Address, addrs: &[Address]) {
    env.as_contract(factory_id, || {
        let mut v: SdkVec<Address> = SdkVec::new(env);
        for a in addrs {
            v.push_back(a.clone());
        }
        env.storage()
            .persistent()
            .set(&DataKey::CreatorRaffles(creator.clone()), &v);
    });
}

pub fn seed_category_index(env: &Env, factory_id: &Address, category: &str, addrs: &[Address]) {
    let cat = String::from_str(env, category);
    env.as_contract(factory_id, || {
        let mut v: SdkVec<Address> = SdkVec::new(env);
        for a in addrs {
            v.push_back(a.clone());
        }
        env.storage()
            .persistent()
            .set(&DataKey::CategoryRaffles(cat.clone()), &v);
    });
}

pub fn rate_limit_config(env: &Env, payment_token: &Address, desc: &str) -> RaffleConfig {
    RaffleConfigBuilder::new(env, payment_token.clone())
        .description(String::from_str(env, desc))
        .max_tickets(10)
        .max_tickets_per_tx(10)
        .ticket_price(10_000)
        .prize_amount(10_000)
        .prizes(SdkVec::from_array(env, [10_000u32]))
        .metadata_hash(BytesN::from_array(env, &[1u8; 32]))
        .build()
        .expect("valid rate limit config")
}

pub fn make_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin)
        .address()
}

pub fn recurring_config(_env: &Env, base: RaffleConfig) -> RecurringRaffleConfig {
    RecurringRaffleConfig {
        base_config: base,
        interval_seconds: 86_400,
        max_rounds: 3,
        auto_fund: false,
    }
}

pub fn valid_base_config(env: &Env, payment_token: &Address) -> RaffleConfig {
    RaffleConfigBuilder::new(env, payment_token.clone())
        .description(String::from_str(env, "Recurring Raffle"))
        .max_tickets(10)
        .max_tickets_per_tx(10)
        .ticket_price(10_000)
        .prize_amount(10_000)
        .prizes(SdkVec::from_array(env, [10_000u32]))
        .metadata_hash(BytesN::from_array(env, &[1u8; 32]))
        .build()
        .expect("valid base config")
}

pub mod budget;
pub mod governance;
pub mod init;
pub mod pagination;
pub mod raffles;
pub mod recurring;
pub mod registry;
pub mod views;
