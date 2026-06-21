import type { CurrencyInfo } from '@/core/currency.ts';
import { A_TO_G_CURRENCIES } from './currency/aToG.ts';
import { H_TO_P_CURRENCIES } from './currency/hToP.ts';
import { Q_TO_Z_CURRENCIES } from './currency/qToZ.ts';

// ISO 4217
// Reference: https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/lists/list-one.xml
export const ALL_CURRENCIES: Record<string, CurrencyInfo> = {
    ...A_TO_G_CURRENCIES,
    ...H_TO_P_CURRENCIES,
    ...Q_TO_Z_CURRENCIES
};

export const DEFAULT_CURRENCY_SYMBOL: string = '¤';
export const DEFAULT_CURRENCY_CODE: string = (ALL_CURRENCIES['USD'] as CurrencyInfo).code;
export const PARENT_ACCOUNT_CURRENCY_PLACEHOLDER: string = '---';
