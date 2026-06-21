import type { CurrencyInfo } from '@/core/currency.ts';

export const H_TO_P_CURRENCIES: Record<string, CurrencyInfo> = {
    'HKD': { // Hong Kong Dollar
        code: 'HKD',
        fraction: 2,
        symbol: {
            normal: 'HK$'
        },
        unit: 'Dollar'
    },
    'HNL': { // Lempira
        code: 'HNL',
        fraction: 2,
        symbol: {
            normal: 'L'
        },
        unit: 'Lempira'
    },
    'HTG': { // Gourde
        code: 'HTG',
        fraction: 2,
        symbol: {
            normal: 'G'
        },
        unit: 'Gourde'
    },
    'HUF': { // Forint
        code: 'HUF',
        fraction: 2,
        symbol: {
            normal: 'Ft'
        },
        unit: 'Forint'
    },
    'IDR': { // Rupiah
        code: 'IDR',
        fraction: 2,
        symbol: {
            normal: 'Rp'
        },
        unit: 'Rupiah'
    },
    'ILS': { // New Israeli Sheqel
        code: 'ILS',
        fraction: 2,
        symbol: {
            normal: '₪'
        },
        unit: 'Shekel'
    },
    'INR': { // Indian Rupee
        code: 'INR',
        fraction: 2,
        symbol: {
            normal: '₹'
        },
        unit: 'Rupee'
    },
    'IQD': { // Iraqi Dinar
        code: 'IQD',
        fraction: 3,
        symbol: {
            normal: 'ID'
        },
        unit: 'Dinar'
    },
    'IRR': { // Iranian Rial
        code: 'IRR',
        fraction: 2,
        symbol: {
            normal: 'Rl',
            plural: 'Rls'
        },
        unit: 'Rial'
    },
    'ISK': { // Iceland Krona
        code: 'ISK',
        fraction: 0,
        symbol: {
            normal: 'kr'
        },
        unit: 'Krona'
    },
    'JMD': { // Jamaican Dollar
        code: 'JMD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'JOD': { // Jordanian Dinar
        code: 'JOD',
        fraction: 3,
        symbol: {
            normal: 'د.أ'
        },
        unit: 'Dinar'
    },
    'JPY': { // Yen
        code: 'JPY',
        fraction: 0,
        symbol: {
            normal: '¥'
        },
        unit: 'Yen'
    },
    'KES': { // Kenyan Shilling
        code: 'KES',
        fraction: 2,
        symbol: {
            normal: '/='
        },
        unit: 'Shilling'
    },
    'KGS': { // Som
        code: 'KGS',
        fraction: 2,
        symbol: {
            normal: '⃀'
        },
        unit: 'Som'
    },
    'KHR': { // Riel
        code: 'KHR',
        fraction: 2,
        symbol: {
            normal: '៛'
        },
        unit: 'Riel'
    },
    'KMF': { // Comorian Franc
        code: 'KMF',
        fraction: 0,
        symbol: {
            normal: 'CF'
        },
        unit: 'Franc'
    },
    'KPW': { // North Korean Won
        code: 'KPW',
        fraction: 2,
        symbol: {
            normal: '₩'
        },
        unit: 'Won'
    },
    'KRW': { // Won
        code: 'KRW',
        fraction: 0,
        symbol: {
            normal: '₩'
        },
        unit: 'Won'
    },
    'KWD': { // Kuwaiti Dinar
        code: 'KWD',
        fraction: 3,
        symbol: {
            normal: 'KD'
        },
        unit: 'Dinar'
    },
    'KYD': { // Cayman Islands Dollar
        code: 'KYD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'KZT': { // Tenge
        code: 'KZT',
        fraction: 2,
        symbol: {
            normal: '₸'
        },
        unit: 'Tenge'
    },
    'LAK': { // Lao Kip
        code: 'LAK',
        fraction: 2,
        symbol: {
            normal: '₭'
        },
        unit: 'Kip'
    },
    'LBP': { // Lebanese Pound
        code: 'LBP',
        fraction: 2,
        symbol: {
            normal: 'LL'
        },
        unit: 'Pound'
    },
    'LKR': { // Sri Lanka Rupee
        code: 'LKR',
        fraction: 2,
        symbol: {
            normal: 'රු'
        },
        unit: 'Rupee'
    },
    'LRD': { // Liberian Dollar
        code: 'LRD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'LSL': { // Loti
        code: 'LSL',
        fraction: 2,
        symbol: {
            normal: 'L',
            plural: 'M'
        },
        unit: 'Loti'
    },
    'LYD': { // Libyan Dinar
        code: 'LYD',
        fraction: 3,
        symbol: {
            normal: 'LD'
        },
        unit: 'Dinar'
    },
    'MAD': { // Moroccan Dirham
        code: 'MAD',
        fraction: 2,
        symbol: {
            normal: 'DH'
        },
        unit: 'Dirham'
    },
    'MDL': { // Moldovan Leu
        code: 'MDL',
        fraction: 2,
        symbol: {
            normal: 'L'
        },
        unit: 'Leu'
    },
    'MGA': { // Malagasy Ariary
        code: 'MGA',
        fraction: 2,
        symbol: {
            normal: 'Ar'
        },
        unit: 'Ariary'
    },
    'MKD': { // Denar
        code: 'MKD',
        fraction: 2,
        symbol: {
            normal: 'DEN'
        },
        unit: 'Denar'
    },
    'MMK': { // Kyat
        code: 'MMK',
        fraction: 2,
        symbol: {
            normal: 'K',
            plural: 'Ks.'
        },
        unit: 'Kyat'
    },
    'MNT': { // Tugrik
        code: 'MNT',
        fraction: 2,
        symbol: {
            normal: '₮'
        },
        unit: 'Tugrik'
    },
    'MOP': { // Pataca
        code: 'MOP',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Pataca'
    },
    'MRU': { // Ouguiya
        code: 'MRU',
        fraction: 2,
        symbol: {
            normal: 'UM'
        },
        unit: 'Ouguiya'
    },
    'MUR': { // Mauritius Rupee
        code: 'MUR',
        fraction: 2,
        symbol: {
            normal: 'Re.',
            plural: 'Rs.'
        },
        unit: 'Rupee'
    },
    'MVR': { // Rufiyaa
        code: 'MVR',
        fraction: 2,
        symbol: {
            normal: 'Rf.'
        },
        unit: 'Rufiyaa'
    },
    'MWK': { // Malawi Kwacha
        code: 'MWK',
        fraction: 2,
        symbol: {
            normal: 'K'
        },
        unit: 'Kwacha'
    },
    'MXN': { // Mexican Peso
        code: 'MXN',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Peso'
    },
    'MYR': { // Malaysian Ringgit
        code: 'MYR',
        fraction: 2,
        symbol: {
            normal: 'RM'
        },
        unit: 'Ringgit'
    },
    'MZN': { // Mozambique Metical
        code: 'MZN',
        fraction: 2,
        symbol: {
            normal: 'MT'
        },
        unit: 'Metical'
    },
    'NAD': { // Namibia Dollar
        code: 'NAD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'NGN': { // Naira
        code: 'NGN',
        fraction: 2,
        symbol: {
            normal: '₦'
        },
        unit: 'Naira'
    },
    'NIO': { // Cordoba Oro
        code: 'NIO',
        fraction: 2,
        symbol: {
            normal: 'C$'
        },
        unit: 'Cordoba'
    },
    'NOK': { // Norwegian Krone
        code: 'NOK',
        fraction: 2,
        symbol: {
            normal: 'kr'
        },
        unit: 'Krone'
    },
    'NPR': { // Nepalese Rupee
        code: 'NPR',
        fraction: 2,
        symbol: {
            normal: 'रु'
        },
        unit: 'Rupee'
    },
    'NZD': { // New Zealand Dollar
        code: 'NZD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'OMR': { // Rial Omani
        code: 'OMR',
        fraction: 3,
        symbol: {
            normal: 'R.O'
        },
        unit: 'Rial'
    },
    'PAB': { // Balboa
        code: 'PAB',
        fraction: 2,
        symbol: {
            normal: 'B/.'
        },
        unit: 'Balboa'
    },
    'PEN': { // Sol
        code: 'PEN',
        fraction: 2,
        symbol: {
            normal: 'S/'
        },
        unit: 'Sol'
    },
    'PGK': { // Kina
        code: 'PGK',
        fraction: 2,
        symbol: {
            normal: 'K'
        },
        unit: 'Kina'
    },
    'PHP': { // Philippine Peso
        code: 'PHP',
        fraction: 2,
        symbol: {
            normal: '₱'
        },
        unit: 'Peso'
    },
    'PKR': { // Pakistan Rupee
        code: 'PKR',
        fraction: 2,
        symbol: {
            normal: 'Re.',
            plural: 'Rs.'
        },
        unit: 'Rupee'
    },
    'PLN': { // Zloty
        code: 'PLN',
        fraction: 2,
        symbol: {
            normal: 'zł'
        },
        unit: 'Zloty'
    },
    'PYG': { // Guarani
        code: 'PYG',
        fraction: 0,
        symbol: {
            normal: '₲'
        },
        unit: 'Guarani'
    },
};
