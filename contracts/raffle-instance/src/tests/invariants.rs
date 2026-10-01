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
        bundles: Vec::new(env),
        randomness_source: raffle_shared::RandomnessSource::Internal,
        oracle_address: None,
        oracle_public_key: None,
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
/// Called by the refund-path lifecycle tests (`claim.rs`) after every
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

use raffle_shared::CancelReason;

/// Table-driven definition for entrypoint status rejection assertions (#1079).
#[derive(Clone, Copy)]
pub struct EntrypointStatusExpectation {
    pub entrypoint: &'static str,
    pub permitted: &'static [RaffleStatus],
    pub rejected: &'static [RaffleStatus],
}

pub const ENTRYPOINT_STATUS_TABLE: &[EntrypointStatusExpectation] = &[
    EntrypointStatusExpectation {
        entrypoint: "deposit_prize",
        permitted: &[RaffleStatus::PendingPrize],
        rejected: &[
            RaffleStatus::Active,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "buy_tickets",
        permitted: &[RaffleStatus::Active],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "buy_tickets_for",
        permitted: &[RaffleStatus::Active],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "submit_commit",
        permitted: &[RaffleStatus::Active],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "finalize_raffle",
        permitted: &[RaffleStatus::Active, RaffleStatus::Drawing],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "claim_prize",
        permitted: &[RaffleStatus::Finalized],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Drawing,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "sweep_unclaimed",
        permitted: &[RaffleStatus::Finalized],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Drawing,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "refund_prize",
        permitted: &[RaffleStatus::Cancelled, RaffleStatus::Failed],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "refund_ticket",
        permitted: &[RaffleStatus::Cancelled, RaffleStatus::Failed],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "batch_refund_tickets",
        permitted: &[RaffleStatus::Cancelled, RaffleStatus::Failed],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "emergency_withdraw",
        permitted: &[RaffleStatus::Drawing],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "pause_ticket_sales",
        permitted: &[RaffleStatus::Active],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "resume_ticket_sales",
        permitted: &[RaffleStatus::Active],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "withdraw_fees",
        permitted: &[RaffleStatus::Finalized, RaffleStatus::Claimed],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Drawing,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "sweep_dust",
        permitted: &[RaffleStatus::Cancelled, RaffleStatus::Claimed],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
            RaffleStatus::Failed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "wipe_storage",
        permitted: &[
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Drawing,
            RaffleStatus::Finalized,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "cancel_raffle",
        permitted: &[RaffleStatus::Active, RaffleStatus::Drawing],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "provide_randomness",
        permitted: &[RaffleStatus::Drawing],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
    EntrypointStatusExpectation {
        entrypoint: "trigger_randomness_fallback",
        permitted: &[RaffleStatus::Drawing],
        rejected: &[
            RaffleStatus::PendingPrize,
            RaffleStatus::Active,
            RaffleStatus::Finalized,
            RaffleStatus::Cancelled,
            RaffleStatus::Failed,
            RaffleStatus::Claimed,
        ],
    },
];

fn set_contract_status(env: &Env, contract_id: &Address, status: RaffleStatus) {
    env.as_contract(contract_id, || {
        let mut raffle: Raffle = env.storage().instance().get(&DataKey::Raffle).unwrap();
        raffle.status = status;
        raffle.prize_deposited = status != RaffleStatus::PendingPrize;
        env.storage().instance().set(&DataKey::Raffle, &raffle);
    });
}

fn invoke_entrypoint_for_rejection(
    client: &crate::RaffleInstanceClient<'_>,
    env: &Env,
    admin: &Address,
    creator: &Address,
    entrypoint: &str,
) -> bool {
    match entrypoint {
        "deposit_prize" => client.try_deposit_prize().is_err(),
        "buy_tickets" => client.try_buy_tickets(creator, &1).is_err(),
        "buy_tickets_for" => client.try_buy_tickets_for(creator, creator, &1).is_err(),
        "submit_commit" => client
            .try_submit_commit(&1, &BytesN::from_array(env, &[0u8; 32]))
            .is_err(),
        "finalize_raffle" => client.try_finalize_raffle().is_err(),
        "claim_prize" => client.try_claim_prize(creator, &0).is_err(),
        "sweep_unclaimed" => client.try_sweep_unclaimed(&0, &1).is_err(),
        "refund_prize" => client.try_refund_prize().is_err(),
        "refund_ticket" => client.try_refund_ticket(creator, &1).is_err(),
        "batch_refund_tickets" => client
            .try_batch_refund_tickets(creator, &soroban_sdk::Vec::new(env))
            .is_err(),
        "emergency_withdraw" => client.try_emergency_withdraw(creator).is_err(),
        "pause_ticket_sales" => client.try_pause_ticket_sales(creator).is_err(),
        "resume_ticket_sales" => client.try_resume_ticket_sales(creator).is_err(),
        "withdraw_fees" => client.try_withdraw_fees(admin, &1).is_err(),
        "sweep_dust" => client.try_sweep_dust().is_err(),
        "wipe_storage" => client.try_wipe_storage().is_err(),
        "cancel_raffle" => client
            .try_cancel_raffle(&CancelReason::CreatorCancelled)
            .is_err(),
        "provide_randomness" => client
            .try_provide_randomness(
                &0,
                &BytesN::from_array(env, &[0u8; 32]),
                &BytesN::from_array(env, &[0u8; 64]),
                &0,
            )
            .is_err(),
        "trigger_randomness_fallback" => client
            .try_trigger_randomness_fallback(creator, &false)
            .is_err(),
        other => panic!("unknown entrypoint: {other}"),
    }
}

/// Table-driven test ensuring every entrypoint partitions all RaffleStatus variants
/// into permitted vs rejected sets without omission or overlap.
#[test]
fn test_entrypoint_status_matrix_coverage() {
    let all = RaffleStatus::all();

    for row in ENTRYPOINT_STATUS_TABLE.iter() {
        assert_eq!(
            row.permitted.len() + row.rejected.len(),
            all.len(),
            "Entrypoint '{}' does not partition all {} RaffleStatus variants",
            row.entrypoint,
            all.len(),
        );

        for &status in all {
            let is_permitted = row.permitted.contains(&status);
            let is_rejected = row.rejected.contains(&status);
            assert!(
                is_permitted ^ is_rejected,
                "Entrypoint '{}' must classify status {:?} as either permitted or rejected, but not both",
                row.entrypoint,
                status
            );
        }
    }
}

#[test]
fn test_entrypoints_reject_every_prohibited_status() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let contract_id = env.register(crate::RaffleInstance, ());
    let client = crate::RaffleInstanceClient::new(&env, &contract_id);
    let factory = Address::generate(&env);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let payment_token = Address::generate(&env);

    let mut prizes = soroban_sdk::Vec::new(&env);
    prizes.push_back(10_000);

    let raffle = Raffle {
        creator: creator.clone(),
        description: String::from_str(&env, "status test"),
        end_time: 0,
        no_deadline: true,
        max_tickets: 10,
        max_tickets_per_tx: 10,
        max_tickets_per_address: 10,
        min_tickets: 1,
        allow_multiple: true,
        ticket_price: MIN_TICKET_PRICE,
        payment_token: payment_token.clone(),
        prize_token: payment_token.clone(),
        prize_amount: 10 * MIN_TICKET_PRICE,
        prizes,
        tickets_sold: 0,
        status: RaffleStatus::PendingPrize,
        prize_deposited: false,
        winners: soroban_sdk::Vec::new(&env),
        randomness_source: raffle_shared::RandomnessSource::Internal,
        oracle_address: None,
        protocol_fee_bp: 0,
        treasury_address: None,
        swap_router: None,
        tikka_token: None,
        finalized_at: None,
        claim_lockup_seconds: 0,
        claim_expiry_seconds: 3600,
        swap_deadline_seconds: 0,
        ticket_sales_paused: false,
        early_bird_ticket_percentage: 0,
        early_bird_discount_bp: 0,
        metadata_hash: BytesN::from_array(&env, &[1; 32]),
        unique_winners: false,
        bundles: soroban_sdk::Vec::new(&env),
        nft_contract: None,
    };

    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Raffle, &raffle);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Factory, &factory);
    });

    for row in ENTRYPOINT_STATUS_TABLE.iter() {
        for &status in row.rejected {
            set_contract_status(&env, &contract_id, status);
            let rejected = invoke_entrypoint_for_rejection(
                &client,
                &env,
                &admin,
                &creator,
                row.entrypoint,
            );
            assert!(
                rejected,
                "Entrypoint '{}' unexpectedly succeeded or did not reject prohibited status {:?}",
                row.entrypoint,
                status
            );
        }
    }
}
