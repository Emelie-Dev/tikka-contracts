use soroban_sdk::{contracttype, Address, String};

/// Factory leaderboard sort key (#484).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[contracttype]
pub enum LeaderboardMetric {
    TicketsSold = 0,
    PrizeAmount = 1,
    TotalVolume = 2,
}

/// On-chain creator profile with display name, verified badge, and track record.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct CreatorProfile {
    pub name: String,
    pub verified: bool,
    pub raffles_created: u32,
}

/// Per-partner aggregate statistics for the partner dashboard API (#488).
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct PartnerStats {
    pub total_raffles: u32,
    pub total_volume: i128,
    pub total_fees_generated: i128,
    pub first_raffle_at: u64,
    pub latest_raffle_at: u64,
}