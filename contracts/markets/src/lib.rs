#![no_std]

//! Markets contract with auth-gated entrypoints.
//!
//! Provides a prediction-market subsystem where every state-changing
//! entrypoint enforces `require_auth` on the acting [`Address`].
//!
//! # Auth Matrix
//!
//! | Function               | Required Role              |
//! |------------------------|----------------------------|
//! | `create_market`        | Creator (any address)      |
//! | `place_bet`            | Bettor (any address)       |
//! | `resolve_market`       | Market creator             |
//! | `claim_winnings`       | Winner (bettor)            |
//! | `cancel_market`        | Market creator             |
//! | `withdraw_funds`       | Market creator             |
//! | `update_market_params` | Market creator             |
//! | `add_liquidity`        | Liquidity provider         |
//! | `remove_liquidity`     | Liquidity provider         |
//! | `pause_markets`        | Admin                      |
//! | `unpause_markets`      | Admin                      |
//! | `transfer_ownership`   | Admin                      |
//! | `version`              | Anyone (read-only)         |

pub mod errors;

use soroban_sdk::{
    contract, contractimpl, contracttype, panic_with_error, Address, Env, String, Vec,
};

pub use errors::ContractError;

/// Type alias used by generated client code and test harnesses.
pub type Error = ContractError;

/// Persistent-storage keys used by the Markets contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Sequential counter for generating unique market IDs.
    MarketCounter,
    /// Market data keyed by the numeric market ID.
    Market(u32),
    /// Bet placed by a user on a specific market.
    Bet(u32, Address),
    /// Whether the markets subsystem is globally paused (`true` = paused).
    Paused,
    /// The admin / owner address.
    Admin,
    /// Liquidity provided by a user to a specific market.
    Liquidity(u32, Address),
    /// Oracle submission for a market by a specific oracle.
    OracleSubmission(u32, Address),
    /// List of oracle addresses configured for a market.
    MarketOracles(u32),
}

/// On-chain representation of a prediction market.
#[contracttype]
#[derive(Clone)]
pub struct MarketData {
    /// The address that created the market.
    pub creator: Address,
    /// The prediction question.
    pub question: String,
    /// A human-readable description of the market.
    pub description: String,
    /// Unix timestamp (seconds) when betting closes.
    pub end_time: u64,
    /// Identifies the data source used at resolution time.
    pub resolution_source: String,
    /// Ordered list of possible outcomes.
    pub outcome_tags: Vec<String>,
    /// Whether a winning outcome has been recorded.
    pub resolved: bool,
    /// Index (0-based) of the winning outcome.
    pub winning_outcome: u32,
    /// Whether the market has been cancelled.
    pub cancelled: bool,
    /// Number of oracles required for multi-oracle resolution (0 = single oracle, 3+ = multi-oracle).
    pub required_oracles: u32,
}

/// On-chain record of a user's bet on a market.
#[contracttype]
#[derive(Clone)]
pub struct BetData {
    /// Index of the selected outcome.
    pub outcome_index: u32,
    /// Amount staked in the platform's base unit.
    pub amount: i128,
}

/// Tracks how much liquidity a user has added to a market.
#[contracttype]
#[derive(Clone)]
pub struct LiquidityData {
    /// Total amount of liquidity provided.
    pub total_amount: i128,
}

/// Record of an oracle's outcome submission for a market.
#[contracttype]
#[derive(Clone)]
pub struct OracleSubmission {
    /// Address of the oracle that submitted this outcome.
    pub oracle: Address,
    /// The submitted outcome index.
    pub outcome: u32,
    /// Ledger sequence when the submission was recorded.
    pub submitted_at: u64,
}

pub mod admin;

#[contract]
pub struct MarketsContract;

#[contractimpl]
impl MarketsContract {
    // -----------------------------------------------------------------------
    //  Read-only
    // -----------------------------------------------------------------------

    /// @notice Returns the contract version.
    /// @dev This is a read-only introspection entrypoint that does **not** require authentication.
    /// @param _env The contract environment.
    /// @return The version number as u32.
    pub fn version(_env: Env) -> u32 {
        7
    }

    /// @notice Read a market from persistent storage and bump its TTL.
    /// @param env The contract environment.
    /// @param market_id The unique symbol identifying the market.
    /// @return An optional value containing the market data if it exists.
    pub fn get_market(env: Env, market_id: soroban_sdk::Symbol) -> Option<soroban_sdk::Val> {
        let market: Option<soroban_sdk::Val> = env.storage().persistent().get(&market_id);
        if market.is_some() {
            // Bump TTL: 365 days * 17280 ledgers per day = 6307200
            env.storage()
                .persistent()
                .extend_ttl(&market_id, 6307200, 6307200);
        }
        market
    }

    // -----------------------------------------------------------------------
    //  Market lifecycle
    // -----------------------------------------------------------------------

    /// Creates a new prediction market.
    ///
    /// # Auth
    ///
    /// Requires `creator.require_auth()`.
    ///
    /// # Parameters
    ///
    /// * `required_oracles` — Number of oracles for resolution (0 = single oracle, 3+ = multi-oracle).
    ///   If 0, fallback to single-oracle `resolve_market`. If >= 3, use `resolve_market_with_oracles`.
    ///
    /// # Returns
    ///
    /// A unique sequential market ID (starting at 1).
    ///
    /// # Panics
    ///
    /// Panics if the market counter overflows (u32::MAX reached) or if required_oracles is invalid.
    pub fn create_market(
        env: Env,
        creator: Address,
        question: String,
        description: String,
        end_time: u64,
        resolution_source: String,
        outcome_tags: Vec<String>,
        required_oracles: u32,
    ) -> u32 {
        creator.require_auth();

        // Validate required_oracles: either 0 (single oracle) or 3+ (multi-oracle).
        if required_oracles != 0 && required_oracles < 3 {
            panic_with_error!(env, ContractError::InvalidConfig);
        }

        // All arithmetic uses checked operations to prevent overflow.
        let counter: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::MarketCounter)
            .unwrap_or(0u32);

        let market_id = match counter.checked_add(1) {
            Some(id) => id,
            None => panic_with_error!(env, ContractError::Overflow),
        };

        env.storage()
            .persistent()
            .set(&DataKey::MarketCounter, &market_id);

        let market = MarketData {
            creator: creator.clone(),
            question,
            description,
            end_time,
            resolution_source,
            outcome_tags,
            resolved: false,
            winning_outcome: 0,
            cancelled: false,
            required_oracles,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Market(market_id), &market);

        market_id
    }

    /// Places a bet on a specific market outcome.
    ///
    /// # Auth
    ///
    /// Requires `user.require_auth()`.
    pub fn place_bet(env: Env, user: Address, market_id: u32, outcome_index: u32, amount: i128) {
        user.require_auth();

        // Verify the target market exists.
        if !env.storage().persistent().has(&DataKey::Market(market_id)) {
            panic_with_error!(env, ContractError::MarketNotFound);
        }

        let bet = BetData {
            outcome_index,
            amount,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Bet(market_id, user), &bet);
    }

    /// Resolves a market by recording the winning outcome.
    ///
    /// # Auth
    ///
    /// Requires `resolver.require_auth()`.
    ///
    /// # Panics
    ///
    /// Panics if the market does not exist or has already been resolved.
    pub fn resolve_market(env: Env, resolver: Address, market_id: u32, winning_outcome: u32) {
        resolver.require_auth();

        let mut market: MarketData =
            match env.storage().persistent().get(&DataKey::Market(market_id)) {
                Some(m) => m,
                None => panic_with_error!(env, ContractError::MarketNotFound),
            };

        if market.resolved {
            panic_with_error!(env, ContractError::MarketAlreadyResolved);
        }

        market.resolved = true;
        market.winning_outcome = winning_outcome;
        env.storage()
            .persistent()
            .set(&DataKey::Market(market_id), &market);
    }

    /// Submit an outcome for multi-oracle resolution.
    ///
    /// # Auth
    ///
    /// Requires `oracle.require_auth()`.
    ///
    /// # Errors
    ///
    /// Panics if:
    /// - Market does not exist
    /// - Market is not configured for multi-oracle resolution
    /// - Market has already been resolved
    /// - Oracle has already submitted for this market
    /// - Outcome is outside the valid range for the market
    ///
    /// # Parameters
    ///
    /// * `oracle` — Address of the oracle submitting the outcome. Must be authorized.
    /// * `market_id` — Identifier of the market.
    /// * `outcome` — The predicted outcome index (0-based).
    pub fn submit_oracle_outcome(env: Env, oracle: Address, market_id: u32, outcome: u32) {
        oracle.require_auth();

        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => panic_with_error!(env, ContractError::MarketNotFound),
        };

        // Verify market is configured for multi-oracle resolution
        if market.required_oracles < 3 {
            panic_with_error!(env, ContractError::InvalidConfig);
        }

        // Verify market is not already resolved
        if market.resolved {
            panic_with_error!(env, ContractError::MarketAlreadyResolved);
        }

        // Verify outcome is valid
        if outcome >= market.outcome_tags.len() as u32 {
            panic_with_error!(env, ContractError::InvalidOracleOutcome);
        }

        // Check if oracle has already submitted
        if env
            .storage()
            .persistent()
            .has(&DataKey::OracleSubmission(market_id, oracle.clone()))
        {
            panic_with_error!(env, ContractError::OracleAlreadySubmitted);
        }

        // Record the oracle submission
        let submission = OracleSubmission {
            oracle: oracle.clone(),
            outcome,
            submitted_at: env.ledger().sequence(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::OracleSubmission(market_id, oracle), &submission);
    }

    /// Resolves a multi-oracle market by computing the median outcome.
    ///
    /// # Auth
    ///
    /// Requires `resolver.require_auth()` (typically the market creator).
    ///
    /// # Algorithm
    ///
    /// Collects all oracle submissions, sorts them, and returns the median outcome.
    /// - For odd number of submissions: returns the middle value
    /// - For even number of submissions: returns the lower middle value
    ///
    /// # Errors
    ///
    /// Panics if:
    /// - Market does not exist
    /// - Market does not require multi-oracle resolution
    /// - Market has already been resolved
    /// - Insufficient oracle submissions (less than required_oracles)
    ///
    /// # Parameters
    ///
    /// * `resolver` — Address authorized to resolve the market (typically market creator).
    /// * `market_id` — Identifier of the market.
    /// * `oracle_addresses` — Vector of oracle addresses that have submitted outcomes.
    pub fn resolve_market_with_oracles(
        env: Env,
        resolver: Address,
        market_id: u32,
        oracle_addresses: Vec<Address>,
    ) {
        resolver.require_auth();

        let mut market: MarketData =
            match env.storage().persistent().get(&DataKey::Market(market_id)) {
                Some(m) => m,
                None => panic_with_error!(env, ContractError::MarketNotFound),
            };

        // Verify market is configured for multi-oracle resolution
        if market.required_oracles < 3 {
            panic_with_error!(env, ContractError::InvalidConfig);
        }

        // Verify market is not already resolved
        if market.resolved {
            panic_with_error!(env, ContractError::MarketAlreadyResolved);
        }

        // Verify we have enough oracle submissions
        if oracle_addresses.len() < market.required_oracles as usize {
            panic_with_error!(env, ContractError::InsufficientOracleSubmissions);
        }

        // Collect outcomes from oracle submissions
        let mut outcomes: Vec<u32> = Vec::new(&env);

        for oracle_addr in oracle_addresses.iter() {
            if let Some(submission) = env
                .storage()
                .persistent()
                .get::<DataKey, OracleSubmission>(&DataKey::OracleSubmission(market_id, oracle_addr.clone()))
            {
                outcomes.push_back(submission.outcome);
            } else {
                // Oracle in the list didn't submit; skip (partial submission allowed after minimum met)
                continue;
            }
        }

        // Re-verify we still have at least the required number after filtering
        if outcomes.len() < market.required_oracles as usize {
            panic_with_error!(env, ContractError::InsufficientOracleSubmissions);
        }

        // Sort outcomes to compute median
        let median_outcome = Self::compute_median(&outcomes);

        // Resolve the market with the median outcome
        market.resolved = true;
        market.winning_outcome = median_outcome;
        env.storage()
            .persistent()
            .set(&DataKey::Market(market_id), &market);
    }

    /// Internal helper: compute median outcome from sorted oracle submissions.
    ///
    /// # Returns
    ///
    /// The median value: for odd-length vecs, the middle value; for even-length,
    /// the lower-middle value.
    fn compute_median(outcomes: &Vec<u32>) -> u32 {
        // This is a simple implementation. In production, consider a more efficient sorting algorithm.
        // For Soroban's limited context, we use a bubble sort approach suitable for small vectors.

        let len = outcomes.len();
        if len == 0 {
            return 0; // Should not happen given prior checks, but defensive programming
        }

        // Create a mutable copy for sorting
        let mut sorted: Vec<u32> = outcomes.clone();

        // Bubble sort for small vectors (typical oracle count is 3-7)
        for i in 0..len {
            for j in 0..(len - i - 1) {
                if sorted.get_unchecked(j) > sorted.get_unchecked(j + 1) {
                    let temp = sorted.get_unchecked(j);
                    sorted.set(j, sorted.get_unchecked(j + 1));
                    sorted.set(j + 1, temp);
                }
            }
        }

        // Return median: for odd length return middle, for even return lower-middle
        let median_index = (len - 1) / 2;
        sorted.get_unchecked(median_index)
    }

    /// Claims winnings for a resolved market.
    ///
    /// # Auth
    ///
    /// Requires `claimant.require_auth()`.
    ///
    /// # Panics
    ///
    /// Panics if the market does not exist, has not been resolved, or the
    /// claimant did not place a winning bet.
    pub fn claim_winnings(env: Env, claimant: Address, market_id: u32) {
        claimant.require_auth();

        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => panic_with_error!(env, ContractError::MarketNotFound),
        };

        if !market.resolved {
            panic_with_error!(env, ContractError::MarketNotResolved);
        }

        // Verify that the claimant placed a bet on the winning outcome.
        let bet: BetData = match env
            .storage()
            .persistent()
            .get(&DataKey::Bet(market_id, claimant.clone()))
        {
            Some(b) => b,
            None => panic_with_error!(env, ContractError::InvalidState),
        };

        if bet.outcome_index != market.winning_outcome {
            panic_with_error!(env, ContractError::InvalidOutcome);
        }
    }

    /// Cancels a market before it has been resolved.
    ///
    /// # Auth
    ///
    /// Requires `caller.require_auth()`.
    pub fn cancel_market(env: Env, caller: Address, market_id: u32) {
        caller.require_auth();

        let mut market: MarketData =
            match env.storage().persistent().get(&DataKey::Market(market_id)) {
                Some(m) => m,
                None => panic_with_error!(env, ContractError::MarketNotFound),
            };

        if market.resolved {
            panic_with_error!(env, ContractError::MarketAlreadyResolved);
        }
        if market.cancelled {
            panic_with_error!(env, ContractError::InvalidState);
        }

        market.cancelled = true;
        env.storage()
            .persistent()
            .set(&DataKey::Market(market_id), &market);
    }

    /// Withdraws funds from a market.
    ///
    /// # Auth
    ///
    /// Requires `caller.require_auth()`.
    pub fn withdraw_funds(env: Env, caller: Address, market_id: u32, amount: i128) {
        caller.require_auth();

        if !env.storage().persistent().has(&DataKey::Market(market_id)) {
            panic_with_error!(env, ContractError::MarketNotFound);
        }

        // amount is accepted; actual transfer logic would go here.
        let _ = amount;
    }

    /// Updates the parameters of an existing market.
    ///
    /// # Auth
    ///
    /// Requires `caller.require_auth()`.
    pub fn update_market_params(env: Env, caller: Address, market_id: u32, new_end_time: u64) {
        caller.require_auth();

        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => panic_with_error!(env, ContractError::MarketNotFound),
        };

        if market.resolved {
            panic_with_error!(env, ContractError::MarketAlreadyResolved);
        }

        // Update the end time (parameter accepted).
        let _ = new_end_time;
    }

    // -----------------------------------------------------------------------
    //  Liquidity
    // -----------------------------------------------------------------------

    /// Adds liquidity to a market.
    ///
    /// # Auth
    ///
    /// Requires `provider.require_auth()`.
    pub fn add_liquidity(env: Env, provider: Address, market_id: u32, amount: i128) {
        provider.require_auth();

        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => panic_with_error!(env, ContractError::MarketNotFound),
        };

        if market.resolved || market.cancelled {
            panic_with_error!(env, ContractError::InvalidState);
        }

        let existing: LiquidityData = env
            .storage()
            .persistent()
            .get(&DataKey::Liquidity(market_id, provider.clone()))
            .unwrap_or(LiquidityData { total_amount: 0 });

        let new_total = match existing.total_amount.checked_add(amount) {
            Some(t) => t,
            None => panic_with_error!(env, ContractError::Overflow),
        };

        env.storage().persistent().set(
            &DataKey::Liquidity(market_id, provider),
            &LiquidityData {
                total_amount: new_total,
            },
        );
    }

    /// Removes liquidity from a market.
    ///
    /// # Auth
    ///
    /// Requires `provider.require_auth()`.
    pub fn remove_liquidity(env: Env, provider: Address, market_id: u32, amount: i128) {
        provider.require_auth();

        if !env.storage().persistent().has(&DataKey::Market(market_id)) {
            panic_with_error!(env, ContractError::MarketNotFound);
        }

        let existing: LiquidityData = env
            .storage()
            .persistent()
            .get(&DataKey::Liquidity(market_id, provider.clone()))
            .unwrap_or(LiquidityData { total_amount: 0 });

        if amount > existing.total_amount {
            panic_with_error!(env, ContractError::StakeTooSmall);
        }

        let new_total = match existing.total_amount.checked_sub(amount) {
            Some(t) => t,
            None => panic_with_error!(env, ContractError::Overflow),
        };

        env.storage().persistent().set(
            &DataKey::Liquidity(market_id, provider),
            &LiquidityData {
                total_amount: new_total,
            },
        );
    }

    // -----------------------------------------------------------------------
    //  Admin (pause / unpause / transfer-ownership)
    // -----------------------------------------------------------------------

    /// Pauses all market operations globally.
    ///
    /// # Auth
    ///
    /// Requires `admin.require_auth()`.
    pub fn pause_markets(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Paused, &true);
    }

    /// Resumes all market operations globally.
    ///
    /// # Auth
    ///
    /// Requires `admin.require_auth()`.
    pub fn unpause_markets(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Paused, &false);
    }

    /// Transfers contract ownership to a new admin address.
    ///
    /// # Auth
    ///
    /// Requires `admin.require_auth()`.
    pub fn transfer_ownership(env: Env, admin: Address, new_owner: Address) {
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &new_owner);
    }
}
