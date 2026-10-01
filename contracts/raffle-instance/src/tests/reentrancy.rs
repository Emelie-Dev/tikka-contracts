//! Reentrancy guard tests: a hostile prize_token that attempts to re-enter
//! refund_prize (or any other payout path) during its transfer callback must
//! be blocked by the guard.

use crate::*;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, BytesN, Env, String,
};

// ---------------------------------------------------------------------------
// Hostile token contract
//
// Behaves like a normal token for every call except the first `transfer` it
// receives, which it uses to re-enter the raffle's `refund_prize`.  If that
// second invocation succeeds the attacker drains the prize twice; the guard
// must prevent this.
// ---------------------------------------------------------------------------

#[contract]
pub struct HostileToken;

#[contractimpl]
impl HostileToken {
    /// Called once by `init::deposit_prize` so the raffle can record the
    /// deposit.  We just do nothing; the raffle contract is the "holder".
    pub fn transfer_from(
        _env: Env,
        _spender: Address,
        _from: Address,
        _to: Address,
        _amount: i128,
    ) {
    }

    /// Called by the raffle's `refund_prize` to pay out the creator.
    /// On the first invocation we immediately attempt a second `refund_prize`
    /// call; the reentrancy guard must fire with `Error::Reentrancy`.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        // `to` is the creator / raffle contract address; `from` is the raffle
        // contract.  We look up the raffle contract via `from` and attempt to
        // re-enter it.
        let raffle_contract = from.clone();
        let reentrant_client = RaffleInstanceClient::new(&env, &raffle_contract);

        // A second refund_prize while the first is in-flight must be rejected.
        let result = reentrant_client.try_refund_prize();
        assert_eq!(
            result,
            Err(Ok(Error::Reentrancy)),
            "hostile re-entrancy into refund_prize was not blocked"
        );

        // We do NOT actually move tokens — the guard is all we verify here.
        let _ = (to, amount);
    }

    /// `balance` is queried by `claim_prize` before paying out; return
    /// a large value so that check never trips.
    pub fn balance(_env: Env, _id: Address) -> i128 {
        i128::MAX
    }
}

// ---------------------------------------------------------------------------
// Helper: spin up a cancelled raffle that used the hostile token as its prize
// token.  Bypasses `deposit_prize` (which calls `transfer_from` on the real
// SAC) by directly writing the raffle state via `env.as_contract`.
// ---------------------------------------------------------------------------

fn setup_hostile_prize_raffle(
    env: &Env,
) -> (RaffleInstanceClient<'_>, Address, Address, Address) {
    let factory = Address::generate(env);
    let admin = Address::generate(env);
    let creator = Address::generate(env);

    // Register the real payment token (used for ticket price, not the prize).
    let payment_token = env
        .register_stellar_asset_contract_v2(Address::generate(env))
        .address();
    StellarAssetClient::new(env, &payment_token).mint(&creator, &1_000_000);

    // Register the hostile prize token.
    let hostile_token = env.register(HostileToken, ());

    let contract_id = env.register(RaffleInstance, ());
    let client = RaffleInstanceClient::new(env, &contract_id);

    let config = RaffleConfig {
        description: String::from_str(env, "hostile prize token"),
        end_time: 0,
        no_deadline: true,
        max_tickets: 1,
        max_tickets_per_tx: 1,
        min_tickets: 1,
        allow_multiple: true,
        ticket_price: MIN_TICKET_PRICE,
        payment_token: payment_token.clone(),
        prize_amount: MIN_TICKET_PRICE * 10,
        prizes: soroban_sdk::vec![env, 10000u32],
        randomness_source: RandomnessSource::Internal,
        oracle_address: None,
        oracle_public_key: None,
        protocol_fee_bp: 0,
        treasury_address: None,
        swap_router: None,
        tikka_token: None,
        unique_winners: false,
        metadata_hash: BytesN::from_array(env, &[42u8; 32]),
        claim_lockup_seconds: Some(0),
        swap_deadline_seconds: Some(0),
        early_bird_ticket_percentage: 0,
        early_bird_discount_bp: 0,
        category: None,
    };

    client.init(&factory, &admin, &creator, &config);

    // Directly patch the raffle to use the hostile token as prize_token and
    // set it into Cancelled + prize_deposited=true, simulating a scenario
    // where a hostile token was the prize token all along.
    env.as_contract(&contract_id, || {
        let mut raffle = read_raffle(env).unwrap();
        raffle.prize_token = hostile_token.clone();
        raffle.status = RaffleStatus::Cancelled;
        raffle.prize_deposited = true;
        write_raffle(env, &raffle);
        // Remove Factory key so buy_tickets cross-contract call is not needed.
        env.storage().instance().remove(&DataKey::Factory);
    });

    (client, contract_id, creator, hostile_token)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The hostile token tries to call `refund_prize` again from inside `transfer`.
/// The reentrancy guard must reject the inner call with `Error::Reentrancy`
/// and the outer call must complete (the hostile token doesn't revert itself,
/// it just can't drain the escrow twice).
#[test]
fn refund_prize_blocks_reentrant_call_via_hostile_token() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let (client, _contract_id, _creator, _hostile_token) = setup_hostile_prize_raffle(&env);

    // The outer call is allowed; the inner attempt inside `transfer` is blocked.
    // The hostile token's `transfer` asserts Reentrancy internally, so if the
    // guard is missing the assertion inside HostileToken::transfer would fail.
    client.refund_prize();

    // After a successful refund the prize_deposited flag must be cleared.
    let raffle = client.get_raffle();
    assert!(!raffle.prize_deposited);
}

/// Verify that after `refund_prize` completes the guard is released, so a
/// subsequent legitimate call (e.g. a second contract that was already
/// cancelled independently) is not permanently locked.
#[test]
fn refund_prize_releases_guard_after_completion() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    // Use a real SAC so this test doesn't depend on the hostile token.
    let factory = Address::generate(&env);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let payment_token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    StellarAssetClient::new(&env, &payment_token).mint(&creator, &1_000_000);

    let contract_id = env.register(RaffleInstance, ());
    let client = RaffleInstanceClient::new(&env, &contract_id);

    let config = RaffleConfig {
        description: String::from_str(&env, "guard release check"),
        end_time: 0,
        no_deadline: true,
        max_tickets: 1,
        max_tickets_per_tx: 1,
        min_tickets: 1,
        allow_multiple: true,
        ticket_price: MIN_TICKET_PRICE,
        payment_token: payment_token.clone(),
        prize_amount: MIN_TICKET_PRICE * 10,
        prizes: soroban_sdk::vec![&env, 10000u32],
        randomness_source: RandomnessSource::Internal,
        oracle_address: None,
        oracle_public_key: None,
        protocol_fee_bp: 0,
        treasury_address: None,
        swap_router: None,
        tikka_token: None,
        unique_winners: false,
        metadata_hash: BytesN::from_array(&env, &[43u8; 32]),
        claim_lockup_seconds: Some(0),
        swap_deadline_seconds: Some(0),
        early_bird_ticket_percentage: 0,
        early_bird_discount_bp: 0,
        category: None,
    };

    client.init(&factory, &admin, &creator, &config);
    client.deposit_prize();
    client.cancel_raffle(&CancelReason::AdminCancelled);

    // First call succeeds.
    client.refund_prize();
    assert!(!client.get_raffle().prize_deposited);

    // Guard must be released: a second call returns PrizeNotDeposited (not Reentrancy).
    let result = client.try_refund_prize();
    assert_eq!(result, Err(Ok(Error::PrizeNotDeposited)));

    // Confirm no guard key is lingering in storage.
    env.as_contract(&contract_id, || {
        assert!(
            !env.storage()
                .instance()
                .has(&DataKey::ReentrancyGuard),
            "ReentrancyGuard key must be cleared after refund_prize"
        );
    });
}
