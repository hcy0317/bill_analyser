use crate::{
    error::{ErrorCode, RuntimeError},
    primitives::{Money, TransactionType},
};

pub fn backend_transaction_type_name(transaction_type: TransactionType) -> &'static str {
    transaction_type.backend_name()
}

pub fn frontend_transaction_type_from_backend(
    raw_value: &str,
) -> Result<TransactionType, RuntimeError> {
    match raw_value {
        "收入" => Ok(TransactionType::Income),
        "支出" => Ok(TransactionType::Expense),
        "转账" => Ok(TransactionType::Transfer),
        "投资" => Ok(TransactionType::Investment),
        _ => Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            "invalid backend transaction type",
        )),
    }
}

pub fn signed_backend_amount(transaction_type: TransactionType, source_amount: Money) -> Money {
    if matches!(
        transaction_type,
        TransactionType::Expense | TransactionType::Transfer | TransactionType::Investment
    ) && source_amount.is_positive()
    {
        source_amount
            .checked_negated()
            .expect("positive money values can always be negated")
    } else {
        source_amount
    }
}
