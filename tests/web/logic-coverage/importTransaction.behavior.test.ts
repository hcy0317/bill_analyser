import { describe, expect, test } from '@jest/globals';

import {
    ImportTransactionColumnType,
    ImportTransactionDataMapping,
    ImportTransactionReplaceRule,
    ImportTransactionReplaceRules
} from '@/core/import_transaction.ts';
import { KnownAmountFormat } from '@/core/numeral.ts';
import { TransactionType } from '@/core/transaction.ts';

function mapColumn(mapping: ImportTransactionDataMapping, type: ImportTransactionColumnType, index: number): void {
    mapping.toggleDataMappingColumn(index, type.type);
}

describe('ImportTransactionColumnType', () => {
    test('exposes the complete stable column catalogue', () => {
        const columns = ImportTransactionColumnType.values();

        expect(columns).toHaveLength(14);
        expect(columns.map(column => column.type)).toStrictEqual(Array.from({ length: 14 }, (_, index) => index + 1));
        expect(columns[0]).toBe(ImportTransactionColumnType.TransactionTime);
        expect(columns[13]).toBe(ImportTransactionColumnType.Description);
        expect(columns.map(column => column.name)).toContain('Transfer In Amount');
    });
});

describe('ImportTransactionDataMapping', () => {
    test('starts with documented defaults and toggles header and unique column assignments', () => {
        const mapping = ImportTransactionDataMapping.createEmpty();

        expect(mapping).toMatchObject({
            includeHeader: true,
            dataColumnMapping: {},
            transactionTypeMapping: {},
            timeFormat: '',
            timezoneFormat: '',
            amountFormat: '',
            geoLocationSeparator: ' ',
            geoLocationOrder: 'lonlat',
            tagSeparator: ';'
        });
        expect(mapping.isColumnMappingSet(ImportTransactionColumnType.Amount)).toBe(false);

        mapping.toggleIncludeHeader();
        mapColumn(mapping, ImportTransactionColumnType.Amount, 2);
        expect(mapping.includeHeader).toBe(false);
        expect(mapping.isColumnMappingSet(ImportTransactionColumnType.Amount)).toBe(true);

        mapColumn(mapping, ImportTransactionColumnType.TransactionTime, 2);
        expect(mapping.dataColumnMapping).toStrictEqual({
            [ImportTransactionColumnType.TransactionTime.type]: 2
        });

        mapColumn(mapping, ImportTransactionColumnType.TransactionTime, 2);
        expect(mapping.dataColumnMapping).toStrictEqual({});
    });

    test('formats geographic coordinates in both supported orders', () => {
        const mapping = ImportTransactionDataMapping.createEmpty();

        expect(mapping.formatGeoLocation('31.2', '121.5')).toBe('121.5 31.2');
        mapping.setGeoLocationFormat(',', 'latlon');
        expect(mapping.formatGeoLocation('31.2', '121.5')).toBe('31.2,121.5');
    });

    test('collects unique transaction types while respecting header and malformed rows', () => {
        const mapping = ImportTransactionDataMapping.createEmpty();
        mapColumn(mapping, ImportTransactionColumnType.TransactionType, 1);
        const rows = [
            ['time', 'type'],
            ['2024-01-01', '支出'],
            ['short'],
            undefined,
            ['2024-01-02', ''],
            ['2024-01-03', '收入'],
            ['2024-01-04', '支出']
        ] as unknown as string[][];

        expect(mapping.parseFileAllTransactionTypes(rows)).toStrictEqual(['支出', '收入']);
        mapping.toggleIncludeHeader();
        expect(mapping.parseFileAllTransactionTypes([['row', 'header-is-now-data']])).toStrictEqual(['header-is-now-data']);
    });

    test.each([undefined, [], [['only-data']]])('returns no transaction types for unusable input %#', fileData => {
        const mapping = ImportTransactionDataMapping.createEmpty();
        expect(mapping.parseFileAllTransactionTypes(fileData as string[][] | undefined)).toStrictEqual([]);
    });

    test('keeps only valid mapped transaction enum values', () => {
        const mapping = ImportTransactionDataMapping.createEmpty();
        mapColumn(mapping, ImportTransactionColumnType.TransactionType, 0);
        mapping.transactionTypeMapping = {
            balance: TransactionType.ModifyBalance,
            expense: TransactionType.Expense,
            investment: TransactionType.Investment,
            tooSmall: 0 as TransactionType,
            tooLarge: 6 as TransactionType
        };

        expect(mapping.parseFileValidMappedTransactionTypes([['expense']])).toStrictEqual({
            balance: TransactionType.ModifyBalance,
            expense: TransactionType.Expense,
            investment: TransactionType.Investment
        });

        mapping.transactionTypeMapping = null as unknown as Record<string, TransactionType>;
        expect(mapping.parseFileValidMappedTransactionTypes([['expense']])).toStrictEqual({});
        expect(ImportTransactionDataMapping.createEmpty().parseFileValidMappedTransactionTypes([['expense']])).toStrictEqual({});
    });

    test('auto-detects a unique time format and rejects ambiguous or invalid samples', () => {
        const mapping = ImportTransactionDataMapping.createEmpty();
        mapColumn(mapping, ImportTransactionColumnType.TransactionTime, 1);

        expect(mapping.parseFileAutoDetectedTimeFormat([
            ['header', 'time'],
            ['a', '2024-01-02'],
            ['too-short'],
            undefined,
            ['b', '2024-12-31']
        ] as unknown as string[][])).toBe('YYYY-MM-DD');
        expect(mapping.parseFileAutoDetectedTimeFormat([['header', 'time'], ['a', '01/02/2024']])).toBeUndefined();
        expect(mapping.parseFileAutoDetectedTimeFormat([['header', 'time'], ['a', 'not-a-date']])).toBeUndefined();
        expect(ImportTransactionDataMapping.createEmpty().parseFileAutoDetectedTimeFormat([['2024-01-01']])).toBeUndefined();
    });

    test('auto-detects timezone and amount formats from populated mapped rows', () => {
        const mapping = ImportTransactionDataMapping.createEmpty();
        mapColumn(mapping, ImportTransactionColumnType.TransactionTimezone, 1);
        mapColumn(mapping, ImportTransactionColumnType.Amount, 2);
        const rows = [
            ['description', 'timezone', 'amount'],
            ['a', '+08:30', '1,234.56'],
            ['short'],
            undefined,
            ['blank', '', ''],
            ['b', '-05:00', '-2,000.00']
        ] as unknown as string[][];

        expect(mapping.parseFileAutoDetectedTimezoneFormat(rows)).toBe('Z');
        expect(mapping.parseFileAutoDetectedAmountFormat(rows)).toBe(KnownAmountFormat.DotDecimalSeparatorWithCommaGroupingSymbol.type);

        expect(mapping.parseFileAutoDetectedTimezoneFormat([['h', 'tz'], ['a', 'invalid']])).toBeUndefined();
        expect(mapping.parseFileAutoDetectedAmountFormat([['h', 'tz', 'amount'], ['a', 'Z', 'invalid']])).toBeUndefined();
        expect(ImportTransactionDataMapping.createEmpty().parseFileAutoDetectedTimezoneFormat([['+08:00']])).toBeUndefined();
        expect(ImportTransactionDataMapping.createEmpty().parseFileAutoDetectedAmountFormat([['1.00']])).toBeUndefined();
    });

    test('round-trips all options and falls back field-by-field for sparse JSON', () => {
        const mapping = ImportTransactionDataMapping.createEmpty();
        mapping.includeHeader = false;
        mapping.dataColumnMapping = { [ImportTransactionColumnType.Amount.type]: 3 };
        mapping.transactionTypeMapping = { expense: TransactionType.Expense };
        mapping.timeFormat = 'YYYY-MM-DD';
        mapping.timezoneFormat = 'Z';
        mapping.amountFormat = '1-2';
        mapping.geoLocationSeparator = ',';
        mapping.geoLocationOrder = 'latlon';
        mapping.tagSeparator = '|';

        expect(ImportTransactionDataMapping.parseFromJson(mapping.toJson())).toStrictEqual(mapping);
        expect(ImportTransactionDataMapping.parseFromJson(JSON.stringify({
            bill_analyserImportTransactionDataMapping: { includeHeader: false }
        }))).toMatchObject({
            includeHeader: false,
            dataColumnMapping: {},
            transactionTypeMapping: {},
            timeFormat: '',
            timezoneFormat: '',
            amountFormat: '',
            geoLocationSeparator: ' ',
            geoLocationOrder: 'lonlat',
            tagSeparator: ';'
        });
        expect(ImportTransactionDataMapping.parseFromJson('{}')).toBeNull();
        expect(ImportTransactionDataMapping.parseFromJson('{')).toBeNull();
    });

    test('reset restores every mutable option', () => {
        const mapping = ImportTransactionDataMapping.parseFromJson(JSON.stringify({
            bill_analyserImportTransactionDataMapping: {
                includeHeader: false,
                dataColumnMapping: { 1: 9 },
                transactionTypeMapping: { a: TransactionType.Transfer },
                timeFormat: 'x',
                timezoneFormat: 'y',
                amountFormat: 'z',
                geoLocationSeparator: ',',
                geoLocationOrder: 'latlon',
                tagSeparator: '|'
            }
        }))!;

        mapping.reset();
        expect(mapping).toStrictEqual(ImportTransactionDataMapping.createEmpty());
    });
});

describe('ImportTransactionReplaceRule and ImportTransactionReplaceRules', () => {
    test.each(['expenseCategory', 'incomeCategory', 'transferCategory', 'account', 'tag'] as const)(
        'round-trips the %s replace rule type',
        dataType => {
            const rule = ImportTransactionReplaceRule.of(dataType, 'source', 'target');
            expect(rule.toJsonObject()).toStrictEqual({ type: dataType, sourceValue: 'source', targetId: 'target' });
            expect(ImportTransactionReplaceRule.parse(rule.toJsonObject())).toStrictEqual(rule);
        }
    );

    test.each([
        null,
        'rule',
        {},
        { type: 'tag' },
        { type: 'tag', sourceValue: 's' },
        { type: 1, sourceValue: 's', targetId: 't' },
        { type: 'tag', sourceValue: 1, targetId: 't' },
        { type: 'tag', sourceValue: 's', targetId: 1 },
        { type: 'unsupported', sourceValue: 's', targetId: 't' }
    ])('rejects malformed replace rule %#', candidate => {
        expect(ImportTransactionReplaceRule.parse(candidate)).toBeNull();
    });

    test('serializes a list and ignores invalid entries when parsing', () => {
        const expense = ImportTransactionReplaceRule.of('expenseCategory', 'food', 'cat-1');
        const tag = ImportTransactionReplaceRule.of('tag', 'old', 'tag-1');
        const rules = ImportTransactionReplaceRules.of([expense, tag]);

        expect(ImportTransactionReplaceRules.parseFromJson(rules.toJson())?.getRules()).toStrictEqual([expense, tag]);
        expect(ImportTransactionReplaceRules.parseFromJson(JSON.stringify({
            bill_analyserImportTransactionReplaceRules: [expense.toJsonObject(), { nope: true }, tag.toJsonObject()]
        }))?.getRules()).toStrictEqual([expense, tag]);
    });

    test.each([
        '{}',
        '{',
        JSON.stringify({ bill_analyserImportTransactionReplaceRules: 1 }),
        JSON.stringify({ bill_analyserImportTransactionReplaceRules: null })
    ])('rejects invalid rule collection %#', json => {
        expect(ImportTransactionReplaceRules.parseFromJson(json)).toBeNull();
    });
});
