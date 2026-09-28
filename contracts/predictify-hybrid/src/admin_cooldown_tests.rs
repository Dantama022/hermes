use crate::voting::VotingValidator;

#[cfg(test)]
mod tests {
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn admin_action_respects_cooldown() {
        let env = Env::default();

        let admin = Address::generate(&env);

        VotingValidator::record_admin_action(&env);

        let result =
            VotingValidator::validate_admin_cooldown(&env);

        assert!(result.is_err());
    }

    #[test]
    fn admin_action_allowed_after_cooldown() {
        let env = Env::default();

        let admin = Address::generate(&env);

        VotingValidator::record_admin_action(&env);

        // Assert that a second call within 24 hours is rejected
        let result_within_cooldown =
            VotingValidator::validate_admin_cooldown(&env);
        assert!(result_within_cooldown.is_err());

        // Advance time beyond 24 hours
        env.ledger()
            .set_timestamp(
                env.ledger().timestamp() + 86401
            );

        // Assert that the call succeeds after cooldown expires
        let result_after_cooldown =
            VotingValidator::validate_admin_cooldown(&env);

        assert!(result_after_cooldown.is_ok());
    }
}