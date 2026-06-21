import { readSource as readPlainSource, readVueSourceWithExternalBlocks } from '../../helpers/vueSource';

const LIST_PAGE_PATH = 'src/views/mobile/transactions/ListPage.vue';
const MONTH_BLOCK_PATH = 'src/views/mobile/transactions/components/MobileTransactionMonthBlock.vue';
const AMOUNT_FILTER_PAGE_PATH = 'src/views/mobile/transactions/AmountFilterPage.vue';

function readSource(relativePath: string): string {
    return relativePath === LIST_PAGE_PATH
        ? readVueSourceWithExternalBlocks(relativePath)
        : readPlainSource(relativePath);
}

describe('mobile ListPage.vue Investment parity (S3)', () => {
    test('category icon renderer is type-agnostic and resolves icon/color from transaction.category', () => {
        const source = readSource(MONTH_BLOCK_PATH);
        // Single ItemIcon binding driven by transaction.category — covers Income, Expense, Transfer
        // and Investment alike. There is no per-type case statement that would silently drop Investment.
        expect(source).toContain('icon-type="category"');
        expect(source).toContain(':icon-id="transaction.category.icon"');
        expect(source).toContain(':color="transaction.category.color"');
        expect(source).toContain('v-if="transaction.category && transaction.category.color"');
    });

    test('destination account row treats Investment like Transfer (parity with desktop)', () => {
        const source = readSource(MONTH_BLOCK_PATH);
        // Both the arrow icon and the destination account name must show for Investment, not just Transfer.
        expect(source).toMatch(
            /transaction\.type === TransactionType\.Transfer \|\| transaction\.type === TransactionType\.Investment[\s\S]+?transaction\.destinationAccount\.name/
        );
    });

    test('More popover Type filter exposes Investment (type 5)', () => {
        const source = readSource(LIST_PAGE_PATH);
        expect(source).toMatch(
            /query\.type === 5[\s\S]+?:title="tt\('Investment'\)"[\s\S]+?changeTypeFilter\(5\)/
        );
        // sanity: Transfer (type 4) should still be present
        expect(source).toMatch(/changeTypeFilter\(4\)/);
    });

    test('TransactionType is imported so the Investment comparison resolves at runtime', () => {
        const source = readSource(MONTH_BLOCK_PATH);
        expect(source).toMatch(/import\s*\{[^}]*TransactionType[^}]*\}\s*from\s*'@\/core\/transaction\.ts'/);
    });
});

describe('mobile AmountFilterPage.vue does not exclude Investment (S3)', () => {
    test('iterates all AmountFilterType values without filtering by transaction type', () => {
        const source = readSource(AMOUNT_FILTER_PAGE_PATH);
        // The filter type list iterates AmountFilterType.values() — same source as desktop —
        // and there is no explicit transaction-type branch that would drop Investment.
        expect(source).toContain('AmountFilterType.values()');
        expect(source).not.toMatch(/TransactionType\.Investment/);
        expect(source).not.toMatch(/excludeInvestment/i);
    });

    test('confirm() persists amountFilterCents through transactionsStore without type-set restriction', () => {
        const source = readSource(AMOUNT_FILTER_PAGE_PATH);
        expect(source).toContain('transactionsStore.updateTransactionListFilter');
        expect(source).toContain('amountFilterCents: amountFilterCents');
        // No silent type narrowing
        expect(source).not.toMatch(/type:\s*TransactionType\.(Expense|Income|Transfer)/);
    });
});
