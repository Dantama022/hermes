use soroban_sdk::contracterror;

/// A stable catalog of errors for the markets smart contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    /// Action is unauthorized; typically thrown when an admin action is invoked by a non-admin.
    Unauthorized = 1,
    /// Market not found; thrown when querying or interacting with a non-existent market.
    MarketNotFound = 2,
    /// Market is closed; thrown when trying to interact with a market that has already ended.
    MarketClosed = 3,
    /// Market already resolved; thrown when attempting to resolve a market more than once.
    MarketAlreadyResolved = 4,
    /// Market not resolved; thrown when attempting to claim winnings before resolution.
    MarketNotResolved = 5,
    /// Invalid outcome; thrown when the provided outcome is not supported by the market.
    InvalidOutcome = 6,
    /// Invalid configuration; thrown when market setup parameters are out of bounds.
    InvalidConfig = 7,
    /// Overflow; thrown when math operations overflow, preventing unsafe state changes.
    Overflow = 8,
    /// Stake too small; thrown when a deposit or bet is below the minimum threshold.
    StakeTooSmall = 9,
    /// Invalid State; thrown when a generic state transition fails.
    InvalidState = 10,
    /// Admin action timelocked; thrown when an admin action is invoked before the cooldown has elapsed.
    AdminActionTimelocked = 11,
    // ===== ADMIN-SPECIFIC ERROR VARIANTS (12-19) =====
    /// Admin cooldown period is still active; the action cannot be executed yet.
    AdminCooldownActive = 12,
    /// The provided admin address is invalid (e.g., zero address or self-referential).
    AdminAddressInvalid = 13,
    /// The requested admin operation is not permitted in the current contract state.
    AdminOperationNotPermitted = 14,
    // ===== MULTI-ORACLE ERROR VARIANTS (15-19) =====
    /// Insufficient oracle submissions; not enough oracles have submitted data.
    InsufficientOracleSubmissions = 15,
    /// Oracle already submitted; an oracle has already provided data for this market.
    OracleAlreadySubmitted = 16,
    /// Invalid oracle outcome; the oracle submitted an outcome outside valid range.
    InvalidOracleOutcome = 17,
    /// Duplicate oracle; attempted to add the same oracle address multiple times.
    DuplicateOracle = 18,
    /// Insufficient oracles configured; market requires at least 3 oracles for multi-oracle resolution.
    InsufficientOraclesConfigured = 19,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_variants() {
        assert_eq!(ContractError::Unauthorized as u32, 1);
        assert_eq!(ContractError::MarketNotFound as u32, 2);
        assert_eq!(ContractError::MarketClosed as u32, 3);
        assert_eq!(ContractError::MarketAlreadyResolved as u32, 4);
        assert_eq!(ContractError::MarketNotResolved as u32, 5);
        assert_eq!(ContractError::InvalidOutcome as u32, 6);
        assert_eq!(ContractError::InvalidConfig as u32, 7);
        assert_eq!(ContractError::Overflow as u32, 8);
        assert_eq!(ContractError::StakeTooSmall as u32, 9);
        assert_eq!(ContractError::InvalidState as u32, 10);
        assert_eq!(ContractError::AdminActionTimelocked as u32, 11);
        assert_eq!(ContractError::AdminCooldownActive as u32, 12);
        assert_eq!(ContractError::AdminAddressInvalid as u32, 13);
        assert_eq!(ContractError::AdminOperationNotPermitted as u32, 14);
        assert_eq!(ContractError::InsufficientOracleSubmissions as u32, 15);
        assert_eq!(ContractError::OracleAlreadySubmitted as u32, 16);
        assert_eq!(ContractError::InvalidOracleOutcome as u32, 17);
        assert_eq!(ContractError::DuplicateOracle as u32, 18);
        assert_eq!(ContractError::InsufficientOraclesConfigured as u32, 19);
    }
}
