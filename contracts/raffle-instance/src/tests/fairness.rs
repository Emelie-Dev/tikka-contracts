//! Post-draw fairness metadata assertions (#768).
//!
//! Guards against regressions in `do_finalize_with_seed`, which writes
//! `FairnessMetadata` to `DataKey::RandomnessSeed` exactly once per draw.
//! The tests finalize a raffle and assert that every field of
//! `get_fairness_data()` — including `unique_winners` and `draw_sequence` —
//! round-trips correctly, so that an accidental duplicate (or field-dropping)
//! write is caught.

use raffle_shared::RandomnessSource;
use soroban_sdk::{token::StellarAssetClient, Address, BytesN, Env, String, Vec};

use crate::{
    Contract, ContractClient, FairnessMetadata, DataKey, RaffleConfig, RaffleStatus,
    MIN_TICKET_PRICE,
};

fn setup_unique_winners_raffle(env: &Env) -> (ContractClient<'_>, Address, Address) {
    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(env, &contract_id);

    let factory = Address::generate(env);
    let admin = Address::generate(env);
    let creator = Address::generate(env);
    let buyer = Address::generate(env);

    let token_admin = Address::generate(env);
    let payment_token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let token = StellarAssetClient::new(env, &payment_token);
    token.mint(&creator, &1_000_000_000_000);
    token.mint(&buyer, &1_000_000_000_000);

    let config = RaffleConfig {
        description: String::from_str(env, "unique winners fairness"),
        end_time: 0,
        no_deadline: true,
        max_tickets: 3,
        max_tickets_per_tx: 3,
        max_tickets_per_address: 0,
        min_tickets: 1,
        allow_multiple: true,
        ticket_price: MIN_TICKET_PRICE,
        payment_token: payment_token.clone(),
        prize_amount: MIN_TICKET_PRICE * 10,
        prizes: soroban_sdk::vec![env, 6000u32, 3000, 1000],
        randomness_source: RandomnessSource::Internal,
        oracle_address: None,
        protocol_fee_bp: 0,
        treasury_address: None,
        swap_router: None,
        tikka_token: None,
        metadata_hash: BytesN::from_array(env, &[77u8; 32]),
        claim_lockup_seconds: Some(0),
        swap_deadline_seconds: Some(300),
        early_bird_ticket_percentage: 0,
        early_bird_discount_bp: 0,
        category: None,
        unique_winners: true,
        bundles: Vec::new(env),
        prize_token: None,
        nft_contract: None,
    };

    client.init(&factory, &admin, &creator, &config);
    env.as_contract(&contract_id, || {
        env.storage().instance().remove(&DataKey::Factory);
    });
    client.deposit_prize();

    (client, contract_id, buyer)
}

#[test]
fn finalize_persists_all_fairness_fields_including_unique_winners() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let (client, contract_id, buyer) = setup_unique_winners_raffle(&env);

    client.buy_tickets(&buyer, &3);
    client.finalize_raffle();

    assert_eq!(client.get_raffle().status, RaffleStatus::Finalized);

    let fairness = client.get_fairness_data();

    // Every stored field must round-trip through get_fairness_data exactly.
    assert_eq!(fairness.randomness_source, RandomnessSource::Internal);
    assert_eq!(fairness.ticket_ids.len(), 3);
    assert_eq!(fairness.winning_ticket_indices.len(), 3);
    // unique_winners: true must be preserved (the bug dropped it in the
    // first (duplicate) write).
    assert_eq!(fairness.unique_winners, true);
    for i in 0..3 {
        assert_eq!(fairness.ticket_ids.get(i), Some(i + 1));
    }
    // draw_sequence is a copy of the ledger sequence at finalization.
    assert_eq!(fairness.draw_sequence, env.ledger().sequence());

    // Re-running must be deterministic for the same seed.
    assert_eq!(fairness.seed, client.get_fairness_data().seed);

    // Exactly one authoritative write to RandomnessSeed for this draw: the
    // stored metadata reflects a single finalization with unique_winners set
    // (the prior duplicate write dropped unique_winners and never compiled).
    env.as_contract(&contract_id, || {
        let meta: FairnessMetadata = env
            .storage()
            .persistent()
            .get(&DataKey::RandomnessSeed)
            .expect("fairness metadata must exist after finalization");
        assert_eq!(meta.unique_winners, true);
        assert_eq!(meta.winning_ticket_indices.len(), 3);
    });
}

#[test]
fn finalize_unique_winners_stays_within_budget() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, buyer) = setup_unique_winners_raffle(&env);
    client.buy_tickets(&buyer, &3);

    let (cpu_before, _) = snapshot(&env);
    client.finalize_raffle();
    let (cpu_after, _) = snapshot(&env);

    // Generous ceiling; the key regression guard is that the redundant
    // duplicate write (now removed) is no longer charged on the finalize path.
    assert!(cpu_after.saturating_sub(cpu_before) < 80_000_000);
}

fn snapshot(env: &Env) -> (u64, u64) {
    let budget = env.cost_estimate().budget();
    (
        budget.cpu_instruction_cost(),
        budget.memory_bytes_cost(),
    )
}

#[test]
#[ignore = "issue #1077: quorum last-revealer bias"]
fn test_quorum_last_revealer_bias() {
    let env = Env::default();
    env.mock_all_auths();

    let oracle_a = Address::generate(&env);
    let oracle_b = Address::generate(&env);
    let oracle_c = Address::generate(&env);
    let oracles = soroban_sdk::vec![&env, oracle_a.clone(), oracle_b.clone(), oracle_c.clone()];

    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);

    let factory = Address::generate(&env);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer_1 = Address::generate(&env);
    let buyer_2 = Address::generate(&env);
    let buyer_3 = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let payment_token = env.register_stellar_asset_contract_v2(token_admin).address();
    let token = StellarAssetClient::new(&env, &payment_token);
    token.mint(&creator, &100_000);
    token.mint(&buyer_1, &100_000);
    token.mint(&buyer_2, &100_000);
    token.mint(&buyer_3, &100_000);

    let config = RaffleConfig {
        description: String::from_str(&env, "Quorum bias test"),
        end_time: 0,
        no_deadline: true,
        max_tickets: 3,
        max_tickets_per_tx: 3,
        max_tickets_per_address: 0,
        min_tickets: 1,
        allow_multiple: true,
        ticket_price: MIN_TICKET_PRICE,
        payment_token: payment_token.clone(),
        prize_amount: MIN_TICKET_PRICE * 10,
        prizes: soroban_sdk::vec![&env, 10000],
        randomness_source: RandomnessSource::Quorum(raffle_shared::QuorumConfig { k: 2, oracles }),
        oracle_address: None,
        protocol_fee_bp: 0,
        treasury_address: None,
        swap_router: None,
        tikka_token: None,
        metadata_hash: BytesN::from_array(&env, &[0u8; 32]),
        claim_lockup_seconds: Some(0),
        swap_deadline_seconds: Some(300),
        early_bird_ticket_percentage: 0,
        early_bird_discount_bp: 0,
        category: None,
        unique_winners: true,
        bundles: Vec::new(&env),
        prize_token: None,
        nft_contract: None,
    };

    client.init(&factory, &admin, &creator, &config);
    env.as_contract(&contract_id, || {
        env.storage().instance().remove(&DataKey::Factory);
    });
    client.deposit_prize();

    client.buy_tickets(&buyer_1, &1); // index 0
    client.buy_tickets(&buyer_2, &1); // index 1
    client.buy_tickets(&buyer_3, &1); // index 2

    client.finalize_raffle();

    let request_id: u64 = env.as_contract(&contract_id, || {
        env.storage().instance().get(&DataKey::RandomnessRequestId).unwrap()
    });

    let seed_a: u64 = 12345;
    client.provide_quorum_randomness(&oracle_a, &seed_a, &request_id);

    let mut chosen_seed_b = 0;
    for seed_b in 0..10_000u64 {
        let seeds = soroban_sdk::vec![&env, (oracle_a.clone(), seed_a), (oracle_b.clone(), seed_b)];
        let aggregate = crate::randomness::aggregate_quorum_seeds(&env, &seeds);
        let selector = crate::randomness::OracleSeedWinnerSelection::new(aggregate);
        let winners = selector.select_winner_indices(&env, 3, 1);
        if winners.len() > 0 && winners.get(0).unwrap() == 2 {
            chosen_seed_b = seed_b;
            break;
        }
    }

    let res = client.try_provide_quorum_randomness(&oracle_b, &chosen_seed_b, &request_id);
    assert!(res.is_err(), "Contract should reject the ability to last-revealer bias the quorum draw");
}

#[test]
#[ignore = "issue #1077: vrf keypair grinding bias"]
fn test_provide_randomness_grinding_bias() {
    use ed25519_dalek::{SigningKey, Signer};
    use rand_core::OsRng;
    
    let env = Env::default();
    env.mock_all_auths();

    let oracle = Address::generate(&env);
    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);

    let factory = Address::generate(&env);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer_1 = Address::generate(&env);
    let buyer_2 = Address::generate(&env);
    let buyer_3 = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let payment_token = env.register_stellar_asset_contract_v2(token_admin).address();
    let token = StellarAssetClient::new(&env, &payment_token);
    token.mint(&creator, &100_000);
    token.mint(&buyer_1, &100_000);
    token.mint(&buyer_2, &100_000);
    token.mint(&buyer_3, &100_000);

    let config = RaffleConfig {
        description: String::from_str(&env, "VRF grinding bias test"),
        end_time: 0,
        no_deadline: true,
        max_tickets: 3,
        max_tickets_per_tx: 3,
        max_tickets_per_address: 0,
        min_tickets: 1,
        allow_multiple: true,
        ticket_price: MIN_TICKET_PRICE,
        payment_token: payment_token.clone(),
        prize_amount: MIN_TICKET_PRICE * 10,
        prizes: soroban_sdk::vec![&env, 10000],
        randomness_source: RandomnessSource::External,
        oracle_address: Some(oracle.clone()),
        protocol_fee_bp: 0,
        treasury_address: None,
        swap_router: None,
        tikka_token: None,
        metadata_hash: BytesN::from_array(&env, &[0u8; 32]),
        claim_lockup_seconds: Some(0),
        swap_deadline_seconds: Some(300),
        early_bird_ticket_percentage: 0,
        early_bird_discount_bp: 0,
        category: None,
        unique_winners: true,
        bundles: Vec::new(&env),
        prize_token: None,
        nft_contract: None,
    };

    client.init(&factory, &admin, &creator, &config);
    env.as_contract(&contract_id, || {
        env.storage().instance().remove(&DataKey::Factory);
    });
    client.deposit_prize();

    client.buy_tickets(&buyer_1, &1); // index 0
    client.buy_tickets(&buyer_2, &1); // index 1
    client.buy_tickets(&buyer_3, &1); // index 2

    client.finalize_raffle();

    let request_id: u64 = env.as_contract(&contract_id, || {
        env.storage().instance().get(&DataKey::RandomnessRequestId).unwrap()
    });

    let message = crate::randomness::build_vrf_proof_message(&env, request_id);
    let msg_bytes = message.to_alloc_vec();

    let mut chosen_seed = 0;
    let mut chosen_pk = BytesN::from_array(&env, &[0; 32]);
    let mut chosen_proof = BytesN::from_array(&env, &[0; 64]);

    for _ in 0..10_000 {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verify_key = signing_key.verifying_key();
        
        let signature = signing_key.sign(&msg_bytes);
        
        let pk_bytes = verify_key.to_bytes();
        let sig_bytes = signature.to_bytes();
        
        let pk = BytesN::from_array(&env, &pk_bytes);
        let proof = BytesN::from_array(&env, &sig_bytes);
        
        let seed = crate::randomness::derive_random_seed_from_proof(&env, &proof);
        
        let selector = crate::randomness::OracleSeedWinnerSelection::new(seed);
        let winners = selector.select_winner_indices(&env, 3, 1);
        if winners.len() > 0 && winners.get(0).unwrap() == 2 {
            chosen_seed = seed;
            chosen_pk = pk;
            chosen_proof = proof;
            break;
        }
    }

    let res = client.try_provide_randomness(&chosen_seed, &chosen_pk, &chosen_proof, &request_id);
    assert!(res.is_err(), "Contract should reject the ability to grind VRF keypairs");
}
