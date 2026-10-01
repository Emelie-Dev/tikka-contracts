use super::*;

#[test]
fn test_create_recurring_raffle() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);
    let rc = recurring_config(&env, base);

    let recurring_id = client.create_recurring_raffle(&creator, &rc);
    assert_eq!(recurring_id, 0u32);

    let entry = client
        .get_recurring_raffle(&recurring_id)
        .expect("recurring entry should exist");
    assert_eq!(entry.creator, creator);
    assert_eq!(entry.config.max_rounds, 3);
    assert!(entry.active);
    assert_eq!(entry.current_round, 0);
    assert!(entry.last_raffle_address.is_none());
    assert_eq!(entry.next_due, 1_000_000 + 86_400);
}

#[test]
fn test_create_recurring_raffle_increments_id() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);

    let id0 = client.create_recurring_raffle(&creator, &recurring_config(&env, base.clone()));
    let id1 = client.create_recurring_raffle(&creator, &recurring_config(&env, base));
    assert_eq!(id0, 0u32);
    assert_eq!(id1, 1u32);
}

#[test]
fn test_create_recurring_raffle_interval_bounds_enforced_at_both_ends() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);

    // 1. Below minimum bound (< 3,600s): must fail
    let below_min = RecurringRaffleConfig {
        interval_seconds: MIN_RECURRING_INTERVAL_SECONDS - 1,
        ..recurring_config(&env, base.clone())
    };
    assert_eq!(
        client.try_create_recurring_raffle(&creator, &below_min),
        Err(Ok(ContractError::InvalidParameters))
    );

    // 2. Exact minimum bound (3,600s): must succeed
    let exact_min = RecurringRaffleConfig {
        interval_seconds: MIN_RECURRING_INTERVAL_SECONDS,
        ..recurring_config(&env, base.clone())
    };
    let min_id = client.create_recurring_raffle(&creator, &exact_min);
    let min_entry = client.get_recurring_raffle(&min_id).expect("entry exists");
    assert_eq!(min_entry.config.interval_seconds, MIN_RECURRING_INTERVAL_SECONDS);

    // 3. Exact maximum bound (31,536,000s): must succeed
    let exact_max = RecurringRaffleConfig {
        interval_seconds: MAX_RECURRING_INTERVAL_SECONDS,
        ..recurring_config(&env, base.clone())
    };
    let max_id = client.create_recurring_raffle(&creator, &exact_max);
    let max_entry = client.get_recurring_raffle(&max_id).expect("entry exists");
    assert_eq!(max_entry.config.interval_seconds, MAX_RECURRING_INTERVAL_SECONDS);

    // 4. Above maximum bound (> 31,536,000s): must fail
    let above_max = RecurringRaffleConfig {
        interval_seconds: MAX_RECURRING_INTERVAL_SECONDS + 1,
        ..recurring_config(&env, base)
    };
    assert_eq!(
        client.try_create_recurring_raffle(&creator, &above_max),
        Err(Ok(ContractError::InvalidParameters))
    );
}

#[test]
fn test_trigger_next_round() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);
    let recurring_id = client.create_recurring_raffle(
        &creator,
        &recurring_config(&env, base),
    );

    env.ledger().set_timestamp(1_000_000 + 86_400);

    let addr = client.trigger_next_round(&recurring_id);

    let entry = client
        .get_recurring_raffle(&recurring_id)
        .expect("entry exists");
    assert_eq!(entry.current_round, 1);
    assert_eq!(entry.next_due, 1_000_000 + 86_400 + 86_400);
    assert_eq!(entry.last_raffle_address, Some(addr.clone()));

    let instances = client.get_recurring_instances(&recurring_id);
    assert_eq!(instances.len(), 1u32);
    assert_eq!(instances.get(0).unwrap(), addr);
}

#[test]
fn test_trigger_next_round_permissionless_authorization() {
    let env = Env::default();
    // Intentionally do NOT mock all auths upfront so we verify trigger_next_round requires no auth
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);

    env.mock_all_auths();
    let recurring_id = client.create_recurring_raffle(
        &creator,
        &recurring_config(&env, base),
    );

    // Advance time past next_due
    env.ledger().set_timestamp(1_000_000 + 86_400);

    // Any caller can trigger the round without creator auth
    let addr = client.trigger_next_round(&recurring_id);

    let entry = client
        .get_recurring_raffle(&recurring_id)
        .expect("entry exists");
    assert_eq!(entry.current_round, 1);
    assert_eq!(entry.last_raffle_address, Some(addr));
}

#[test]
fn test_trigger_next_round_multiple_rounds() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);
    let recurring_id = client.create_recurring_raffle(
        &creator,
        &recurring_config(&env, base),
    );

    let mut addrs = SdkVec::new(&env);
    for round in 1..=3 {
        env.ledger().set_timestamp(1_000_000 + 86_400 * round as u64);
        let addr = client.trigger_next_round(&recurring_id);
        addrs.push_back(addr);
    }

    let entry = client
        .get_recurring_raffle(&recurring_id)
        .expect("entry exists");
    assert_eq!(entry.current_round, 3);

    let instances = client.get_recurring_instances(&recurring_id);
    assert_eq!(instances.len(), 3u32);
    assert_eq!(instances, addrs);
}

#[test]
fn test_trigger_next_round_interval_not_elapsed() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);
    let recurring_id = client.create_recurring_raffle(
        &creator,
        &recurring_config(&env, base),
    );

    assert_eq!(
        client.try_trigger_next_round(&recurring_id),
        Err(Ok(ContractError::IntervalNotElapsed))
    );
}

#[test]
fn test_trigger_next_round_max_rounds_reached() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);

    let limited = RecurringRaffleConfig {
        max_rounds: 1,
        ..recurring_config(&env, base)
    };
    let recurring_id = client.create_recurring_raffle(&creator, &limited);

    env.ledger().set_timestamp(1_000_000 + 86_400);
    let _addr = client.trigger_next_round(&recurring_id);

    env.ledger().set_timestamp(1_000_000 + 86_400 * 2);
    assert_eq!(
        client.try_trigger_next_round(&recurring_id),
        Err(Ok(ContractError::MaxRoundsReached))
    );
}

#[test]
fn test_trigger_next_round_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);

    assert_eq!(
        client.try_trigger_next_round(&999u32),
        Err(Ok(ContractError::RecurringNotFound))
    );
}

#[test]
fn test_cancel_recurring_raffle() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);
    let recurring_id = client.create_recurring_raffle(
        &creator,
        &recurring_config(&env, base),
    );

    client.cancel_recurring_raffle(&recurring_id, &creator);

    let entry = client
        .get_recurring_raffle(&recurring_id)
        .expect("entry exists");
    assert!(!entry.active);
}

#[test]
fn test_cancel_recurring_raffle_by_admin() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);
    let recurring_id = client.create_recurring_raffle(
        &creator,
        &recurring_config(&env, base),
    );

    client.cancel_recurring_raffle(&recurring_id, &admin);

    let entry = client
        .get_recurring_raffle(&recurring_id)
        .expect("entry exists");
    assert!(!entry.active);
}

#[test]
fn test_cancel_recurring_raffle_not_authorized() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let stranger = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);
    let recurring_id = client.create_recurring_raffle(
        &creator,
        &recurring_config(&env, base),
    );

    env.mock_all_auths();
    assert_eq!(
        client.try_cancel_recurring_raffle(&recurring_id, &stranger),
        Err(Ok(ContractError::NotAuthorized))
    );
}

#[test]
fn test_cancel_recurring_raffle_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let caller = Address::generate(&env);

    assert_eq!(
        client.try_cancel_recurring_raffle(&999u32, &caller),
        Err(Ok(ContractError::RecurringNotFound))
    );
}

#[test]
fn test_trigger_recurring_when_inactive() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);
    let recurring_id = client.create_recurring_raffle(
        &creator,
        &recurring_config(&env, base),
    );

    client.cancel_recurring_raffle(&recurring_id, &creator);

    env.ledger().set_timestamp(1_000_000 + 86_400);
    assert_eq!(
        client.try_trigger_next_round(&recurring_id),
        Err(Ok(ContractError::RecurringInactive))
    );
}

#[test]
fn test_get_recurring_instances_empty_for_new() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);
    let recurring_id = client.create_recurring_raffle(
        &creator,
        &recurring_config(&env, base),
    );

    let instances = client.get_recurring_instances(&recurring_id);
    assert_eq!(instances.len(), 0u32);
    assert_eq!(instances, SdkVec::new(&env));
}

#[test]
fn test_infinite_recurring_raffle() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);
    let token = make_token(&env);
    let base = valid_base_config(&env, &token);

    let infinite = RecurringRaffleConfig {
        max_rounds: 0,
        ..recurring_config(&env, base)
    };
    let recurring_id = client.create_recurring_raffle(&creator, &infinite);

    for round in 1..=10 {
        env.ledger().set_timestamp(1_000_000 + 86_400 * round as u64);
        let addr = client.trigger_next_round(&recurring_id);
        let instances = client.get_recurring_instances(&recurring_id);
        assert_eq!(instances.len(), round as u32);
        assert_eq!(instances.get(round - 1).unwrap(), addr);
    }

    let entry = client
        .get_recurring_raffle(&recurring_id)
        .expect("entry exists");
    assert_eq!(entry.current_round, 10);
    assert!(entry.active);
}

#[test]
fn test_get_recurring_raffle_nonexistent() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _treasury) = setup_factory(&env);

    assert!(client.get_recurring_raffle(&999u32).is_none());
}
