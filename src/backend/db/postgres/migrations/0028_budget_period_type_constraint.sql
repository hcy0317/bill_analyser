ALTER TABLE budgets
    ADD CONSTRAINT chk_budgets_period_type
    CHECK (
        period_type IN ('daily', 'weekly', 'monthly', 'quarterly', 'yearly')
    ) NOT VALID;

ALTER TABLE budgets
    VALIDATE CONSTRAINT chk_budgets_period_type;
