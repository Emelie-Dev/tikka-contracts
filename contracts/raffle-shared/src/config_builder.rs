use soroban_sdk::{token, Address, BytesN, Env, String, Vec};

use crate::constants::{
    DEFAULT_CLAIM_EXPIRY_SECONDS, DEFAULT_CLAIM_LOCKUP_SECONDS,
    DEFAULT_SWAP_DEADLINE_SECONDS, MAX_CATEGORY_LENGTH, MAX_CLAIM_LOCKUP_SECONDS,
    MAX_DESCRIPTION_LENGTH, MAX_PRIZE_AMOUNT, MAX_PRIZES, MAX_SWAP_DEADLINE_SECONDS,
    MAX_TICKETS_LIMIT, MIN_CLAIM_EXPIRY_SECONDS, MIN_TICKET_PRICE,
};
use crate::{RandomnessSource, RaffleConfig};

/// Fluent builder for [`RaffleConfig`].
///
/// # Example
///
/// ```rust
/// let config = RaffleConfigBuilder::new(&env, payment_token)
///     .max_tickets(100)
///     .ticket_price(10_000)
///     .prizes(vec![&env, 10_000])
///     .build()?;
/// ```
#[allow(dead_code)]
pub struct RaffleConfigBuilder<'a> {
    env: &'a Env,
    payment_token: Address,
    description: String,
    end_time: u64,
    no_deadline: bool,
    max_tickets: u32,
    max_tickets_per_tx: Option<u32>,
    max_tickets_per_address: u32,
    min_tickets: u32,
    allow_multiple: bool,
    ticket_price: i128,
    prize_amount: i128,
    prizes: Vec<u32>,
    randomness_source: RandomnessSource,
    oracle_address: Option<Address>,
    oracle_public_key: Option<BytesN<32>>,
    protocol_fee_bp: u32,
    treasury_address: Option<Address>,
    swap_router: Option<Address>,
    tikka_token: Option<Address>,
    metadata_hash: BytesN<32>,
    claim_lockup_seconds: Option<u64>,
    claim_expiry_seconds: Option<u64>,
    swap_deadline_seconds: Option<u64>,
    early_bird_ticket_percentage: u32,
    early_bird_discount_bp: u32,
    category: Option<String>,
    unique_winners: bool,
    bundles: Vec<crate::TicketBundle>,
    prize_token: Option<Address>,
    nft_contract: Option<Address>,
}

impl<'a> RaffleConfigBuilder<'a> {
    /// Create a new builder with the given environment and payment token.
    ///
    /// All other fields receive safe defaults that satisfy `raffle-instance`
    /// validation.
    pub fn new(env: &'a Env, payment_token: Address) -> Self {
        Self {
            env,
            payment_token,
            description: String::from_str(env, "Test Raffle"),
            end_time: 0,
            no_deadline: true,
            max_tickets: 100,
            max_tickets_per_tx: None,
            max_tickets_per_address: 0,
            min_tickets: 1,
            allow_multiple: true,
            ticket_price: 10_000,
            prize_amount: 10_000,
            prizes: Vec::from_array(env, [10_000]),
            randomness_source: RandomnessSource::Internal,
            oracle_address: None,
            oracle_public_key: None,
            protocol_fee_bp: 0,
            treasury_address: None,
            swap_router: None,
            tikka_token: None,
            metadata_hash: BytesN::from_array(env, &[1u8; 32]),
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
        }
    }

    /// Start a builder from an existing config, preserving every field.
    pub fn from_config(env: &'a Env, config: RaffleConfig) -> Self {
        Self {
            env,
            payment_token: config.payment_token,
            description: config.description,
            end_time: config.end_time,
            no_deadline: config.no_deadline,
            max_tickets: config.max_tickets,
            max_tickets_per_tx: Some(config.max_tickets_per_tx),
            max_tickets_per_address: config.max_tickets_per_address,
            min_tickets: config.min_tickets,
            allow_multiple: config.allow_multiple,
            ticket_price: config.ticket_price,
            prize_amount: config.prize_amount,
            prizes: config.prizes,
            randomness_source: config.randomness_source,
            oracle_address: config.oracle_address,
            oracle_public_key: config.oracle_public_key,
            protocol_fee_bp: config.protocol_fee_bp,
            treasury_address: config.treasury_address,
            swap_router: config.swap_router,
            tikka_token: config.tikka_token,
            metadata_hash: config.metadata_hash,
            claim_lockup_seconds: config.claim_lockup_seconds,
            claim_expiry_seconds: config.claim_expiry_seconds,
            swap_deadline_seconds: config.swap_deadline_seconds,
            early_bird_ticket_percentage: config.early_bird_ticket_percentage,
            early_bird_discount_bp: config.early_bird_discount_bp,
            category: config.category,
            unique_winners: config.unique_winners,
            bundles: config.bundles,
            prize_token: config.prize_token,
            nft_contract: config.nft_contract,
        }
    }

    pub fn description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub fn end_time(mut self, end_time: u64) -> Self {
        self.end_time = end_time;
        self
    }

    pub fn no_deadline(mut self, no_deadline: bool) -> Self {
        self.no_deadline = no_deadline;
        self
    }

    pub fn max_tickets(mut self, max_tickets: u32) -> Self {
        self.max_tickets = max_tickets;
        self
    }

    pub fn max_tickets_per_tx(mut self, max_tickets_per_tx: u32) -> Self {
        self.max_tickets_per_tx = Some(max_tickets_per_tx);
        self
    }

    pub fn max_tickets_per_address(mut self, max_tickets_per_address: u32) -> Self {
        self.max_tickets_per_address = max_tickets_per_address;
        self
    }

    pub fn min_tickets(mut self, min_tickets: u32) -> Self {
        self.min_tickets = min_tickets;
        self
    }

    pub fn allow_multiple(mut self, allow_multiple: bool) -> Self {
        self.allow_multiple = allow_multiple;
        self
    }

    pub fn ticket_price(mut self, ticket_price: i128) -> Self {
        self.ticket_price = ticket_price;
        self
    }

    pub fn prize_amount(mut self, prize_amount: i128) -> Self {
        self.prize_amount = prize_amount;
        self
    }

    pub fn prizes(mut self, prizes: Vec<u32>) -> Self {
        self.prizes = prizes;
        self
    }

    pub fn bundles(mut self, bundles: Vec<crate::TicketBundle>) -> Self {
        self.bundles = bundles;
        self
    }

    pub fn randomness_source(mut self, randomness_source: RandomnessSource) -> Self {
        self.randomness_source = randomness_source;
        self
    }

    pub fn oracle_address(mut self, oracle_address: Option<Address>) -> Self {
        self.oracle_address = oracle_address;
        self
    }

    /// Set the Ed25519 public key for the registered VRF oracle (#985).
    ///
    /// Must be set alongside `oracle_address` when
    /// `randomness_source == External` so that `provide_randomness` can
    /// verify the submitted `public_key` argument matches the key on record.
    pub fn oracle_public_key(mut self, oracle_public_key: Option<BytesN<32>>) -> Self {
        self.oracle_public_key = oracle_public_key;
        self
    }

    pub fn protocol_fee_bp(mut self, protocol_fee_bp: u32) -> Self {
        self.protocol_fee_bp = protocol_fee_bp;
        self
    }

    pub fn treasury_address(mut self, treasury_address: Option<Address>) -> Self {
        self.treasury_address = treasury_address;
        self
    }

    pub fn swap_router(mut self, swap_router: Option<Address>) -> Self {
        self.swap_router = swap_router;
        self
    }

    pub fn tikka_token(mut self, tikka_token: Option<Address>) -> Self {
        self.tikka_token = tikka_token;
        self
    }

    pub fn metadata_hash(mut self, metadata_hash: BytesN<32>) -> Self {
        self.metadata_hash = metadata_hash;
        self
    }

    pub fn claim_lockup_seconds(mut self, claim_lockup_seconds: u64) -> Self {
        self.claim_lockup_seconds = Some(claim_lockup_seconds);
        self
    }

    pub fn swap_deadline_seconds(mut self, swap_deadline_seconds: u64) -> Self {
        self.swap_deadline_seconds = Some(swap_deadline_seconds);
        self
    }

    pub fn early_bird_ticket_percentage(mut self, early_bird_ticket_percentage: u32) -> Self {
        self.early_bird_ticket_percentage = early_bird_ticket_percentage;
        self
    }

    pub fn early_bird_discount_bp(mut self, early_bird_discount_bp: u32) -> Self {
        self.early_bird_discount_bp = early_bird_discount_bp;
        self
    }

    pub fn claim_expiry_seconds(mut self, claim_expiry_seconds: u64) -> Self {
        self.claim_expiry_seconds = Some(claim_expiry_seconds);
        self
    }

    pub fn category(mut self, category: Option<String>) -> Self {
        self.category = category;
        self
    }

    pub fn unique_winners(mut self, unique_winners: bool) -> Self {
        self.unique_winners = unique_winners;
        self
    }

    pub fn prize_token(mut self, prize_token: Option<Address>) -> Self {
        self.prize_token = prize_token;
        self
    }

    pub fn nft_contract(mut self, nft_contract: Option<Address>) -> Self {
        self.nft_contract = nft_contract;
        self
    }

    /// Build and validate the [`RaffleConfig`] using the instance initializer's rules.
    pub fn build(self) -> Result<RaffleConfig, ConfigValidationError> {
        let max_tickets_per_tx = self
            .max_tickets_per_tx
            .unwrap_or(self.max_tickets);

        let mut config = RaffleConfig {
            description: self.description,
            end_time: self.end_time,
            no_deadline: self.no_deadline,
            max_tickets: self.max_tickets,
            max_tickets_per_tx,
            max_tickets_per_address: self.max_tickets_per_address,
            min_tickets: self.min_tickets,
            allow_multiple: self.allow_multiple,
            ticket_price: self.ticket_price,
            payment_token: self.payment_token,
            prize_amount: self.prize_amount,
            prizes: self.prizes,
            randomness_source: self.randomness_source,
            oracle_address: self.oracle_address,
            oracle_public_key: self.oracle_public_key,
            protocol_fee_bp: self.protocol_fee_bp,
            treasury_address: self.treasury_address,
            swap_router: self.swap_router,
            tikka_token: self.tikka_token,
            metadata_hash: self.metadata_hash,
            claim_lockup_seconds: self.claim_lockup_seconds,
            claim_expiry_seconds: self.claim_expiry_seconds,
            swap_deadline_seconds: self.swap_deadline_seconds,
            early_bird_ticket_percentage: self.early_bird_ticket_percentage,
            early_bird_discount_bp: self.early_bird_discount_bp,
            category: self.category,
            unique_winners: self.unique_winners,
            bundles: self.bundles,
            prize_token: self.prize_token,
            nft_contract: self.nft_contract,
        };
        validate_config(self.env, &mut config, None)?;
        Ok(config)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ConfigValidationError {
    InvalidParameters,
    InvalidTicketRange,
    InvalidEndTime,
    TooManyPrizes,
    InvalidTokenAddress,
}

/// Validate and normalize config fields shared with raffle-instance `init`.
/// `instance_address` is supplied only by the instance initializer for its
/// external-oracle self-address check.
pub fn validate_config(
    env: &Env,
    config: &mut RaffleConfig,
    instance_address: Option<&Address>,
) -> Result<(), ConfigValidationError> {
    let invalid = || ConfigValidationError::InvalidParameters;
    let now = env.ledger().timestamp();
    if config.description.len() > MAX_DESCRIPTION_LENGTH {
        return Err(invalid());
    }
    if config.no_deadline && config.end_time != 0 {
        return Err(invalid());
    }
    if !config.no_deadline && config.end_time <= now {
        return Err(invalid());
    }
    if config.end_time != 0 && config.end_time <= now {
        return Err(ConfigValidationError::InvalidEndTime);
    }
    if config.max_tickets == 0 || config.max_tickets > MAX_TICKETS_LIMIT {
        return Err(invalid());
    }
    if config.max_tickets < config.min_tickets {
        return Err(ConfigValidationError::InvalidTicketRange);
    }
    if config.max_tickets_per_tx == 0 || config.max_tickets_per_tx > config.max_tickets {
        return Err(invalid());
    }
    if config.max_tickets_per_address == 0 && !config.allow_multiple {
        config.max_tickets_per_address = 1;
    }
    if config.max_tickets_per_address > config.max_tickets {
        return Err(invalid());
    }
    if config.ticket_price < MIN_TICKET_PRICE {
        return Err(invalid());
    }
    if config.bundles.len() > 16 {
        return Err(invalid());
    }
    let mut previous_quantity = 0;
    for index in 0..config.bundles.len() {
        let Some(bundle) = config.bundles.get(index) else {
            return Err(invalid());
        };
        if bundle.quantity == 0 || bundle.price_per_ticket < MIN_TICKET_PRICE {
            return Err(invalid());
        }
        if index > 0 {
            let Some(previous) = config.bundles.get(index - 1) else {
                return Err(invalid());
            };
            if bundle.quantity <= previous_quantity
                || bundle.price_per_ticket > previous.price_per_ticket
            {
                return Err(invalid());
            }
        }
        previous_quantity = bundle.quantity;
    }
    if config.prize_amount < config.ticket_price || config.prize_amount > MAX_PRIZE_AMOUNT {
        return Err(invalid());
    }
    if config.prizes.is_empty() {
        return Err(invalid());
    }
    if config.prizes.len() > MAX_PRIZES {
        return Err(ConfigValidationError::TooManyPrizes);
    }
    let mut prize_total = 0u32;
    for basis_points in config.prizes.iter() {
        prize_total = prize_total
            .checked_add(basis_points)
            .ok_or_else(invalid)?;
    }
    if prize_total != 10_000 {
        return Err(invalid());
    }
    if config.protocol_fee_bp > 10_000
        || (config.protocol_fee_bp > 0 && config.treasury_address.is_none())
    {
        return Err(invalid());
    }
    if config.randomness_source == RandomnessSource::External {
        match &config.oracle_address {
            None => return Err(invalid()),
            Some(address) if instance_address == Some(address) => {
                return Err(invalid());
            }
            Some(_) => {}
        }
    } else if config.oracle_address.is_some() {
        return Err(invalid());
    }
    if config.metadata_hash == BytesN::from_array(env, &[0u8; 32]) {
        return Err(invalid());
    }
    validate_category(&config.category)?;

    let token_client = token::Client::new(env, &config.payment_token);
    let _ = token_client
        .try_decimals()
        .map_err(|_| ConfigValidationError::InvalidTokenAddress)?;

    *config = config.clone().resolve_defaults();
    let claim_lockup = config
        .claim_lockup_seconds
        .unwrap_or(DEFAULT_CLAIM_LOCKUP_SECONDS);
    let claim_expiry = config
        .claim_expiry_seconds
        .unwrap_or(DEFAULT_CLAIM_EXPIRY_SECONDS);
    let swap_deadline = config
        .swap_deadline_seconds
        .unwrap_or(DEFAULT_SWAP_DEADLINE_SECONDS);
    if claim_lockup > MAX_CLAIM_LOCKUP_SECONDS
        || claim_expiry < MIN_CLAIM_EXPIRY_SECONDS
        || claim_expiry <= claim_lockup
        || swap_deadline > MAX_SWAP_DEADLINE_SECONDS
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_category(category: &Option<String>) -> Result<(), ConfigValidationError> {
    let Some(category) = category else {
        return Ok(());
    };
    let length = category.len();
    if length == 0 || length > MAX_CATEGORY_LENGTH {
        return Err(ConfigValidationError::InvalidParameters);
    }
    let mut bytes = [0u8; MAX_CATEGORY_LENGTH as usize];
    let slice = &mut bytes[..length as usize];
    category.copy_into_slice(slice);
    if slice
        .iter()
        .any(|byte| !byte.is_ascii_alphanumeric() && *byte != b'-')
    {
        return Err(ConfigValidationError::InvalidParameters);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn build_rejects_invalid_ticket_limits() {
        let env = Env::default();
        let payment_token = Address::generate(&env);

        let result = RaffleConfigBuilder::new(&env, payment_token)
            .max_tickets(0)
            .build();

        assert_eq!(result, Err(ConfigValidationError::InvalidParameters));
    }
}
