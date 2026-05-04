import type { PresetCategory } from '@/core/category.ts';

export const DEFAULT_EXPENSE_CATEGORIES: PresetCategory[] = [
    {
        name: 'Food & Drink',
        categoryIconId: '1',
        color: 'ff6b22',
        subCategories: [
            { name: 'Breakfast', categoryIconId: '2', color: 'ff6b22' },
            { name: 'Lunch', categoryIconId: '2', color: 'ff6b22' },
            { name: 'Dinner', categoryIconId: '2', color: 'ff6b22' },
            { name: 'Coffee & Tea', categoryIconId: '30', color: 'ff6b22' },
            { name: 'Snacks & Drinks', categoryIconId: '70', color: 'ff6b22' },
            { name: 'Food Delivery', categoryIconId: '2', color: 'ff6b22' },
            { name: 'Dining Out', categoryIconId: '540', color: 'ff6b22' }
        ]
    },
    {
        name: 'Groceries & Daily Supplies',
        categoryIconId: '210',
        color: '4caf50',
        subCategories: [
            { name: 'Supermarkets & Convenience', categoryIconId: '210', color: '4caf50' },
            { name: 'Fresh Produce', categoryIconId: '70', color: '4caf50' },
            { name: 'Grain & Seasoning', categoryIconId: '2', color: '4caf50' },
            { name: 'Daily Supplies', categoryIconId: '210', color: '4caf50' },
            { name: 'Cleaning & Paper', categoryIconId: '210', color: '4caf50' },
            { name: 'Pets', categoryIconId: '580', color: '4caf50' }
        ]
    },
    {
        name: 'Housing & Household',
        categoryIconId: '200',
        color: '607d8b',
        subCategories: [
            { name: 'Rent', categoryIconId: '290', color: '607d8b' },
            { name: 'Mortgage', categoryIconId: '290', color: '607d8b' },
            { name: 'Property Management', categoryIconId: '200', color: '607d8b' },
            { name: 'Gas Bill', categoryIconId: '270', color: '607d8b' },
            { name: 'Water Bill', categoryIconId: '270', color: '607d8b' },
            { name: 'Electricity Bill', categoryIconId: '270', color: '607d8b' },
            { name: 'Internet & TV', categoryIconId: '430', color: '607d8b' },
            { name: 'Furniture & Appliances', categoryIconId: '230', color: '607d8b' },
            { name: 'Repairs', categoryIconId: '250', color: '607d8b' }
        ]
    },
    {
        name: 'Transportation',
        categoryIconId: '300',
        color: '009688',
        subCategories: [
            { name: 'Public Transit', categoryIconId: '310', color: '009688' },
            { name: 'Ride Hailing', categoryIconId: '320', color: '009688' },
            { name: 'Bike Sharing', categoryIconId: '310', color: '009688' },
            { name: 'Train Tickets', categoryIconId: '370', color: '009688' },
            { name: 'Airline Tickets', categoryIconId: '390', color: '009688' },
            { name: 'Fuel & Charging', categoryIconId: '330', color: '009688' },
            { name: 'Parking & Tolls', categoryIconId: '330', color: '009688' },
            { name: 'Car Maintenance & Insurance', categoryIconId: '330', color: '009688' }
        ]
    },
    {
        name: 'Medical & Healthcare',
        categoryIconId: '800',
        color: 'ff3b30',
        subCategories: [
            { name: 'Diagnosis & Treatment', categoryIconId: '840', color: 'ff3b30' },
            { name: 'Medications', categoryIconId: '860', color: 'ff3b30' },
            { name: 'Health Checkups', categoryIconId: '840', color: 'ff3b30' },
            { name: 'Dental Care', categoryIconId: '840', color: 'ff3b30' },
            { name: 'Glasses', categoryIconId: '890', color: 'ff3b30' },
            { name: 'Sports & Fitness', categoryIconId: '510', color: 'ff3b30' },
            { name: 'Healthcare & Nursing', categoryIconId: '890', color: 'ff3b30' }
        ]
    },
    {
        name: 'Clothing & Beauty',
        categoryIconId: '100',
        color: '673ab7',
        subCategories: [
            { name: 'Clothing', categoryIconId: '110', color: '673ab7' },
            { name: 'Shoes & Bags', categoryIconId: '110', color: '673ab7' },
            { name: 'Skincare & Makeup', categoryIconId: '180', color: '673ab7' },
            { name: 'Hair & Nails', categoryIconId: '190', color: '673ab7' },
            { name: 'Accessories', categoryIconId: '170', color: '673ab7' }
        ]
    },
    {
        name: 'Digital & Office',
        categoryIconId: '230',
        color: '3f51b5',
        subCategories: [
            { name: 'Phones & Computers', categoryIconId: '230', color: '3f51b5' },
            { name: 'Software Tools', categoryIconId: '230', color: '3f51b5' },
            { name: 'Office Supplies', categoryIconId: '210', color: '3f51b5' },
            { name: 'Repairs & Parts', categoryIconId: '250', color: '3f51b5' },
            { name: 'Cloud Services', categoryIconId: '430', color: '3f51b5' }
        ]
    },
    {
        name: 'Education & Growth',
        categoryIconId: '600',
        color: 'cddc39',
        subCategories: [
            { name: 'Tuition', categoryIconId: '600', color: 'cddc39' },
            { name: 'Courses & Training', categoryIconId: '660', color: 'cddc39' },
            { name: 'Books & Materials', categoryIconId: '610', color: 'cddc39' },
            { name: 'Certification & Examination', categoryIconId: '680', color: 'cddc39' },
            { name: 'Children Education', categoryIconId: '660', color: 'cddc39' }
        ]
    },
    {
        name: 'Entertainment & Leisure',
        categoryIconId: '500',
        color: 'ff2d55',
        subCategories: [
            { name: 'Movies & Shows', categoryIconId: '550', color: 'ff2d55' },
            { name: 'Games', categoryIconId: '560', color: 'ff2d55' },
            { name: 'Subscriptions', categoryIconId: '570', color: 'ff2d55' },
            { name: 'Tourist Tickets', categoryIconId: '590', color: 'ff2d55' },
            { name: 'Board Games & Hobbies', categoryIconId: '560', color: 'ff2d55' }
        ]
    },
    {
        name: 'Travel & Accommodation',
        categoryIconId: '590',
        color: '00bcd4',
        subCategories: [
            { name: 'Hotels & Homestays', categoryIconId: '590', color: '00bcd4' },
            { name: 'Attractions', categoryIconId: '590', color: '00bcd4' },
            { name: 'Tours', categoryIconId: '590', color: '00bcd4' },
            { name: 'Visa & Travel Insurance', categoryIconId: '950', color: '00bcd4' },
            { name: 'Travel Supplies', categoryIconId: '110', color: '00bcd4' }
        ]
    },
    {
        name: 'Social & Gifts',
        categoryIconId: '700',
        color: '4cd964',
        subCategories: [
            { name: 'Red Packets & Transfers', categoryIconId: '710', color: '4cd964' },
            { name: 'Gifts', categoryIconId: '710', color: '4cd964' },
            { name: 'Treats', categoryIconId: '540', color: '4cd964' },
            { name: 'Weddings & Ceremonies', categoryIconId: '710', color: '4cd964' },
            { name: 'Donations', categoryIconId: '780', color: '4cd964' }
        ]
    },
    {
        name: 'Finance & Insurance',
        categoryIconId: '900',
        color: 'ff9500',
        subCategories: [
            { name: 'Insurance Expense', categoryIconId: '950', color: 'ff9500' },
            { name: 'Loan Interest', categoryIconId: '970', color: 'ff9500' },
            { name: 'Service Charge', categoryIconId: '930', color: 'ff9500' },
            { name: 'Tax & Fines', categoryIconId: '910', color: 'ff9500' },
            { name: 'Investment Expense', categoryIconId: '810', color: 'ff9500' }
        ]
    },
    {
        name: 'Other Expense',
        categoryIconId: '1000',
        color: '8e8e93',
        subCategories: [
            { name: 'Uncategorized', categoryIconId: '1010', color: '8e8e93' },
            { name: 'Temporary Miscellaneous', categoryIconId: '1010', color: '8e8e93' }
        ]
    }
];

export const DEFAULT_INCOME_CATEGORIES: PresetCategory[] = [
    {
        name: 'Work Income',
        categoryIconId: '2000',
        color: 'ff6b22',
        subCategories: [
            { name: 'Salary Income', categoryIconId: '2010', color: 'ff6b22' },
            { name: 'Bonus Income', categoryIconId: '2020', color: 'ff6b22' },
            { name: 'Allowance', categoryIconId: '231', color: 'ff6b22' },
            { name: 'Reimbursement', categoryIconId: '920', color: 'ff6b22' },
            { name: 'Side Job Income', categoryIconId: '2080', color: 'ff6b22' }
        ]
    },
    {
        name: 'Business Income',
        categoryIconId: '2080',
        color: '4caf50',
        subCategories: [
            { name: 'Sales Income', categoryIconId: '2080', color: '4caf50' },
            { name: 'Service Income', categoryIconId: '2080', color: '4caf50' },
            { name: 'Commission Income', categoryIconId: '2080', color: '4caf50' }
        ]
    },
    {
        name: 'Investment Returns',
        categoryIconId: '2100',
        color: 'ff9500',
        subCategories: [
            { name: 'Interest Income', categoryIconId: '970', color: 'ff9500' },
            { name: 'Dividend Income', categoryIconId: '2100', color: 'ff9500' },
            { name: 'Funds & Stocks', categoryIconId: '810', color: 'ff9500' },
            { name: 'Wealth Management Income', categoryIconId: '830', color: 'ff9500' }
        ]
    },
    {
        name: 'Life Income',
        categoryIconId: '710',
        color: '4cd964',
        subCategories: [
            { name: 'Refunds', categoryIconId: '920', color: '4cd964' },
            { name: 'Reimbursement Received', categoryIconId: '920', color: '4cd964' },
            { name: 'Red Packets & Gifts', categoryIconId: '710', color: '4cd964' },
            { name: 'Second-hand Sales', categoryIconId: '2080', color: '4cd964' },
            { name: 'Rental Income', categoryIconId: '290', color: '4cd964' }
        ]
    },
    {
        name: 'Other Income',
        categoryIconId: '1000',
        color: '8e8e93',
        subCategories: [
            { name: 'Other Income', categoryIconId: '3010', color: '8e8e93' }
        ]
    }
];

export const DEFAULT_TRANSFER_CATEGORIES: PresetCategory[] = [
    {
        name: 'Account Transfer',
        categoryIconId: '4000',
        color: '2196f3',
        subCategories: [
            { name: 'Bank Transfer', categoryIconId: '900', color: '2196f3' },
            { name: 'Balance Top-up', categoryIconId: '981', color: '2196f3' },
            { name: 'Credit Card Repayment', categoryIconId: '980', color: '2196f3' },
            { name: 'Withdrawals', categoryIconId: '981', color: '2196f3' },
            { name: 'Borrowing & Repayment', categoryIconId: '930', color: '2196f3' }
        ]
    }
];

export const DEFAULT_INVESTMENT_CATEGORIES: PresetCategory[] = [
    {
        name: 'Investment Principal',
        categoryIconId: '800',
        color: 'ff9500',
        subCategories: [
            {
                name: 'Fund Investment',
                categoryIconId: '810',
                color: 'ff9500'
            },
            {
                name: 'Stock Investment',
                categoryIconId: '820',
                color: 'ff9500'
            },
            {
                name: 'Wealth Management',
                categoryIconId: '830',
                color: 'ff9500'
            }
        ]
    },
    {
        name: 'Alternative Investments',
        categoryIconId: '840',
        color: '607d8b',
        subCategories: [
            {
                name: 'Gold Investment',
                categoryIconId: '850',
                color: '607d8b'
            },
            {
                name: 'Crypto Investment',
                categoryIconId: '860',
                color: '607d8b'
            },
            {
                name: 'Other Investment',
                categoryIconId: '890',
                color: '607d8b'
            }
        ]
    }
];
