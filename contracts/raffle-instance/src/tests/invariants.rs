use proptest::prelude::*;
use crate::{
    assert_solvent, calculate_tier_prize, DataKey, Raffle, RaffleStatus, Ticket, MAX_PRIZE_AMOUNT,
    MIN_TICKET_PRICE,
};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Vec};

fn valid_prize_weights() -> impl Strategy<Value = std::vec::Vec<u32>> {
    prop::collection::vec(0u32..=10_000, 0..=99)
        .prop_filter("basis points must leave room for the final tier", |weights| {
            weights.iter().copied().sum::<u32>() <= 10_000
        })
        .prop_map(|mut weights| {
            let allocated = weights.iter().copied().sum::<u32>();
            weights.push(10_000 - allocated);
            weights
        })
}

fn test_raffle(env: &Env, weights: &[u32], prize_amount: i128) -> Raffle {
    let mut prizes = Vec::new(env);
    for weight in weights {
        prizes.push_back(*weight);
    }

    Raffle {
        creator: Address::generate(env),
        description: String::from_str(env, "tier invariant"),
        end_time: 0,
        no_deadline: true,
        max_tickets: 1,
        max_tickets_per_tx: 1,
        max_tickets_per_address: 1,
        min_tickets: 1,
        allow_multiple: true,
        ticket_price: MIN_TICKET_PRICE,
        payment_token: Address::generate(env),
        prize_token: Address::generate(env),
        prize_amount,
        prizes,
        tickets_sold: 0,
        status: RaffleStatus::PendingPrize,
        prize_deposited: false,
        winners: Vec::new(env),
        claimed_winners: Vec::new(env),
        randomness_source: raffle_shared::RandomnessSource::Internal,
        oracle_address: None,
        protocol_fee_bp: 0,
        treasury_address: None,
        swap_router: None,
        tikka_token: None,
        finalized_at: None,
        claim_lockup_seconds: 0,
        claim_expiry_seconds: 1,
        swap_deadline_seconds: 0,
        ticket_sales_paused: false,
        early_bird_ticket_percentage: 0,
        early_bird_discount_bp: 0,
        metadata_hash: BytesN::from_array(env, &[1; 32]),
        unique_winners: false,
        nft_contract: None,
    }
}

fn assert_tier_sum(weights: &[u32], prize_amount: i128) {
    let env = Env::default();
    let raffle = test_raffle(&env, weights, prize_amount);
    let mut total = 0i128;

    for index in 0..raffle.prizes.len() {
        let amount = calculate_tier_prize(&raffle, index).unwrap();
        assert!(amount >= 0, "tier {index} computed a negative prize");
        total += amount;
    }

    assert_eq!(total, prize_amount);
}

proptest! {
    #[test]
    fn tier_prizes_sum_to_prize_amount(
        weights in valid_prize_weights(),
        prize_amount in MIN_TICKET_PRICE..=MAX_PRIZE_AMOUNT,
    ) {
        assert_tier_sum(&weights, prize_amount);
    }
}

#[test]
fn one_hundred_equal_tiers_sum_exactly() {
    assert_tier_sum(&[100; 100], 1_000_003);
}

#[test]
fn one_tier_receives_the_entire_prize() {
    assert_tier_sum(&[10_000], MAX_PRIZE_AMOUNT);
}

#[test]
fn final_tier_absorbs_maximum_rounding_dust() {
    assert_tier_sum(
        &[101; 99].iter().copied().chain([1]).collect::<std::vec::Vec<_>>(),
        10_000,
    );
}

fn assert_contract_solvent(env: &Env, contract_id: &Address) {
    env.as_contract(contract_id, || assert_solvent(env));
}

fn run_solvency_lifecycle(ticket_count: u32, first_tier_bp: u32, fee_bp: u32, cancel: bool) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let factory = env.register(crate::MockFactory, ());
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let treasury = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let payment_token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let token = soroban_sdk::token::StellarAssetClient::new(&env, &payment_token);
    let contract_id = env.register(crate::RaffleInstance, ());
    let client = crate::RaffleInstanceClient::new(&env, &contract_id);
    let prize_amount = MAX_PRIZE_AMOUNT;

    token.mint(
        &creator,
        &(prize_amount + MIN_TICKET_PRICE * ticket_count as i128 * 2),
    );

    let config = raffle_shared::RaffleConfig {
        description: String::from_str(&env, "solvency lifecycle"),
        end_time: 0,
        no_deadline: true,
        max_tickets: ticket_count,
        max_tickets_per_tx: ticket_count,
        min_tickets: 1,
        allow_multiple: true,
        ticket_price: MIN_TICKET_PRICE,
        payment_token: payment_token.clone(),
        prize_amount,
        prizes: soroban_sdk::vec![&env, first_tier_bp, 10_000 - first_tier_bp],
        randomness_source: raffle_shared::RandomnessSource::Internal,
        oracle_address: None,
        protocol_fee_bp: if cancel { 0 } else { fee_bp },
        treasury_address: Some(treasury.clone()),
        swap_router: None,
        tikka_token: None,
        metadata_hash: BytesN::from_array(&env, &[107; 32]),
        claim_lockup_seconds: Some(0),
        claim_expiry_seconds: Some(5),
        swap_deadline_seconds: Some(0),
        early_bird_ticket_percentage: 50,
        early_bird_discount_bp: 1_000,
        category: None,
        unique_winners: false,
        bundles: Vec::new(&env),
        prize_token: None,
        nft_contract: None,
    };

    client.init(&factory, &admin, &creator, &config);
    env.as_contract(&contract_id, || {
        env.storage().instance().remove(&DataKey::Factory)
    });
    assert_contract_solvent(&env, &contract_id);
    client.deposit_prize();
    assert_contract_solvent(&env, &contract_id);

    let mut payers = std::vec::Vec::new();
    for ticket_index in 0..ticket_count {
        let payer = Address::generate(&env);
        token.mint(&payer, &(MIN_TICKET_PRICE * 2));
        if ticket_index == 1 {
            let recipient = Address::generate(&env);
            client.buy_tickets_for(&payer, &recipient, &1);
        } else {
            client.buy_tickets(&payer, &1);
        }
        payers.push(payer);
        assert_contract_solvent(&env, &contract_id);
    }

    if cancel {
        client.cancel_raffle(&raffle_shared::CancelReason::CreatorCancelled);
        assert_contract_solvent(&env, &contract_id);
        client.refund_prize();
        assert_contract_solvent(&env, &contract_id);

        for ticket_id in 1..=ticket_count {
            client.refund_ticket(&payers[(ticket_id - 1) as usize], &ticket_id);
            assert_contract_solvent(&env, &contract_id);
        }

        let balance = soroban_sdk::token::Client::new(&env, &payment_token).balance(&contract_id);
        assert_eq!(balance, 0, "cancelled raffle escrow must settle to zero");
        return;
    }

    client.finalize_raffle();
    assert_contract_solvent(&env, &contract_id);

    let raffle = client.get_raffle();
    let first_winner = raffle.winners.get(0).unwrap().address;
    client.claim_prize(&first_winner, &0);
    assert_contract_solvent(&env, &contract_id);

    let fees = client.get_accumulated_fees();
    if fees > 0 {
        client.withdraw_fees(&treasury, &fees);
        assert_contract_solvent(&env, &contract_id);
    }

    env.ledger().set_timestamp(1_006);
    client.sweep_unclaimed(&0, &0);
    assert_contract_solvent(&env, &contract_id);
    assert_eq!(client.get_raffle().status, RaffleStatus::Claimed);
}

#[test]
fn claim_withdraw_and_sweep_preserve_solvency() {
    run_solvency_lifecycle(4, 5_000, 1_000, false);
}

#[test]
fn cancelled_raffle_refunds_settle_escrow_to_zero() {
    run_solvency_lifecycle(4, 5_000, 1_000, true);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 12, .. ProptestConfig::default() })]

    #[test]
    fn lifecycle_solvency_holds_for_ticket_counts_and_tier_splits(
        ticket_count in 4u32..=8,
        first_tier_bp in 1u32..10_000,
        fee_bp in 0u32..=2_000,
    ) {
        run_solvency_lifecycle(ticket_count, first_tier_bp, fee_bp, false);
    }
}

/// Refund solvency invariant (#827).
///
/// After any refund operation the contract must hold at least as much as it
/// still owes to ticket holders (`per_ticket_refund` for each not-yet-refunded
/// ticket id) plus any un-refunded prize escrowed on behalf of the creator.
///
/// Called by the refund-path lifecycle tests (`tests/claim.rs`) after every
/// refund/prize-recovery operation, and asserted inline by the fuzz harness
/// (`fuzz/fuzz_targets/real_harness.rs::refund_cancel`).
pub fn assert_refund_solvency(
    env: &Env,
    contract_id: &Address,
    payment_token: &Address,
    prize_token: &Address,
    ticket_ids_owing: &[u32],
    per_ticket_refund: i128,
    prize_owing: i128,
) {
    let payment_balance = soroban_sdk::token::Client::new(env, payment_token).balance(contract_id);
    // When prize and payment tokens are the same address (the current wiring)
    // the prize pot is part of the payment balance and must not be counted twice.
    let prize_balance = if payment_token == prize_token {
        0
    } else {
        soroban_sdk::token::Client::new(env, prize_token).balance(contract_id)
    };
    let held = payment_balance + prize_balance;
    let outstanding = ticket_ids_owing.len() as i128 * per_ticket_refund + prize_owing;
    assert!(
        held >= outstanding,
        "refund solvency violated: contract holds {held} but owes {outstanding} \
         ({} tickets outstanding, prize owing {prize_owing}, payment {}, prize {})",
        ticket_ids_owing.len(),
        payment_balance,
        prize_balance,
    );
}
