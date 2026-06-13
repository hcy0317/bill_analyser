// 中文导读：注册时可选的后端拥有默认包内容。
// 维护重点：保持通用个人/家庭账单覆盖，不写入具体银行、证券、基金或用户私有别名。

use crate::auth_registration::{
    DefaultAccount, DefaultAccountRule, DefaultCategory, DefaultCategoryRule, DefaultSubCategory,
};

const EXPENSE: i64 = 3;
const INCOME: i64 = 2;
const TRANSFER: i64 = 4;
const INVESTMENT: i64 = 5;

const ACCOUNT_SINGLE: &str = "1";

const ACCOUNT_CASH: i64 = 1;
const ACCOUNT_BANK_CARD: i64 = 2;
const ACCOUNT_CREDIT_CARD: i64 = 3;
const ACCOUNT_VIRTUAL: i64 = 4;
const ACCOUNT_DEBT: i64 = 5;
const ACCOUNT_RECEIVABLE: i64 = 6;
const ACCOUNT_INVESTMENT: i64 = 7;

const FOOD_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "餐馆",
        icon: "2",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "外卖",
        icon: "2",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "食材水果",
        icon: "3",
        color: "ff8a00",
    },
    DefaultSubCategory {
        name: "饮料甜点",
        icon: "4",
        color: "ff9f0a",
    },
    DefaultSubCategory {
        name: "烟酒茶叶",
        icon: "5",
        color: "c17f59",
    },
];

const SHOPPING_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "网购",
        icon: "300",
        color: "af52de",
    },
    DefaultSubCategory {
        name: "服饰鞋包",
        icon: "301",
        color: "bf5af2",
    },
    DefaultSubCategory {
        name: "日用百货",
        icon: "302",
        color: "8e8e93",
    },
    DefaultSubCategory {
        name: "数码电器",
        icon: "303",
        color: "5e5ce6",
    },
    DefaultSubCategory {
        name: "快递物流",
        icon: "304",
        color: "ff9500",
    },
];

const HOUSING_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "房租房贷",
        icon: "400",
        color: "34c759",
    },
    DefaultSubCategory {
        name: "水电燃气",
        icon: "401",
        color: "30b0c7",
    },
    DefaultSubCategory {
        name: "物业管理",
        icon: "402",
        color: "64d2ff",
    },
    DefaultSubCategory {
        name: "家居家装",
        icon: "403",
        color: "a2845e",
    },
    DefaultSubCategory {
        name: "维修保养",
        icon: "404",
        color: "8e8e93",
    },
];

const TRANSPORT_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "公共交通",
        icon: "500",
        color: "007aff",
    },
    DefaultSubCategory {
        name: "打车",
        icon: "501",
        color: "0a84ff",
    },
    DefaultSubCategory {
        name: "共享出行",
        icon: "502",
        color: "30b0c7",
    },
    DefaultSubCategory {
        name: "长途出行",
        icon: "503",
        color: "5e5ce6",
    },
    DefaultSubCategory {
        name: "停车加油",
        icon: "504",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "通讯服务",
        icon: "505",
        color: "32d74b",
    },
];

const DAILY_SERVICE_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "美容美发",
        icon: "600",
        color: "ff375f",
    },
    DefaultSubCategory {
        name: "洗护维修",
        icon: "601",
        color: "64d2ff",
    },
    DefaultSubCategory {
        name: "家政服务",
        icon: "602",
        color: "30d158",
    },
    DefaultSubCategory {
        name: "政务办事",
        icon: "603",
        color: "8e8e93",
    },
];

const HEALTH_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "药品就医",
        icon: "700",
        color: "ff453a",
    },
    DefaultSubCategory {
        name: "体检牙科",
        icon: "701",
        color: "ff6961",
    },
    DefaultSubCategory {
        name: "医疗器械",
        icon: "702",
        color: "ff9f0a",
    },
    DefaultSubCategory {
        name: "运动健康",
        icon: "703",
        color: "30d158",
    },
];

const EDUCATION_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "学费培训",
        icon: "800",
        color: "5856d6",
    },
    DefaultSubCategory {
        name: "在线学习",
        icon: "801",
        color: "5e5ce6",
    },
    DefaultSubCategory {
        name: "书籍文具",
        icon: "802",
        color: "bf5af2",
    },
    DefaultSubCategory {
        name: "考试认证",
        icon: "803",
        color: "ff9f0a",
    },
];

const ENTERTAINMENT_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "影视音乐",
        icon: "900",
        color: "ff2d55",
    },
    DefaultSubCategory {
        name: "游戏",
        icon: "901",
        color: "af52de",
    },
    DefaultSubCategory {
        name: "旅游度假",
        icon: "902",
        color: "0a84ff",
    },
    DefaultSubCategory {
        name: "兴趣爱好",
        icon: "903",
        color: "ff9f0a",
    },
    DefaultSubCategory {
        name: "聚会娱乐",
        icon: "904",
        color: "ff6b22",
    },
];

const SOCIAL_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "红包转账",
        icon: "910",
        color: "ff453a",
    },
    DefaultSubCategory {
        name: "礼品随礼",
        icon: "911",
        color: "ff9f0a",
    },
    DefaultSubCategory {
        name: "聚会宴请",
        icon: "912",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "捐赠慈善",
        icon: "913",
        color: "30d158",
    },
];

const FINANCE_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "保险",
        icon: "980",
        color: "30d158",
    },
    DefaultSubCategory {
        name: "手续费",
        icon: "981",
        color: "8e8e93",
    },
    DefaultSubCategory {
        name: "税费社保公积金",
        icon: "982",
        color: "ff9f0a",
    },
    DefaultSubCategory {
        name: "罚款赔偿",
        icon: "983",
        color: "ff453a",
    },
];

const BORROWING_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "借出",
        icon: "990",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "还款",
        icon: "991",
        color: "ff9f0a",
    },
    DefaultSubCategory {
        name: "收款",
        icon: "992",
        color: "30d158",
    },
    DefaultSubCategory {
        name: "借入",
        icon: "993",
        color: "ff453a",
    },
];

const OTHER_EXPENSE_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "无法归类",
        icon: "1010",
        color: "8e8e93",
    },
    DefaultSubCategory {
        name: "其他支出",
        icon: "1011",
        color: "636366",
    },
];

const JOB_INCOME_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "工资",
        icon: "2010",
        color: "30d158",
    },
    DefaultSubCategory {
        name: "奖金",
        icon: "2011",
        color: "34c759",
    },
    DefaultSubCategory {
        name: "兼职",
        icon: "2012",
        color: "32d74b",
    },
    DefaultSubCategory {
        name: "报销",
        icon: "2013",
        color: "64d2ff",
    },
];

const INVESTMENT_INCOME_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "利息",
        icon: "2020",
        color: "30d158",
    },
    DefaultSubCategory {
        name: "理财收益",
        icon: "2021",
        color: "34c759",
    },
    DefaultSubCategory {
        name: "分红",
        icon: "2022",
        color: "32d74b",
    },
    DefaultSubCategory {
        name: "资产变现",
        icon: "2023",
        color: "0a84ff",
    },
];

const OTHER_INCOME_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "退款",
        icon: "2030",
        color: "4cd964",
    },
    DefaultSubCategory {
        name: "红包礼金",
        icon: "2031",
        color: "ff453a",
    },
    DefaultSubCategory {
        name: "中奖意外",
        icon: "2032",
        color: "ffcc00",
    },
    DefaultSubCategory {
        name: "其他收入",
        icon: "2033",
        color: "8e8e93",
    },
];

const TRANSFER_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "银行卡互转",
        icon: "4000",
        color: "2196f3",
    },
    DefaultSubCategory {
        name: "信用卡还款",
        icon: "4001",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "支付账户互转",
        icon: "4002",
        color: "30b0c7",
    },
    DefaultSubCategory {
        name: "投资账户转入转出",
        icon: "4003",
        color: "34c759",
    },
];

const INVESTMENT_SUBS: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "金融理财",
        icon: "5000",
        color: "34c759",
    },
    DefaultSubCategory {
        name: "证券基金",
        icon: "5001",
        color: "30d158",
    },
    DefaultSubCategory {
        name: "黄金贵金属",
        icon: "5002",
        color: "ffcc00",
    },
    DefaultSubCategory {
        name: "其他投资",
        icon: "5003",
        color: "8e8e93",
    },
];

pub(crate) const STANDARD_DAILY_V1_CATEGORIES: &[DefaultCategory] = &[
    DefaultCategory {
        type_code: EXPENSE,
        name: "餐饮食品",
        icon: "1",
        color: "ff6b22",
        priority: 100,
        sub_categories: FOOD_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "购物衣物",
        icon: "300",
        color: "af52de",
        priority: 200,
        sub_categories: SHOPPING_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "居住水电",
        icon: "400",
        color: "34c759",
        priority: 300,
        sub_categories: HOUSING_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "交通通信",
        icon: "500",
        color: "007aff",
        priority: 400,
        sub_categories: TRANSPORT_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "生活服务",
        icon: "600",
        color: "64d2ff",
        priority: 500,
        sub_categories: DAILY_SERVICE_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "医疗健康",
        icon: "700",
        color: "ff453a",
        priority: 600,
        sub_categories: HEALTH_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "教育学习",
        icon: "800",
        color: "5856d6",
        priority: 700,
        sub_categories: EDUCATION_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "文化娱乐",
        icon: "900",
        color: "ff2d55",
        priority: 800,
        sub_categories: ENTERTAINMENT_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "人情社交",
        icon: "910",
        color: "ff453a",
        priority: 900,
        sub_categories: SOCIAL_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "金融保险税费",
        icon: "980",
        color: "30d158",
        priority: 1000,
        sub_categories: FINANCE_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "借贷往来",
        icon: "990",
        color: "ff9500",
        priority: 1100,
        sub_categories: BORROWING_SUBS,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "其他支出",
        icon: "1000",
        color: "8e8e93",
        priority: 1200,
        sub_categories: OTHER_EXPENSE_SUBS,
    },
    DefaultCategory {
        type_code: INCOME,
        name: "职业收入",
        icon: "2000",
        color: "30d158",
        priority: 2000,
        sub_categories: JOB_INCOME_SUBS,
    },
    DefaultCategory {
        type_code: INCOME,
        name: "投资收入",
        icon: "2020",
        color: "34c759",
        priority: 2100,
        sub_categories: INVESTMENT_INCOME_SUBS,
    },
    DefaultCategory {
        type_code: INCOME,
        name: "其他收入",
        icon: "2030",
        color: "4cd964",
        priority: 2200,
        sub_categories: OTHER_INCOME_SUBS,
    },
    DefaultCategory {
        type_code: TRANSFER,
        name: "账户互转",
        icon: "4000",
        color: "2196f3",
        priority: 3000,
        sub_categories: TRANSFER_SUBS,
    },
    DefaultCategory {
        type_code: INVESTMENT,
        name: "投资理财",
        icon: "5000",
        color: "34c759",
        priority: 4000,
        sub_categories: INVESTMENT_SUBS,
    },
];

pub(crate) const STANDARD_DAILY_V1_ACCOUNTS: &[DefaultAccount] = &[
    DefaultAccount {
        name: "现金",
        category: ACCOUNT_CASH,
        account_type: ACCOUNT_SINGLE,
        icon: "1",
        color: "000000",
        display_order: 0,
    },
    DefaultAccount {
        name: "银行卡",
        category: ACCOUNT_BANK_CARD,
        account_type: ACCOUNT_SINGLE,
        icon: "100",
        color: "0a84ff",
        display_order: 1,
    },
    DefaultAccount {
        name: "信用卡",
        category: ACCOUNT_CREDIT_CARD,
        account_type: ACCOUNT_SINGLE,
        icon: "100",
        color: "ff9500",
        display_order: 2,
    },
    DefaultAccount {
        name: "微信",
        category: ACCOUNT_VIRTUAL,
        account_type: ACCOUNT_SINGLE,
        icon: "500",
        color: "30d158",
        display_order: 3,
    },
    DefaultAccount {
        name: "支付宝",
        category: ACCOUNT_VIRTUAL,
        account_type: ACCOUNT_SINGLE,
        icon: "500",
        color: "0a84ff",
        display_order: 4,
    },
    DefaultAccount {
        name: "花呗/消费信贷",
        category: ACCOUNT_DEBT,
        account_type: ACCOUNT_SINGLE,
        icon: "600",
        color: "ff453a",
        display_order: 5,
    },
    DefaultAccount {
        name: "理财/投资账户",
        category: ACCOUNT_INVESTMENT,
        account_type: ACCOUNT_SINGLE,
        icon: "800",
        color: "34c759",
        display_order: 6,
    },
    DefaultAccount {
        name: "应收款",
        category: ACCOUNT_RECEIVABLE,
        account_type: ACCOUNT_SINGLE,
        icon: "700",
        color: "30d158",
        display_order: 7,
    },
    DefaultAccount {
        name: "应付款/借款",
        category: ACCOUNT_DEBT,
        account_type: ACCOUNT_SINGLE,
        icon: "600",
        color: "ff9f0a",
        display_order: 8,
    },
];

pub(crate) const STANDARD_DAILY_V1_CATEGORY_RULES: &[DefaultCategoryRule] = &[
    DefaultCategoryRule {
        name: "default:standard_daily_v1:餐饮食品/外卖",
        main_category: "餐饮食品",
        sub_category: "外卖",
        rule_expression: "OR={美团外卖,饿了么,外卖,外卖订单}+NOT={退款,返现}",
        priority: 100,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:餐饮食品/餐馆",
        main_category: "餐饮食品",
        sub_category: "餐馆",
        rule_expression: "OR={餐饮,餐厅,饭店,小吃,烧烤,火锅,咖啡,奶茶,茶饮,肯德基,麦当劳,必胜客,瑞幸,星巴克}+NOT={退款}",
        priority: 110,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:餐饮食品/食材水果",
        main_category: "餐饮食品",
        sub_category: "食材水果",
        rule_expression: "OR={超市,菜市场,生鲜,水果,蔬菜,买菜,便利店,盒马,叮咚买菜,朴朴,钱大妈}+NOT={退款}",
        priority: 120,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:购物衣物/网购",
        main_category: "购物衣物",
        sub_category: "网购",
        rule_expression: "OR={淘宝,天猫,京东,拼多多,抖音电商,快手小店,小红书,唯品会,苏宁}+NOT={退款,退货}",
        priority: 200,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:购物衣物/快递物流",
        main_category: "购物衣物",
        sub_category: "快递物流",
        rule_expression: "OR={快递,顺丰,中通,圆通,韵达,邮政,丰巢,菜鸟驿站}",
        priority: 210,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:居住水电/房租房贷",
        main_category: "居住水电",
        sub_category: "房租房贷",
        rule_expression: "OR={房租,租金,房贷,按揭,住房贷款}",
        priority: 300,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:居住水电/水电燃气",
        main_category: "居住水电",
        sub_category: "水电燃气",
        rule_expression: "OR={水费,电费,燃气,煤气,供暖,宽带缴费}",
        priority: 310,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:居住水电/物业管理",
        main_category: "居住水电",
        sub_category: "物业管理",
        rule_expression: "OR={物业,物业费,停车管理费}",
        priority: 320,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:交通通信/公共交通",
        main_category: "交通通信",
        sub_category: "公共交通",
        rule_expression: "OR={地铁,公交,公交车,轨道交通,交通卡}",
        priority: 400,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:交通通信/打车",
        main_category: "交通通信",
        sub_category: "打车",
        rule_expression: "OR={滴滴,高德打车,T3出行,网约车,出租车}",
        priority: 410,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:交通通信/长途出行",
        main_category: "交通通信",
        sub_category: "长途出行",
        rule_expression: "OR={高铁,火车,机票,航空,机场,客运,船票}",
        priority: 420,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:交通通信/停车加油",
        main_category: "交通通信",
        sub_category: "停车加油",
        rule_expression: "OR={停车,停车费,加油,充电桩,高速,ETC}",
        priority: 430,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:交通通信/通讯服务",
        main_category: "交通通信",
        sub_category: "通讯服务",
        rule_expression: "OR={话费,流量,手机费,宽带费,通讯费}+NOT={退款}",
        priority: 440,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:生活服务/美容美发",
        main_category: "生活服务",
        sub_category: "美容美发",
        rule_expression: "OR={美容,美发,理发,美甲,护肤}",
        priority: 500,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:生活服务/洗护维修",
        main_category: "生活服务",
        sub_category: "洗护维修",
        rule_expression: "OR={洗衣,干洗,维修,开锁,修理,保养}",
        priority: 510,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:医疗健康/药品就医",
        main_category: "医疗健康",
        sub_category: "药品就医",
        rule_expression: "OR={医院,门诊,药店,医保,挂号,药品,处方}",
        priority: 600,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:医疗健康/体检牙科",
        main_category: "医疗健康",
        sub_category: "体检牙科",
        rule_expression: "OR={体检,牙科,口腔,洗牙}",
        priority: 610,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:教育学习/学费培训",
        main_category: "教育学习",
        sub_category: "学费培训",
        rule_expression: "OR={学费,培训,课程,补习,托管,幼儿园}",
        priority: 700,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:教育学习/书籍文具",
        main_category: "教育学习",
        sub_category: "书籍文具",
        rule_expression: "OR={书店,图书,教材,文具,打印,复印}",
        priority: 710,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:文化娱乐/影视音乐",
        main_category: "文化娱乐",
        sub_category: "影视音乐",
        rule_expression: "OR={电影,影院,视频会员,音乐会员,演出,票务}",
        priority: 800,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:文化娱乐/游戏",
        main_category: "文化娱乐",
        sub_category: "游戏",
        rule_expression: "OR={游戏,手游,主机游戏,点券,皮肤}",
        priority: 810,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:文化娱乐/旅游度假",
        main_category: "文化娱乐",
        sub_category: "旅游度假",
        rule_expression: "OR={旅游,酒店,民宿,景区,门票,度假}",
        priority: 820,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:人情社交/红包转账",
        main_category: "人情社交",
        sub_category: "红包转账",
        rule_expression: "OR={红包,转账红包,收红包,发红包}",
        priority: 900,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:人情社交/礼品随礼",
        main_category: "人情社交",
        sub_category: "礼品随礼",
        rule_expression: "OR={礼品,礼物,随礼,礼金,份子钱,结婚红包}",
        priority: 910,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:金融保险税费/保险",
        main_category: "金融保险税费",
        sub_category: "保险",
        rule_expression: "OR={保险,保费,车险,医保缴费}",
        priority: 1000,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:金融保险税费/手续费",
        main_category: "金融保险税费",
        sub_category: "手续费",
        rule_expression: "OR={手续费,服务费,年费,管理费}",
        priority: 1010,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:金融保险税费/税费社保公积金",
        main_category: "金融保险税费",
        sub_category: "税费社保公积金",
        rule_expression: "OR={个税,税费,社保,公积金}",
        priority: 1020,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:借贷往来/还款",
        main_category: "借贷往来",
        sub_category: "还款",
        rule_expression: "OR={借款还款,还借款,还钱,归还借款}+NOT={信用卡}",
        priority: 1100,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:职业收入/工资",
        main_category: "职业收入",
        sub_category: "工资",
        rule_expression: "REGEX={(工资|薪资|薪金|代发工资)}",
        priority: 2000,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:职业收入/奖金兼职报销",
        main_category: "职业收入",
        sub_category: "奖金",
        rule_expression: "OR={奖金,年终奖,绩效,兼职,劳务费,报销,补贴}+NOT={退款}",
        priority: 2010,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:投资收入/收益",
        main_category: "投资收入",
        sub_category: "理财收益",
        rule_expression: "OR={利息,分红,理财收益,收益到账}",
        priority: 2100,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:其他收入/退款",
        main_category: "其他收入",
        sub_category: "退款",
        rule_expression: "OR={退款,退货,冲正,返现,退票}",
        priority: 2200,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:账户互转/信用卡还款",
        main_category: "账户互转",
        sub_category: "信用卡还款",
        rule_expression: "OR={信用卡还款,还信用卡,自动还款}",
        priority: 3000,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:账户互转/银行卡互转",
        main_category: "账户互转",
        sub_category: "银行卡互转",
        rule_expression: "OR={跨行转账,银行卡转账,快捷转账}+NOT={红包,礼金,工资,退款}",
        priority: 3010,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:账户互转/支付账户互转",
        main_category: "账户互转",
        sub_category: "支付账户互转",
        rule_expression: "OR={余额提现,充值,提现到银行卡,微信零钱提现,支付宝提现}",
        priority: 3020,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:投资理财/金融理财",
        main_category: "投资理财",
        sub_category: "金融理财",
        rule_expression: "OR={理财,基金,证券,股票,债券}+NOT={退款}",
        priority: 4000,
    },
    DefaultCategoryRule {
        name: "default:standard_daily_v1:投资理财/黄金贵金属",
        main_category: "投资理财",
        sub_category: "黄金贵金属",
        rule_expression: "OR={黄金,贵金属}+NOT={礼品,首饰}",
        priority: 4010,
    },
];

pub(crate) const STANDARD_DAILY_V1_ACCOUNT_RULES: &[DefaultAccountRule] = &[
    DefaultAccountRule {
        name: "default:standard_daily_v1:account:cash",
        account_name: "现金",
        rule_expression: "OR={现金,现钞}",
        priority: 100,
        source_key: "standard_daily_v1:account:cash",
    },
    DefaultAccountRule {
        name: "default:standard_daily_v1:account:bank-card",
        account_name: "银行卡",
        rule_expression: "OR={银行卡,储蓄卡,借记卡,银联,快捷支付,网银,手机银行}+NOT={信用卡,花呗}",
        priority: 110,
        source_key: "standard_daily_v1:account:bank-card",
    },
    DefaultAccountRule {
        name: "default:standard_daily_v1:account:credit-card",
        account_name: "信用卡",
        rule_expression: "OR={信用卡,贷记卡}",
        priority: 120,
        source_key: "standard_daily_v1:account:credit-card",
    },
    DefaultAccountRule {
        name: "default:standard_daily_v1:account:wechat",
        account_name: "微信",
        rule_expression: "OR={微信支付,财付通,微信零钱,零钱通,微信红包}",
        priority: 130,
        source_key: "standard_daily_v1:account:wechat",
    },
    DefaultAccountRule {
        name: "default:standard_daily_v1:account:alipay",
        account_name: "支付宝",
        rule_expression: "OR={支付宝,余额宝,支付宝余额}+NOT={花呗}",
        priority: 140,
        source_key: "standard_daily_v1:account:alipay",
    },
    DefaultAccountRule {
        name: "default:standard_daily_v1:account:consumer-credit",
        account_name: "花呗/消费信贷",
        rule_expression: "OR={花呗,白条,消费贷,分期}",
        priority: 150,
        source_key: "standard_daily_v1:account:consumer-credit",
    },
    DefaultAccountRule {
        name: "default:standard_daily_v1:account:investment",
        account_name: "理财/投资账户",
        rule_expression: "OR={理财,基金,证券,股票,债券,黄金,贵金属}",
        priority: 160,
        source_key: "standard_daily_v1:account:investment",
    },
    DefaultAccountRule {
        name: "default:standard_daily_v1:account:receivable",
        account_name: "应收款",
        rule_expression: "OR={应收,借出,待收,收款}",
        priority: 170,
        source_key: "standard_daily_v1:account:receivable",
    },
    DefaultAccountRule {
        name: "default:standard_daily_v1:account:payable",
        account_name: "应付款/借款",
        rule_expression: "OR={应付,借入,借款,待还}",
        priority: 180,
        source_key: "standard_daily_v1:account:payable",
    },
];

#[cfg(test)]
mod tests {
    use bill_analyser_core::category_rules::{compile_rule_expression, match_rule_expression};

    use super::{STANDARD_DAILY_V1_ACCOUNT_RULES, STANDARD_DAILY_V1_CATEGORY_RULES};

    #[test]
    fn standard_daily_v1_category_rule_expressions_compile_non_empty() {
        for rule in STANDARD_DAILY_V1_CATEGORY_RULES {
            let compiled = compile_rule_expression(rule.rule_expression, false);

            assert!(
                !compiled.is_empty,
                "{} should compile: {}",
                rule.name, rule.rule_expression
            );
        }
    }

    #[test]
    fn standard_daily_v1_account_rule_expressions_compile_non_empty() {
        for rule in STANDARD_DAILY_V1_ACCOUNT_RULES {
            let compiled = compile_rule_expression(rule.rule_expression, false);

            assert!(
                !compiled.is_empty,
                "{} should compile: {}",
                rule.name, rule.rule_expression
            );
        }
    }

    #[test]
    fn standard_daily_v1_representative_category_rules_match_expected_text() {
        let cases = [
            (
                "美团外卖订单",
                "default:standard_daily_v1:餐饮食品/外卖",
                true,
            ),
            (
                "饿了么午餐",
                "default:standard_daily_v1:餐饮食品/外卖",
                true,
            ),
            ("滴滴出行", "default:standard_daily_v1:交通通信/打车", true),
            ("高德打车", "default:standard_daily_v1:交通通信/打车", true),
            ("本月工资", "default:standard_daily_v1:职业收入/工资", true),
            ("薪资入账", "default:standard_daily_v1:职业收入/工资", true),
            (
                "信用卡还款",
                "default:standard_daily_v1:账户互转/信用卡还款",
                true,
            ),
            (
                "黄金礼品",
                "default:standard_daily_v1:投资理财/黄金贵金属",
                false,
            ),
            (
                "借款还款",
                "default:standard_daily_v1:账户互转/信用卡还款",
                false,
            ),
            ("退款到账", "default:standard_daily_v1:职业收入/工资", false),
        ];

        for (text, rule_name, expected) in cases {
            let rule = STANDARD_DAILY_V1_CATEGORY_RULES
                .iter()
                .find(|item| item.name == rule_name)
                .expect("rule exists");

            assert_eq!(
                match_rule_expression(text, rule.rule_expression, false),
                expected,
                "{text} against {}",
                rule.rule_expression
            );
        }
    }

    #[test]
    fn standard_daily_v1_representative_account_rules_match_expected_text() {
        let cases = [
            (
                "微信支付-餐饮",
                "default:standard_daily_v1:account:wechat",
                true,
            ),
            (
                "财付通付款",
                "default:standard_daily_v1:account:wechat",
                true,
            ),
            (
                "支付宝消费",
                "default:standard_daily_v1:account:alipay",
                true,
            ),
            (
                "余额宝转入",
                "default:standard_daily_v1:account:alipay",
                true,
            ),
            (
                "花呗分期",
                "default:standard_daily_v1:account:consumer-credit",
                true,
            ),
            (
                "信用卡消费",
                "default:standard_daily_v1:account:credit-card",
                true,
            ),
            (
                "银行卡快捷支付",
                "default:standard_daily_v1:account:bank-card",
                true,
            ),
            (
                "花呗快捷支付",
                "default:standard_daily_v1:account:bank-card",
                false,
            ),
        ];

        for (text, rule_name, expected) in cases {
            let rule = STANDARD_DAILY_V1_ACCOUNT_RULES
                .iter()
                .find(|item| item.name == rule_name)
                .expect("rule exists");

            assert_eq!(
                match_rule_expression(text, rule.rule_expression, false),
                expected,
                "{text} against {}",
                rule.rule_expression
            );
        }
    }
}
