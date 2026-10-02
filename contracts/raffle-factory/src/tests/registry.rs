use super::*;

#[test]
fn test_get_raffles_by_creator_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _treasury) = setup_factory(&env);
    let creator = Address::generate(&env);

    let page = client.get_raffles_by_creator(
        &creator,
        &PaginationParams {
            limit: 10,
            offset: 0,
        },
    );
    assert_eq!(page.items.len(), 0u32);
    assert_eq!(page.total, 0u32);
    assert!(!page.has_more);
}

#[test]
fn test_get_raffles_by_creator_basic() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _treasury) = setup_factory(&env);

    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);

    let a_addrs = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    let b_addrs = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];

    seed_creator_index(&env, &client.address, &creator_a, &a_addrs);
    seed_creator_index(&env, &client.address, &creator_b, &b_addrs);

    let page_a = client.get_raffles_by_creator(
        &creator_a,
        &PaginationParams {
            limit: 10,
            offset: 0,
        },
    );
    assert_eq!(page_a.total, 5u32);
    assert_eq!(page_a.items.len(), 5u32);
    assert!(!page_a.has_more);
    for (i, addr) in a_addrs.iter().enumerate() {
        assert_eq!(page_a.items.get(i as u32).unwrap(), addr.clone());
    }

    let page_b = client.get_raffles_by_creator(
        &creator_b,
        &PaginationParams {
            limit: 10,
            offset: 0,
        },
    );
    assert_eq!(page_b.total, 3u32);
    assert_eq!(page_b.items.len(), 3u32);
    assert!(!page_b.has_more);
    for (i, addr) in b_addrs.iter().enumerate() {
        assert_eq!(page_b.items.get(i as u32).unwrap(), addr.clone());
    }
}

#[test]
fn test_get_raffles_by_creator_pagination() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _treasury) = setup_factory(&env);

    let creator = Address::generate(&env);
    let addrs = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    seed_creator_index(&env, &client.address, &creator, &addrs);

    let p0 = client.get_raffles_by_creator(
        &creator,
        &PaginationParams {
            limit: 3,
            offset: 0,
        },
    );
    assert_eq!(p0.items.len(), 3u32);
    assert_eq!(p0.total, 5u32);
    assert!(p0.has_more);
    assert_eq!(p0.items.get(0).unwrap(), addrs[0].clone());
    assert_eq!(p0.items.get(2).unwrap(), addrs[2].clone());

    let p1 = client.get_raffles_by_creator(
        &creator,
        &PaginationParams {
            limit: 3,
            offset: 3,
        },
    );
    assert_eq!(p1.items.len(), 2u32);
    assert_eq!(p1.total, 5u32);
    assert!(!p1.has_more);
    assert_eq!(p1.items.get(0).unwrap(), addrs[3].clone());
    assert_eq!(p1.items.get(1).unwrap(), addrs[4].clone());

    let p_oor = client.get_raffles_by_creator(
        &creator,
        &PaginationParams {
            limit: 10,
            offset: 99,
        },
    );
    assert_eq!(p_oor.items.len(), 0u32);
    assert!(!p_oor.has_more);

    let p_exact = client.get_raffles_by_creator(
        &creator,
        &PaginationParams {
            limit: 10,
            offset: 5,
        },
    );
    assert_eq!(p_exact.items.len(), 0u32);
    assert!(!p_exact.has_more);
}

#[test]
fn test_creator_index_isolates_separate_creators() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _treasury) = setup_factory(&env);

    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);

    let a_addrs = [Address::generate(&env), Address::generate(&env)];
    let b_addrs = [Address::generate(&env)];

    seed_creator_index(&env, &client.address, &creator_a, &a_addrs);
    seed_creator_index(&env, &client.address, &creator_b, &b_addrs);

    let pa = client.get_raffles_by_creator(
        &creator_a,
        &PaginationParams {
            limit: 10,
            offset: 0,
        },
    );
    assert_eq!(pa.total, 2u32);

    let pb = client.get_raffles_by_creator(
        &creator_b,
        &PaginationParams {
            limit: 10,
            offset: 0,
        },
    );
    assert_eq!(pb.total, 1u32);
    assert_eq!(pb.items.get(0).unwrap(), b_addrs[0].clone());
}

#[test]
fn get_raffles_by_category_unknown_is_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _treasury) = setup_factory(&env);

    let page = client.get_raffles_by_category(
        &String::from_str(&env, "gaming"),
        &PaginationParams {
            limit: 10,
            offset: 0,
        },
    );
    assert_eq!(page.items.len(), 0u32);
    assert_eq!(page.total, 0u32);
    assert!(!page.has_more);
}

#[test]
fn get_raffles_by_category_returns_only_matching() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _treasury) = setup_factory(&env);

    let gaming = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    let art = [Address::generate(&env), Address::generate(&env)];

    seed_category_index(&env, &client.address, "gaming", &gaming);
    seed_category_index(&env, &client.address, "art", &art);

    let gaming_page = client.get_raffles_by_category(
        &String::from_str(&env, "gaming"),
        &PaginationParams {
            limit: 10,
            offset: 0,
        },
    );
    assert_eq!(gaming_page.total, 3u32);
    assert_eq!(gaming_page.items.len(), 3u32);
    assert!(!gaming_page.has_more);
    for (i, addr) in gaming.iter().enumerate() {
        assert_eq!(gaming_page.items.get(i as u32).unwrap(), addr.clone());
    }

    let art_page = client.get_raffles_by_category(
        &String::from_str(&env, "art"),
        &PaginationParams {
            limit: 10,
            offset: 0,
        },
    );
    assert_eq!(art_page.total, 2u32);
    assert_eq!(art_page.items.len(), 2u32);
    assert!(!art_page.has_more);

    let charity_page = client.get_raffles_by_category(
        &String::from_str(&env, "charity"),
        &PaginationParams {
            limit: 10,
            offset: 0,
        },
    );
    assert_eq!(charity_page.total, 0u32);
    assert_eq!(charity_page.items.len(), 0u32);
}

#[test]
fn get_raffles_by_category_paginates() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _treasury) = setup_factory(&env);

    let addrs = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    seed_category_index(&env, &client.address, "gaming", &addrs);

    let p0 = client.get_raffles_by_category(
        &String::from_str(&env, "gaming"),
        &PaginationParams {
            limit: 3,
            offset: 0,
        },
    );
    assert_eq!(p0.items.len(), 3u32);
    assert_eq!(p0.total, 5u32);
    assert!(p0.has_more);
    assert_eq!(p0.items.get(0).unwrap(), addrs[0].clone());
    assert_eq!(p0.items.get(2).unwrap(), addrs[2].clone());

    let p1 = client.get_raffles_by_category(
        &String::from_str(&env, "gaming"),
        &PaginationParams {
            limit: 3,
            offset: 3,
        },
    );
    assert_eq!(p1.items.len(), 2u32);
    assert!(!p1.has_more);
    assert_eq!(p1.items.get(0).unwrap(), addrs[3].clone());
    assert_eq!(p1.items.get(1).unwrap(), addrs[4].clone());
}
