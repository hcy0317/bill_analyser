import { readSource as readPlainSource, readVueSourceWithExternalBlocks } from '../../helpers/vueSource';

const EDIT_PAGE_PATH = 'src/views/mobile/transactions/EditPage.vue';
const EDIT_PAGE_DISPLAY_HELPERS_PATH = 'src/views/mobile/transactions/edit-page/displayHelpers.ts';

function readSource(): string {
    return [
        readVueSourceWithExternalBlocks(EDIT_PAGE_PATH),
        readPlainSource(EDIT_PAGE_DISPLAY_HELPERS_PATH)
    ].join('\n');
}

describe('mobile EditPage.vue Investment branch parity (S2)', () => {
    test('renders Investment as the 5th segmented type button', () => {
        const source = readSource();
        expect(source).toContain(":text=\"tt('Investment')\"");
        expect(source).toContain('transaction.type = TransactionType.Investment');
        // Investment must coexist with Expense / Income / Transfer / ModifyBalance
        expect(source).toContain(":text=\"tt('Expense')\"");
        expect(source).toContain(":text=\"tt('Income')\"");
        expect(source).toContain(":text=\"tt('Transfer')\"");
        expect(source).toContain(":text=\"tt('Modify Balance')\"");
    });

    test('imports hasAvailableInvestmentCategories from base composable', () => {
        const source = readSource();
        expect(source).toMatch(/hasAvailableInvestmentCategories\s*,/);
    });

    test('renders destination amount input for Investment with Investment Amount label', () => {
        const source = readSource();
        expect(source).toMatch(/Investment Amount[\s\S]+?transaction\.type === TransactionType\.Investment[\s\S]+?v-model="transaction\.destinationAmountCents"/);
    });

    test('renders investment category picker bound to investmentCategoryId and CategoryType.Investment', () => {
        const source = readSource();
        expect(source).toContain('v-model="transaction.investmentCategoryId"');
        expect(source).toContain(':items="allCategories[CategoryType.Investment]"');
        expect(source).toContain('key="investmentCategorySelection"');
    });

    test('renders Investment Account destination selector that writes to destinationAccountId', () => {
        const source = readSource();
        expect(source).toMatch(/tt\('Investment Account'\)[\s\S]+?transaction\.type === TransactionType\.Investment[\s\S]+?v-model="transaction\.destinationAccountId"/);
    });

    test('reset watcher clears investmentCategoryId when transaction type changes', () => {
        const source = readSource();
        expect(source).toContain("transaction.value.investmentCategoryId = ''");
    });

    test('hasAvailableCategoriesForType handles Investment branch', () => {
        const source = readSource();
        expect(source).toMatch(/type === TransactionType\.Investment\)\s*\{[\s\S]+?hasAvailableInvestmentCategories\.value/);
    });

    test('init queryType range accepts Investment (<= TransactionType.Investment)', () => {
        const source = readSource();
        expect(source).toContain('queryType <= TransactionType.Investment');
        expect(source).not.toContain('queryType <= TransactionType.Transfer');
    });

    test('parses cents query parameters as strict integers', () => {
        const source = readSource();
        expect(source).toContain('const INTEGER_CENTS_PATTERN = /^[+-]?\\d+$/u;');
        expect(source).toContain('function parseStrictQueryCents(value: unknown): number | undefined');
        expect(source).toContain('Number.isSafeInteger(parsed) ? parsed : undefined');
        expect(source).toContain("sourceAmountCents: parseStrictQueryCents(query['sourceAmountCents'])");
        expect(source).toContain("destinationAmountCents: parseStrictQueryCents(query['destinationAmountCents'])");
        expect(source).toContain("const initAmount: number | undefined = parseStrictQueryCents(query['sourceAmountCents']);");
        expect(source).not.toContain("parseInt(query['sourceAmountCents'])");
        expect(source).not.toContain("parseInt(query['destinationAmountCents'])");
    });

    test('does not regress existing Expense / Income / Transfer / ModifyBalance branches', () => {
        const source = readSource();
        expect(source).toContain('v-model="transaction.expenseCategoryId"');
        expect(source).toContain('v-model="transaction.incomeCategoryId"');
        expect(source).toContain('v-model="transaction.transferCategoryId"');
        expect(source).toContain('TransactionType.ModifyBalance');
        expect(source).toContain('transaction.type === TransactionType.Transfer');
    });

    test('uses the modern mobile transaction composer visual contract', () => {
        const source = readSource();
        expect(source).toContain('transaction-composer-surface');
        expect(source).toContain('transaction-composer-validation');
        expect(source).toContain('transaction-save-button');
        expect(source).toContain('font-variant-numeric: tabular-nums');
        expect(source).toContain('data-testid="mobile.transactions.edit.account-empty"');
        expect(source).toContain('data-testid="mobile.transactions.edit.action.add-account"');
        expect(source).toContain(':disabled="inputIsEmpty || submitting"');
        expect(source.match(/data-testid="mobile\.transactions\.edit\.action\.save"/gu)).toHaveLength(1);
    });
});
