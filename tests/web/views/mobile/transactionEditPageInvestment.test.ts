import fs from 'node:fs';
import path from 'node:path';

const EDIT_PAGE_PATH = 'src/views/mobile/transactions/EditPage.vue';

function readSource(): string {
    return fs.readFileSync(path.resolve(process.cwd(), EDIT_PAGE_PATH), 'utf-8');
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
        expect(source).toMatch(/Investment Amount[\s\S]+?transaction\.type === TransactionType\.Investment[\s\S]+?v-model="transaction\.destinationAmount"/);
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

    test('does not regress existing Expense / Income / Transfer / ModifyBalance branches', () => {
        const source = readSource();
        expect(source).toContain('v-model="transaction.expenseCategoryId"');
        expect(source).toContain('v-model="transaction.incomeCategoryId"');
        expect(source).toContain('v-model="transaction.transferCategoryId"');
        expect(source).toContain('TransactionType.ModifyBalance');
        expect(source).toContain('transaction.type === TransactionType.Transfer');
    });
});
