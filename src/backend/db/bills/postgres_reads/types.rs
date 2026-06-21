#[derive(Debug, Clone)]
pub struct PostgresReconciliationAccount {
    pub name: String,
    pub initial_balance: Money,
}
