#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VersionContractState {
    Versioned,
    LegacyMissingVersion,
}

impl VersionContractState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Versioned => "versioned",
            Self::LegacyMissingVersion => "legacy_missing_version",
        }
    }
}

fn version_contract_state(required_tokens: usize, present_tokens: usize) -> VersionContractState {
    if present_tokens >= required_tokens {
        VersionContractState::Versioned
    } else {
        VersionContractState::LegacyMissingVersion
    }
}

/// 记录导入写操作采用的并发合同，不包含用户、会话或交易字段。
pub(crate) fn observe_import_version_contract(
    operation: &'static str,
    token_kind: &'static str,
    required_tokens: usize,
    present_tokens: usize,
) {
    if required_tokens == 0 {
        return;
    }
    let present_tokens = present_tokens.min(required_tokens);
    let state = version_contract_state(required_tokens, present_tokens);
    let missing_tokens = required_tokens - present_tokens;
    match state {
        VersionContractState::Versioned => tracing::debug!(
            domain = "import_contract",
            operation,
            token_kind,
            contract = state.as_str(),
            required_tokens,
            present_tokens,
            missing_tokens,
            "import mutation concurrency contract observed"
        ),
        VersionContractState::LegacyMissingVersion => tracing::warn!(
            domain = "import_contract",
            operation,
            token_kind,
            contract = state.as_str(),
            required_tokens,
            present_tokens,
            missing_tokens,
            "legacy import mutation omitted concurrency token"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_complete_partial_and_empty_token_sets() {
        assert_eq!(
            version_contract_state(1, 1),
            VersionContractState::Versioned
        );
        assert_eq!(
            version_contract_state(2, 1),
            VersionContractState::LegacyMissingVersion
        );
        assert_eq!(
            version_contract_state(1, 0),
            VersionContractState::LegacyMissingVersion
        );
        assert_eq!(
            version_contract_state(2, 3),
            VersionContractState::Versioned
        );
        assert_eq!(VersionContractState::Versioned.as_str(), "versioned");
        assert_eq!(
            VersionContractState::LegacyMissingVersion.as_str(),
            "legacy_missing_version"
        );
    }

    #[test]
    fn observation_accepts_not_applicable_and_both_contract_states() {
        observe_import_version_contract("test", "row_version", 0, 0);
        observe_import_version_contract("test", "row_version", 1, 1);
        observe_import_version_contract("test", "row_version", 2, 1);
    }
}
