use bill_analyser_core::{
    derive_ledger_balance_effects, LedgerBalanceInput, LedgerBalanceLeg, Money, TransactionType,
};

fn input(
    transaction_type: TransactionType,
    amount_cents: i64,
    destination_amount_cents: Option<i64>,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
) -> LedgerBalanceInput {
    LedgerBalanceInput {
        transaction_type,
        amount: Money::from_cents(amount_cents),
        destination_amount: destination_amount_cents.map(Money::from_cents),
        source_account_id,
        destination_account_id,
    }
}

fn leg(account_id: i64, delta_cents: i64) -> LedgerBalanceLeg {
    LedgerBalanceLeg {
        account_id,
        delta: Money::from_cents(delta_cents),
    }
}

#[test]
fn ledger_balance_effects_preserve_signed_source_and_destination_legs() {
    let cases = [
        (
            input(TransactionType::Income, -1_200, None, Some(11), Some(22)),
            Some(leg(11, 1_200)),
            None,
        ),
        (
            input(TransactionType::Expense, -1_200, None, Some(11), Some(22)),
            Some(leg(11, -1_200)),
            None,
        ),
        (
            input(
                TransactionType::Transfer,
                -1_200,
                Some(-1_500),
                Some(11),
                Some(22),
            ),
            Some(leg(11, -1_200)),
            Some(leg(22, 1_500)),
        ),
        (
            input(TransactionType::Investment, 1_200, None, Some(11), Some(22)),
            Some(leg(11, -1_200)),
            Some(leg(22, 1_200)),
        ),
        (
            input(
                TransactionType::Transfer,
                1_200,
                Some(0),
                Some(11),
                Some(22),
            ),
            Some(leg(11, -1_200)),
            Some(leg(22, 0)),
        ),
        (
            input(
                TransactionType::Transfer,
                1_200,
                Some(1_500),
                Some(11),
                Some(11),
            ),
            Some(leg(11, -1_200)),
            Some(leg(11, 1_500)),
        ),
    ];

    for (input, expected_source, expected_destination) in cases {
        let effects = derive_ledger_balance_effects(input).expect("valid balance facts");
        assert_eq!(effects.source(), expected_source);
        assert_eq!(effects.destination(), expected_destination);
        assert_eq!(
            effects.legs().collect::<Vec<_>>(),
            [expected_source, expected_destination]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn ledger_balance_effects_ignore_non_positive_account_ids() {
    let effects = derive_ledger_balance_effects(input(
        TransactionType::Transfer,
        1_200,
        Some(1_500),
        Some(0),
        Some(-22),
    ))
    .expect("non-positive account identifiers are absent legs");

    assert_eq!(effects.source(), None);
    assert_eq!(effects.destination(), None);
    assert!(effects.legs().next().is_none());
}

#[test]
fn ledger_balance_effects_reject_unrepresentable_absolute_amounts() {
    let source_error = derive_ledger_balance_effects(input(
        TransactionType::Expense,
        i64::MIN,
        None,
        Some(11),
        None,
    ))
    .expect_err("i64::MIN source amount must fail closed");
    assert_eq!(source_error.code.as_str(), "invalid_input");

    let destination_error = derive_ledger_balance_effects(input(
        TransactionType::Transfer,
        1_200,
        Some(i64::MIN),
        Some(11),
        Some(22),
    ))
    .expect_err("i64::MIN destination amount must fail closed");
    assert_eq!(destination_error.code.as_str(), "invalid_input");
}
