import type { CurrencyInfo } from '@/core/currency.ts';

export const Q_TO_Z_CURRENCIES: Record<string, CurrencyInfo> = {
    'QAR': { // Qatari Rial
        code: 'QAR',
        fraction: 2,
        symbol: {
            normal: 'QR'
        },
        unit: 'Rial'
    },
    'RON': { // Romanian Leu
        code: 'RON',
        fraction: 2,
        symbol: {
            normal: 'L'
        },
        unit: 'Leu'
    },
    'RSD': { // Serbian Dinar
        code: 'RSD',
        fraction: 2,
        symbol: {
            normal: 'din.'
        },
        unit: 'Dinar'
    },
    'RUB': { // Russian Ruble
        code: 'RUB',
        fraction: 2,
        symbol: {
            normal: '₽'
        },
        unit: 'Ruble'
    },
    'RWF': { // Rwanda Franc
        code: 'RWF',
        fraction: 0,
        symbol: {
            normal: 'FRw'
        },
        unit: 'Franc'
    },
    'SAR': { // Saudi Riyal
        code: 'SAR',
        fraction: 2,
        symbol: {
            normal: 'SAR'
        },
        unit: 'Riyal'
    },
    'SBD': { // Solomon Islands Dollar
        code: 'SBD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'SCR': { // Seychelles Rupee
        code: 'SCR',
        fraction: 2,
        symbol: {
            normal: 'Re.',
            plural: 'Rs.'
        },
        unit: 'Rupee'
    },
    'SDG': { // Sudanese Pound
        code: 'SDG',
        fraction: 2,
        symbol: {
            normal: 'LS'
        },
        unit: 'Pound'
    },
    'SEK': { // Swedish Krona
        code: 'SEK',
        fraction: 2,
        symbol: {
            normal: 'kr'
        },
        unit: 'Krona'
    },
    'SGD': { // Singapore Dollar
        code: 'SGD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'SHP': { // Saint Helena Pound
        code: 'SHP',
        fraction: 2,
        symbol: {
            normal: '£'
        },
        unit: 'Pound'
    },
    'SLE': { // Leone
        code: 'SLE',
        fraction: 2,
        symbol: {
            normal: 'Le'
        },
        unit: 'Leone'
    },
    'SOS': { // Somali Shilling
        code: 'SOS',
        fraction: 2,
        symbol: {
            normal: 'Sh.So.'
        },
        unit: 'Shilling'
    },
    'SRD': { // Surinam Dollar
        code: 'SRD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'SSP': { // South Sudanese Pound
        code: 'SSP',
        fraction: 2,
        symbol: {
            normal: 'SS£'
        },
        unit: 'Pound'
    },
    'STN': { // Dobra
        code: 'STN',
        fraction: 2,
        symbol: {
            normal: 'Db'
        },
        unit: 'Dobra'
    },
    'SVC': { // El Salvador Colon
        code: 'SVC',
        fraction: 2,
        symbol: {
            normal: '₡'
        },
        unit: 'Colon'
    },
    'SYP': { // Syrian Pound
        code: 'SYP',
        fraction: 2,
        symbol: {
            normal: 'LS'
        },
        unit: 'Pound'
    },
    'SZL': { // Lilangeni
        code: 'SZL',
        fraction: 2,
        symbol: {
            normal: 'E'
        },
        unit: 'Lilangeni'
    },
    'THB': { // Baht
        code: 'THB',
        fraction: 2,
        symbol: {
            normal: '฿'
        },
        unit: 'Baht'
    },
    'TJS': { // Somoni
        code: 'TJS',
        fraction: 2,
        symbol: {
            normal: 'SM'
        },
        unit: 'Somoni'
    },
    'TMT': { // Turkmenistan New Manat
        code: 'TMT',
        fraction: 2,
        symbol: {
            normal: 'm'
        },
        unit: 'Manat'
    },
    'TND': { // Tunisian Dinar
        code: 'TND',
        fraction: 3,
        symbol: {
            normal: 'DT'
        },
        unit: 'Dinar'
    },
    'TOP': { // Pa’anga
        code: 'TOP',
        fraction: 2,
        symbol: {
            normal: 'T$'
        },
        unit: 'Paanga'
    },
    'TRY': { // Turkish Lira
        code: 'TRY',
        fraction: 2,
        symbol: {
            normal: '₺'
        },
        unit: 'Lira'
    },
    'TTD': { // Trinidad and Tobago Dollar
        code: 'TTD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'TWD': { // New Taiwan Dollar
        code: 'TWD',
        fraction: 2,
        symbol: {
            normal: 'NT$'
        },
        unit: 'Dollar'
    },
    'TZS': { // Tanzanian Shilling
        code: 'TZS',
        fraction: 2,
        symbol: {
            normal: '/='
        },
        unit: 'Shilling'
    },
    'UAH': { // Hryvnia
        code: 'UAH',
        fraction: 2,
        symbol: {
            normal: '₴'
        },
        unit: 'Hryvnia'
    },
    'UGX': { // Uganda Shilling
        code: 'UGX',
        fraction: 0,
        symbol: {
            normal: '/='
        },
        unit: 'Shilling'
    },
    'USD': { // US Dollar
        code: 'USD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'UYU': { // Peso Uruguayo
        code: 'UYU',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Peso'
    },
    'UZS': { // Uzbekistan Sum
        code: 'UZS',
        fraction: 2,
        unit: 'Sum'
    },
    'VED': { // Bolívar Soberano
        code: 'VED',
        fraction: 2,
        symbol: {
            normal: 'Bs.D'
        },
        unit: 'Bolivar'
    },
    'VES': { // Bolívar Soberano
        code: 'VES',
        fraction: 2,
        symbol: {
            normal: 'Bs.S'
        },
        unit: 'Bolivar'
    },
    'VND': { // Dong
        code: 'VND',
        fraction: 0,
        symbol: {
            normal: '₫'
        },
        unit: 'Dong'
    },
    'VUV': { // Vatu
        code: 'VUV',
        fraction: 0,
        symbol: {
            normal: 'VT'
        },
        unit: 'Vatu'
    },
    'WST': { // Tala
        code: 'WST',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Tala'
    },
    'XAF': { // CFA Franc BEAC
        code: 'XAF',
        fraction: 0,
        symbol: {
            normal: 'F.CFA'
        },
        unit: 'Franc'
    },
    'XCD': { // East Caribbean Dollar
        code: 'XCD',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    },
    'XOF': { // CFA Franc BCEAO
        code: 'XOF',
        fraction: 0,
        symbol: {
            normal: 'F.CFA'
        },
        unit: 'Franc'
    },
    'XPF': { // CFP Franc
        code: 'XPF',
        fraction: 0,
        symbol: {
            normal: 'F'
        },
        unit: 'Franc'
    },
    'XSU': { // Sucre
        code: 'XSU',
        symbol: {
            normal: 'S/.'
        },
        unit: 'Sucre'
    },
    'YER': { // Yemeni Rial
        code: 'YER',
        fraction: 2,
        symbol: {
            normal: 'YRl',
            plural: 'YRls'
        },
        unit: 'Rial'
    },
    'ZAR': { // Rand
        code: 'ZAR',
        fraction: 2,
        symbol: {
            normal: 'R'
        },
        unit: 'Rand'
    },
    'ZMW': { // Zambian Kwacha
        code: 'ZMW',
        fraction: 2,
        symbol: {
            normal: 'K'
        },
        unit: 'Kwacha'
    },
    'ZWG': { // Zimbabwe Gold
        code: 'ZWG',
        fraction: 2,
        symbol: {
            normal: 'ZiG'
        },
        unit: 'ZiG'
    },
    'ZWL': { // Zimbabwe Dollar
        code: 'ZWL',
        fraction: 2,
        symbol: {
            normal: '$'
        },
        unit: 'Dollar'
    }
};
