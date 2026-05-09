use rusqlite::{params, Connection, OptionalExtension};

use crate::auth::{create_auth_log, AuthLogDraft};
use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterUserDraft {
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub nickname: String,
    pub language: String,
    pub default_currency: String,
    pub first_day_of_week: i64,
    pub email_verified: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterPresetCategory {
    pub name: String,
    pub type_code: i64,
    pub icon: String,
    pub color: String,
    pub sub_categories: Vec<RegisterPresetSubCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterPresetSubCategory {
    pub name: String,
    pub icon: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterDefaultSeedSummary {
    pub categories_created: i64,
    pub categories_skipped: i64,
    pub rules_created: i64,
    pub rules_skipped: i64,
    pub rules_missing_categories: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterUserResult {
    pub user_id: i64,
    pub preset_categories_saved: bool,
    pub preset_accounts_saved: bool,
    pub cash_account_id: Option<i64>,
    pub default_account_id: Option<i64>,
    pub default_seed: RegisterDefaultSeedSummary,
}

struct DefaultSubCategory {
    name: &'static str,
    icon: &'static str,
    color: &'static str,
}

struct DefaultCategory {
    type_code: i64,
    name: &'static str,
    icon: &'static str,
    color: &'static str,
    priority: i64,
    sub_categories: &'static [DefaultSubCategory],
}

struct DefaultCategoryRule {
    name: &'static str,
    main_category: &'static str,
    sub_category: &'static str,
    rule_expression: &'static str,
    priority: i64,
}

struct DefaultAccountTemplate {
    name: &'static str,
    type_code: i64,
    category: i64,
    currency: &'static str,
    icon: &'static str,
    color: &'static str,
    aliases: &'static [&'static str],
    display_order: i64,
}

const EXPENSE: i64 = 3;
const INCOME: i64 = 2;
const TRANSFER: i64 = 4;

const CAT_DINING: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "早餐",
        icon: "2",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "午餐",
        icon: "2",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "晚餐",
        icon: "2",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "咖啡奶茶",
        icon: "30",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "零食饮料",
        icon: "70",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "外卖",
        icon: "2",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "聚餐",
        icon: "540",
        color: "ff6b22",
    },
];
const CAT_GROCERY: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "超市便利",
        icon: "210",
        color: "4caf50",
    },
    DefaultSubCategory {
        name: "菜场生鲜",
        icon: "70",
        color: "4caf50",
    },
    DefaultSubCategory {
        name: "粮油调味",
        icon: "2",
        color: "4caf50",
    },
    DefaultSubCategory {
        name: "日用品",
        icon: "210",
        color: "4caf50",
    },
    DefaultSubCategory {
        name: "清洁纸品",
        icon: "210",
        color: "4caf50",
    },
    DefaultSubCategory {
        name: "宠物",
        icon: "580",
        color: "4caf50",
    },
];
const CAT_HOUSING: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "房租",
        icon: "290",
        color: "607d8b",
    },
    DefaultSubCategory {
        name: "房贷",
        icon: "290",
        color: "607d8b",
    },
    DefaultSubCategory {
        name: "物业",
        icon: "200",
        color: "607d8b",
    },
    DefaultSubCategory {
        name: "燃气",
        icon: "270",
        color: "607d8b",
    },
    DefaultSubCategory {
        name: "水费",
        icon: "270",
        color: "607d8b",
    },
    DefaultSubCategory {
        name: "电费",
        icon: "270",
        color: "607d8b",
    },
    DefaultSubCategory {
        name: "网络电视",
        icon: "430",
        color: "607d8b",
    },
    DefaultSubCategory {
        name: "家具家电",
        icon: "230",
        color: "607d8b",
    },
    DefaultSubCategory {
        name: "维修",
        icon: "250",
        color: "607d8b",
    },
];
const CAT_TRANSPORT: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "公交地铁",
        icon: "310",
        color: "009688",
    },
    DefaultSubCategory {
        name: "打车网约车",
        icon: "320",
        color: "009688",
    },
    DefaultSubCategory {
        name: "共享单车",
        icon: "310",
        color: "009688",
    },
    DefaultSubCategory {
        name: "高铁火车",
        icon: "370",
        color: "009688",
    },
    DefaultSubCategory {
        name: "机票",
        icon: "390",
        color: "009688",
    },
    DefaultSubCategory {
        name: "加油充电",
        icon: "330",
        color: "009688",
    },
    DefaultSubCategory {
        name: "停车过路",
        icon: "330",
        color: "009688",
    },
    DefaultSubCategory {
        name: "保养保险",
        icon: "330",
        color: "009688",
    },
];
const CAT_HEALTH: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "挂号诊疗",
        icon: "840",
        color: "ff3b30",
    },
    DefaultSubCategory {
        name: "药品",
        icon: "860",
        color: "ff3b30",
    },
    DefaultSubCategory {
        name: "体检",
        icon: "840",
        color: "ff3b30",
    },
    DefaultSubCategory {
        name: "口腔",
        icon: "840",
        color: "ff3b30",
    },
    DefaultSubCategory {
        name: "眼镜",
        icon: "890",
        color: "ff3b30",
    },
    DefaultSubCategory {
        name: "运动健身",
        icon: "510",
        color: "ff3b30",
    },
    DefaultSubCategory {
        name: "保健护理",
        icon: "890",
        color: "ff3b30",
    },
];
const CAT_FASHION: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "服装",
        icon: "110",
        color: "673ab7",
    },
    DefaultSubCategory {
        name: "鞋包",
        icon: "110",
        color: "673ab7",
    },
    DefaultSubCategory {
        name: "护肤彩妆",
        icon: "180",
        color: "673ab7",
    },
    DefaultSubCategory {
        name: "美发美甲",
        icon: "190",
        color: "673ab7",
    },
    DefaultSubCategory {
        name: "配饰",
        icon: "170",
        color: "673ab7",
    },
];
const CAT_DIGITAL: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "手机电脑",
        icon: "230",
        color: "3f51b5",
    },
    DefaultSubCategory {
        name: "软件工具",
        icon: "230",
        color: "3f51b5",
    },
    DefaultSubCategory {
        name: "办公用品",
        icon: "210",
        color: "3f51b5",
    },
    DefaultSubCategory {
        name: "维修配件",
        icon: "250",
        color: "3f51b5",
    },
    DefaultSubCategory {
        name: "云服务",
        icon: "430",
        color: "3f51b5",
    },
];
const CAT_EDUCATION: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "学费",
        icon: "600",
        color: "cddc39",
    },
    DefaultSubCategory {
        name: "课程培训",
        icon: "660",
        color: "cddc39",
    },
    DefaultSubCategory {
        name: "图书资料",
        icon: "610",
        color: "cddc39",
    },
    DefaultSubCategory {
        name: "考试证书",
        icon: "680",
        color: "cddc39",
    },
    DefaultSubCategory {
        name: "儿童教育",
        icon: "660",
        color: "cddc39",
    },
];
const CAT_ENTERTAINMENT: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "电影演出",
        icon: "550",
        color: "ff2d55",
    },
    DefaultSubCategory {
        name: "游戏",
        icon: "560",
        color: "ff2d55",
    },
    DefaultSubCategory {
        name: "会员订阅",
        icon: "570",
        color: "ff2d55",
    },
    DefaultSubCategory {
        name: "旅游门票",
        icon: "590",
        color: "ff2d55",
    },
    DefaultSubCategory {
        name: "棋牌桌游",
        icon: "560",
        color: "ff2d55",
    },
    DefaultSubCategory {
        name: "兴趣爱好",
        icon: "560",
        color: "ff2d55",
    },
];
const CAT_TRAVEL: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "酒店民宿",
        icon: "590",
        color: "00bcd4",
    },
    DefaultSubCategory {
        name: "景点",
        icon: "590",
        color: "00bcd4",
    },
    DefaultSubCategory {
        name: "旅行团",
        icon: "590",
        color: "00bcd4",
    },
    DefaultSubCategory {
        name: "签证保险",
        icon: "950",
        color: "00bcd4",
    },
    DefaultSubCategory {
        name: "行李用品",
        icon: "110",
        color: "00bcd4",
    },
];
const CAT_SOCIAL: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "红包转账",
        icon: "710",
        color: "4cd964",
    },
    DefaultSubCategory {
        name: "礼物",
        icon: "710",
        color: "4cd964",
    },
    DefaultSubCategory {
        name: "请客",
        icon: "540",
        color: "4cd964",
    },
    DefaultSubCategory {
        name: "婚丧喜庆",
        icon: "710",
        color: "4cd964",
    },
    DefaultSubCategory {
        name: "捐赠公益",
        icon: "780",
        color: "4cd964",
    },
];
const CAT_FINANCE: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "保险",
        icon: "950",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "贷款利息",
        icon: "970",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "手续费",
        icon: "930",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "税费罚款",
        icon: "910",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "投资支出",
        icon: "810",
        color: "ff9500",
    },
];
const CAT_OTHER_EXPENSE: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "无法归类",
        icon: "1010",
        color: "8e8e93",
    },
    DefaultSubCategory {
        name: "临时杂项",
        icon: "1010",
        color: "8e8e93",
    },
];
const CAT_WORK_INCOME: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "工资",
        icon: "2010",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "奖金",
        icon: "2020",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "补贴",
        icon: "231",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "报销",
        icon: "920",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "兼职",
        icon: "2080",
        color: "ff6b22",
    },
];
const CAT_BUSINESS_INCOME: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "销售收入",
        icon: "2080",
        color: "4caf50",
    },
    DefaultSubCategory {
        name: "服务收入",
        icon: "2080",
        color: "4caf50",
    },
    DefaultSubCategory {
        name: "佣金",
        icon: "2080",
        color: "4caf50",
    },
];
const CAT_INVESTMENT_INCOME: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "利息",
        icon: "970",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "股息",
        icon: "2100",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "基金股票",
        icon: "810",
        color: "ff9500",
    },
    DefaultSubCategory {
        name: "理财收益",
        icon: "830",
        color: "ff9500",
    },
];
const CAT_LIFE_INCOME: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "退款",
        icon: "920",
        color: "4cd964",
    },
    DefaultSubCategory {
        name: "报销入账",
        icon: "920",
        color: "4cd964",
    },
    DefaultSubCategory {
        name: "红包礼金",
        icon: "710",
        color: "4cd964",
    },
    DefaultSubCategory {
        name: "二手售卖",
        icon: "2080",
        color: "4cd964",
    },
    DefaultSubCategory {
        name: "租金收入",
        icon: "290",
        color: "4cd964",
    },
];
const CAT_OTHER_INCOME: &[DefaultSubCategory] = &[DefaultSubCategory {
    name: "其他",
    icon: "3010",
    color: "8e8e93",
}];
const CAT_TRANSFER: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "银行卡互转",
        icon: "900",
        color: "2196f3",
    },
    DefaultSubCategory {
        name: "余额充值",
        icon: "981",
        color: "2196f3",
    },
    DefaultSubCategory {
        name: "信用卡还款",
        icon: "980",
        color: "2196f3",
    },
    DefaultSubCategory {
        name: "提现",
        icon: "981",
        color: "2196f3",
    },
    DefaultSubCategory {
        name: "借还款",
        icon: "930",
        color: "2196f3",
    },
];

const DEFAULT_DAILY_CATEGORIES: &[DefaultCategory] = &[
    DefaultCategory {
        type_code: EXPENSE,
        name: "餐饮",
        icon: "1",
        color: "ff6b22",
        priority: 100,
        sub_categories: CAT_DINING,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "食品日用",
        icon: "210",
        color: "4caf50",
        priority: 200,
        sub_categories: CAT_GROCERY,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "居住家庭",
        icon: "200",
        color: "607d8b",
        priority: 300,
        sub_categories: CAT_HOUSING,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "交通出行",
        icon: "300",
        color: "009688",
        priority: 400,
        sub_categories: CAT_TRANSPORT,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "医疗健康",
        icon: "800",
        color: "ff3b30",
        priority: 500,
        sub_categories: CAT_HEALTH,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "服饰美妆",
        icon: "100",
        color: "673ab7",
        priority: 600,
        sub_categories: CAT_FASHION,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "数码办公",
        icon: "230",
        color: "3f51b5",
        priority: 700,
        sub_categories: CAT_DIGITAL,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "教育成长",
        icon: "600",
        color: "cddc39",
        priority: 800,
        sub_categories: CAT_EDUCATION,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "娱乐休闲",
        icon: "500",
        color: "ff2d55",
        priority: 900,
        sub_categories: CAT_ENTERTAINMENT,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "旅行住宿",
        icon: "590",
        color: "00bcd4",
        priority: 1000,
        sub_categories: CAT_TRAVEL,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "人情社交",
        icon: "700",
        color: "4cd964",
        priority: 1100,
        sub_categories: CAT_SOCIAL,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "金融保险",
        icon: "900",
        color: "ff9500",
        priority: 1200,
        sub_categories: CAT_FINANCE,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "其他支出",
        icon: "1000",
        color: "8e8e93",
        priority: 1300,
        sub_categories: CAT_OTHER_EXPENSE,
    },
    DefaultCategory {
        type_code: INCOME,
        name: "工作收入",
        icon: "2000",
        color: "ff6b22",
        priority: 2000,
        sub_categories: CAT_WORK_INCOME,
    },
    DefaultCategory {
        type_code: INCOME,
        name: "经营收入",
        icon: "2080",
        color: "4caf50",
        priority: 2100,
        sub_categories: CAT_BUSINESS_INCOME,
    },
    DefaultCategory {
        type_code: INCOME,
        name: "投资收益",
        icon: "2100",
        color: "ff9500",
        priority: 2200,
        sub_categories: CAT_INVESTMENT_INCOME,
    },
    DefaultCategory {
        type_code: INCOME,
        name: "生活收入",
        icon: "710",
        color: "4cd964",
        priority: 2300,
        sub_categories: CAT_LIFE_INCOME,
    },
    DefaultCategory {
        type_code: INCOME,
        name: "其他收入",
        icon: "1000",
        color: "8e8e93",
        priority: 2400,
        sub_categories: CAT_OTHER_INCOME,
    },
    DefaultCategory {
        type_code: TRANSFER,
        name: "账户互转",
        icon: "4000",
        color: "2196f3",
        priority: 3000,
        sub_categories: CAT_TRANSFER,
    },
];

const DEFAULT_DAILY_CATEGORY_RULES: &[DefaultCategoryRule] = &[
    DefaultCategoryRule { name: "default:餐饮/外卖", main_category: "餐饮", sub_category: "外卖", rule_expression: "(OR={美团外卖,饿了么,外卖,饭团}/REGEX={(美团|饿了么).*(外卖|订单)})+NOT={退款,退货,取消,冲正}", priority: 100 },
    DefaultCategoryRule { name: "default:餐饮/咖啡奶茶", main_category: "餐饮", sub_category: "咖啡奶茶", rule_expression: "OR={瑞幸,星巴克,库迪,奈雪,喜茶,蜜雪冰城,霸王茶姬,沪上阿姨,茶百道,咖啡,奶茶}", priority: 110 },
    DefaultCategoryRule { name: "default:餐饮/早餐", main_category: "餐饮", sub_category: "早餐", rule_expression: "REGEX={(早餐|早饭|包子|豆浆|油条|粥店)}", priority: 120 },
    DefaultCategoryRule { name: "default:食品日用/超市便利", main_category: "食品日用", sub_category: "超市便利", rule_expression: "OR={盒马,山姆,沃尔玛,永辉,华润万家,便利蜂,罗森,全家,7-11,超市,便利店}", priority: 200 },
    DefaultCategoryRule { name: "default:食品日用/菜场生鲜", main_category: "食品日用", sub_category: "菜场生鲜", rule_expression: "OR={叮咚买菜,朴朴,每日优鲜,菜市场,生鲜,水果,蔬菜,肉铺}", priority: 210 },
    DefaultCategoryRule { name: "default:居住家庭/水费", main_category: "居住家庭", sub_category: "水费", rule_expression: "OR={自来水,水务}/REGEX={水费}", priority: 300 },
    DefaultCategoryRule { name: "default:居住家庭/电费", main_category: "居住家庭", sub_category: "电费", rule_expression: "OR={国家电网,南方电网,供电}/REGEX={电费}", priority: 310 },
    DefaultCategoryRule { name: "default:居住家庭/燃气", main_category: "居住家庭", sub_category: "燃气", rule_expression: "OR={燃气,天然气,煤气}", priority: 320 },
    DefaultCategoryRule { name: "default:居住家庭/房租", main_category: "居住家庭", sub_category: "房租", rule_expression: "(OR={房租,租金,公寓}/REGEX={(房租|租金).*(支付|转账|缴费)})+NOT={退款,退回}", priority: 330 },
    DefaultCategoryRule { name: "default:交通出行/打车网约车", main_category: "交通出行", sub_category: "打车网约车", rule_expression: "OR={滴滴,高德打车,曹操出行,T3出行,花小猪,出租车}", priority: 400 },
    DefaultCategoryRule { name: "default:交通出行/公交地铁", main_category: "交通出行", sub_category: "公交地铁", rule_expression: "OR={地铁,公交,交通卡,一卡通,乘车码}", priority: 410 },
    DefaultCategoryRule { name: "default:交通出行/高铁火车", main_category: "交通出行", sub_category: "高铁火车", rule_expression: "OR={铁路12306,12306,火车票,高铁票,动车票}", priority: 420 },
    DefaultCategoryRule { name: "default:交通出行/机票", main_category: "交通出行", sub_category: "机票", rule_expression: "OR={航旅纵横,机票,航空,机场,携程,飞猪,同程,去哪儿}", priority: 430 },
    DefaultCategoryRule { name: "default:交通出行/加油充电", main_category: "交通出行", sub_category: "加油充电", rule_expression: "OR={中石化,中石油,壳牌,加油,充电桩,特来电,星星充电}", priority: 440 },
    DefaultCategoryRule { name: "default:交通出行/停车过路", main_category: "交通出行", sub_category: "停车过路", rule_expression: "OR={停车,ETCP,高速费,过路费,路桥费}", priority: 450 },
    DefaultCategoryRule { name: "default:医疗健康/挂号诊疗", main_category: "医疗健康", sub_category: "挂号诊疗", rule_expression: "OR={医院,诊所,挂号,门诊,急诊,体检,口腔}", priority: 500 },
    DefaultCategoryRule { name: "default:医疗健康/药品", main_category: "医疗健康", sub_category: "药品", rule_expression: "OR={药房,药店,阿里健康,京东健康,叮当快药,药品}", priority: 510 },
    DefaultCategoryRule { name: "default:服饰美妆/护肤彩妆", main_category: "服饰美妆", sub_category: "护肤彩妆", rule_expression: "OR={屈臣氏,丝芙兰,护肤,彩妆,化妆品,美妆}", priority: 600 },
    DefaultCategoryRule { name: "default:数码办公/软件工具", main_category: "数码办公", sub_category: "软件工具", rule_expression: "OR={Apple,App Store,微软,Adobe,JetBrains,Notion,飞书,钉钉,软件,订阅,云服务}", priority: 700 },
    DefaultCategoryRule { name: "default:教育成长/课程培训", main_category: "教育成长", sub_category: "课程培训", rule_expression: "OR={学费,培训,课程,得到,知识星球,考试,报名费,教材}", priority: 800 },
    DefaultCategoryRule { name: "default:娱乐休闲/会员订阅", main_category: "娱乐休闲", sub_category: "会员订阅", rule_expression: "OR={腾讯视频,爱奇艺,优酷,网易云音乐,QQ音乐,B站大会员,Netflix,Spotify,会员,订阅}", priority: 900 },
    DefaultCategoryRule { name: "default:娱乐休闲/游戏", main_category: "娱乐休闲", sub_category: "游戏", rule_expression: "OR={Steam,PlayStation,Nintendo,腾讯游戏,网易游戏,米哈游,游戏}", priority: 910 },
    DefaultCategoryRule { name: "default:旅行住宿/酒店民宿", main_category: "旅行住宿", sub_category: "酒店民宿", rule_expression: "OR={酒店,民宿,携程酒店,飞猪酒店,美团酒店,Booking,Airbnb}", priority: 1000 },
    DefaultCategoryRule { name: "default:人情社交/红包转账", main_category: "人情社交", sub_category: "红包转账", rule_expression: "(OR={红包,礼金,份子钱}/REGEX={(微信|支付宝).*(红包|转账)})+NOT={退款,退回,还款}", priority: 1100 },
    DefaultCategoryRule { name: "default:金融保险/保险", main_category: "金融保险", sub_category: "保险", rule_expression: "OR={保险,保费,众安,平安保险,太平洋保险,医保,社保}", priority: 1200 },
    DefaultCategoryRule { name: "default:金融保险/手续费", main_category: "金融保险", sub_category: "手续费", rule_expression: "OR={手续费,服务费,年费,管理费}", priority: 1210 },
    DefaultCategoryRule { name: "default:工作收入/工资", main_category: "工作收入", sub_category: "工资", rule_expression: "REGEX={(工资|薪资|薪金|工资代发|工资发放)}", priority: 2000 },
    DefaultCategoryRule { name: "default:工作收入/奖金", main_category: "工作收入", sub_category: "奖金", rule_expression: "REGEX={(奖金|绩效|年终奖)}", priority: 2010 },
    DefaultCategoryRule { name: "default:生活收入/退款", main_category: "生活收入", sub_category: "退款", rule_expression: "REGEX={(退款|退货|冲正|原路退回)}", priority: 2300 },
    DefaultCategoryRule { name: "default:工作收入/报销", main_category: "工作收入", sub_category: "报销", rule_expression: "REGEX={报销}", priority: 2310 },
    DefaultCategoryRule { name: "default:账户互转/信用卡还款", main_category: "账户互转", sub_category: "信用卡还款", rule_expression: "OR={信用卡还款,还信用卡}", priority: 3000 },
    DefaultCategoryRule { name: "default:账户互转/银行卡互转", main_category: "账户互转", sub_category: "银行卡互转", rule_expression: "REGEX={(银行卡转入|银行卡转出|转账到银行卡|银行卡入账)}", priority: 3010 },
    DefaultCategoryRule { name: "default:账户互转/余额充值", main_category: "账户互转", sub_category: "余额充值", rule_expression: "REGEX={(充值|余额宝转入|零钱通转入)}", priority: 3020 },
    DefaultCategoryRule { name: "default:账户互转/提现", main_category: "账户互转", sub_category: "提现", rule_expression: "REGEX={(提现|余额宝转出|零钱通转出)}", priority: 3030 },
];

const ZH_DEFAULT_ACCOUNTS: &[DefaultAccountTemplate] = &[
    DefaultAccountTemplate {
        name: "现金",
        type_code: 1,
        category: 1,
        currency: "CNY",
        icon: "1",
        color: "4caf50",
        aliases: &["现金", "现金钱包", "cash"],
        display_order: 0,
    },
    DefaultAccountTemplate {
        name: "借记卡",
        type_code: 1,
        category: 2,
        currency: "CNY",
        icon: "100",
        color: "2196f3",
        aliases: &["借记卡", "储蓄卡", "银行卡", "debit card"],
        display_order: 1,
    },
    DefaultAccountTemplate {
        name: "信用卡",
        type_code: 1,
        category: 3,
        currency: "CNY",
        icon: "100",
        color: "ff9800",
        aliases: &["信用卡", "贷记卡", "credit card"],
        display_order: 2,
    },
    DefaultAccountTemplate {
        name: "支付宝",
        type_code: 1,
        category: 4,
        currency: "CNY",
        icon: "500",
        color: "1677ff",
        aliases: &["支付宝", "alipay", "花呗", "余额宝"],
        display_order: 3,
    },
    DefaultAccountTemplate {
        name: "微信",
        type_code: 1,
        category: 4,
        currency: "CNY",
        icon: "500",
        color: "07c160",
        aliases: &["微信", "微信支付", "wechat"],
        display_order: 4,
    },
];

const DEFAULT_ACCOUNTS: &[DefaultAccountTemplate] = &[
    DefaultAccountTemplate {
        name: "Cash",
        type_code: 1,
        category: 1,
        currency: "CNY",
        icon: "1",
        color: "4caf50",
        aliases: &["cash", "wallet"],
        display_order: 0,
    },
    DefaultAccountTemplate {
        name: "Debit Card",
        type_code: 1,
        category: 2,
        currency: "CNY",
        icon: "100",
        color: "2196f3",
        aliases: &["debit card", "bank card", "checking"],
        display_order: 1,
    },
    DefaultAccountTemplate {
        name: "Credit Card",
        type_code: 1,
        category: 3,
        currency: "CNY",
        icon: "100",
        color: "ff9800",
        aliases: &["credit card"],
        display_order: 2,
    },
    DefaultAccountTemplate {
        name: "Alipay",
        type_code: 1,
        category: 4,
        currency: "CNY",
        icon: "500",
        color: "1677ff",
        aliases: &["alipay"],
        display_order: 3,
    },
    DefaultAccountTemplate {
        name: "WeChat",
        type_code: 1,
        category: 4,
        currency: "CNY",
        icon: "500",
        color: "07c160",
        aliases: &["wechat", "wechat pay"],
        display_order: 4,
    },
];

pub fn auth_username_exists(connection: &Connection, username: &str) -> DbResult<bool> {
    exists_by_text(
        connection,
        "SELECT 1 FROM users WHERE username = ?1 LIMIT 1",
        username,
    )
}

pub fn auth_email_exists(connection: &Connection, email: &str) -> DbResult<bool> {
    exists_by_text(
        connection,
        "SELECT 1 FROM users WHERE email = ?1 LIMIT 1",
        email,
    )
}

pub fn create_registered_user_with_defaults(
    connection: &Connection,
    draft: &RegisterUserDraft,
    preset_categories: &[RegisterPresetCategory],
    auth_log: &AuthLogDraft,
) -> DbResult<RegisterUserResult> {
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let user_id = insert_registered_user(connection, draft)?;
        let preset_categories_saved = insert_register_preset_categories(
            connection,
            user_id,
            preset_categories,
            &draft.created_at,
        )?;
        let default_seed = ensure_default_category_seed(connection, user_id, &draft.created_at)?;
        let account_result = create_register_default_accounts(
            connection,
            user_id,
            &draft.language,
            &draft.created_at,
        )?;
        create_auth_log(
            connection,
            &AuthLogDraft {
                user_id: Some(
                    bill_analyser_core::UserId::new(user_id as u64).map_err(|_| {
                        DbError::InvalidOperation("registered user id must be positive".to_string())
                    })?,
                ),
                username: auth_log.username.clone(),
                event_type: auth_log.event_type.clone(),
                ip_address: auth_log.ip_address.clone(),
                user_agent: auth_log.user_agent.clone(),
                success: auth_log.success,
                error_message: auth_log.error_message.clone(),
                metadata: auth_log.metadata.clone(),
                created_at: auth_log.created_at.clone(),
            },
        )?;
        Ok(RegisterUserResult {
            user_id,
            preset_categories_saved,
            preset_accounts_saved: account_result.success,
            cash_account_id: account_result.cash_account_id,
            default_account_id: account_result.default_account_id,
            default_seed,
        })
    })();

    match result {
        Ok(value) => {
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error.into());
            }
            Ok(value)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn exists_by_text(connection: &Connection, sql: &str, value: &str) -> DbResult<bool> {
    connection
        .query_row(sql, [value], |_| Ok(()))
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

fn insert_registered_user(connection: &Connection, draft: &RegisterUserDraft) -> DbResult<i64> {
    connection.execute(
        r#"
        INSERT INTO users (
            username, email, password_hash, nickname, language,
            default_currency, first_day_of_week, is_active, email_verified,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?9)
        "#,
        params![
            draft.username,
            draft.email,
            draft.password_hash,
            draft.nickname,
            draft.language,
            draft.default_currency,
            draft.first_day_of_week,
            if draft.email_verified { 1 } else { 0 },
            draft.created_at,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

fn insert_register_preset_categories(
    connection: &Connection,
    user_id: i64,
    categories: &[RegisterPresetCategory],
    created_at: &str,
) -> DbResult<bool> {
    for item in categories {
        let main_category = item.name.trim();
        if main_category.is_empty() {
            continue;
        }
        insert_category_ignore(
            connection,
            &CategoryInsertDraft {
                user_id,
                type_code: item.type_code,
                main_category,
                sub_category: "",
                priority: 0,
                icon: &item.icon,
                color: &item.color,
                created_at,
            },
        )?;
        for sub_item in &item.sub_categories {
            let sub_category = sub_item.name.trim();
            if sub_category.is_empty() {
                continue;
            }
            insert_category_ignore(
                connection,
                &CategoryInsertDraft {
                    user_id,
                    type_code: item.type_code,
                    main_category,
                    sub_category,
                    priority: 0,
                    icon: if sub_item.icon.is_empty() {
                        &item.icon
                    } else {
                        &sub_item.icon
                    },
                    color: if sub_item.color.is_empty() {
                        &item.color
                    } else {
                        &sub_item.color
                    },
                    created_at,
                },
            )?;
        }
    }
    Ok(true)
}

fn ensure_default_category_seed(
    connection: &Connection,
    user_id: i64,
    created_at: &str,
) -> DbResult<RegisterDefaultSeedSummary> {
    let mut summary = RegisterDefaultSeedSummary {
        categories_created: 0,
        categories_skipped: 0,
        rules_created: 0,
        rules_skipped: 0,
        rules_missing_categories: 0,
    };
    for category in DEFAULT_DAILY_CATEGORIES {
        if insert_category_ignore(
            connection,
            &CategoryInsertDraft {
                user_id,
                type_code: category.type_code,
                main_category: category.name,
                sub_category: "",
                priority: category.priority,
                icon: category.icon,
                color: category.color,
                created_at,
            },
        )? {
            summary.categories_created += 1;
        } else {
            summary.categories_skipped += 1;
        }
        for (offset, sub_category) in category.sub_categories.iter().enumerate() {
            if insert_category_ignore(
                connection,
                &CategoryInsertDraft {
                    user_id,
                    type_code: category.type_code,
                    main_category: category.name,
                    sub_category: sub_category.name,
                    priority: category.priority + offset as i64 + 1,
                    icon: sub_category.icon,
                    color: sub_category.color,
                    created_at,
                },
            )? {
                summary.categories_created += 1;
            } else {
                summary.categories_skipped += 1;
            }
        }
    }
    for rule in DEFAULT_DAILY_CATEGORY_RULES {
        let Some(category_id) =
            find_category_id(connection, user_id, rule.main_category, rule.sub_category)?
        else {
            summary.rules_missing_categories += 1;
            continue;
        };
        if auth_rule_name_exists(connection, user_id, rule.name)? {
            summary.rules_skipped += 1;
            continue;
        }
        let changed = connection.execute(
            r#"
            INSERT INTO category_rules (
                user_id, category_id, name, priority, rule_expression,
                regex_enabled, enabled, applied_count, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 0, 1, 0, ?6, ?6)
            "#,
            params![
                user_id,
                category_id,
                rule.name,
                rule.priority,
                rule.rule_expression,
                created_at,
            ],
        )?;
        if changed > 0 {
            summary.rules_created += 1;
        } else {
            summary.rules_skipped += 1;
        }
    }
    Ok(summary)
}

struct CategoryInsertDraft<'a> {
    user_id: i64,
    type_code: i64,
    main_category: &'a str,
    sub_category: &'a str,
    priority: i64,
    icon: &'a str,
    color: &'a str,
    created_at: &'a str,
}

fn insert_category_ignore(
    connection: &Connection,
    draft: &CategoryInsertDraft<'_>,
) -> DbResult<bool> {
    let changed = connection.execute(
        r#"
        INSERT OR IGNORE INTO categories (
            user_id, type, main_category, sub_category, description, priority,
            keywords, hidden, icon, color, created_at
        ) VALUES (?1, ?2, ?3, ?4, '', ?5, '', 0, ?6, ?7, ?8)
        "#,
        params![
            draft.user_id,
            draft.type_code,
            draft.main_category,
            draft.sub_category,
            draft.priority,
            draft.icon,
            draft.color,
            draft.created_at,
        ],
    )?;
    Ok(changed > 0)
}

fn find_category_id(
    connection: &Connection,
    user_id: i64,
    main_category: &str,
    sub_category: &str,
) -> DbResult<Option<i64>> {
    connection
        .query_row(
            "SELECT id FROM categories WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3",
            params![user_id, main_category, sub_category],
            |row| row.get(0),
        )
        .optional()
        .map_err(DbError::from)
}

fn auth_rule_name_exists(connection: &Connection, user_id: i64, name: &str) -> DbResult<bool> {
    connection
        .query_row(
            "SELECT 1 FROM category_rules WHERE user_id = ?1 AND name = ?2 LIMIT 1",
            params![user_id, name],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

struct RegisterAccountsResult {
    success: bool,
    cash_account_id: Option<i64>,
    default_account_id: Option<i64>,
}

fn create_register_default_accounts(
    connection: &Connection,
    user_id: i64,
    language: &str,
    created_at: &str,
) -> DbResult<RegisterAccountsResult> {
    let templates = if language.to_ascii_lowercase().starts_with("zh") {
        ZH_DEFAULT_ACCOUNTS
    } else {
        DEFAULT_ACCOUNTS
    };
    let mut created_account_ids = Vec::new();
    let mut cash_account_id = None;
    for account in templates {
        let aliases = serde_json::to_string(account.aliases)
            .map_err(|error| DbError::InvalidOperation(error.to_string()))?;
        connection.execute(
            r#"
            INSERT INTO accounts (
                user_id, name, type, category, currency, icon, color,
                balance, initial_balance, hidden, display_order, comment,
                aliases, parent_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, 0, ?8, NULL, ?9, 0, ?10, ?10)
            "#,
            params![
                user_id,
                account.name,
                account.type_code,
                account.category,
                account.currency,
                account.icon,
                account.color,
                account.display_order,
                aliases,
                created_at,
            ],
        )?;
        let account_id = connection.last_insert_rowid();
        created_account_ids.push(account_id);
        if account.category == 1 && cash_account_id.is_none() {
            cash_account_id = Some(account_id);
        }
    }
    let default_account_id = created_account_ids.first().copied();
    if let Some(default_account_id) = default_account_id {
        connection.execute(
            "UPDATE users SET default_account_id = ?1, cash_account_id = ?2, updated_at = ?3 WHERE id = ?4",
            params![default_account_id, cash_account_id, created_at, user_id],
        )?;
    }
    Ok(RegisterAccountsResult {
        success: true,
        cash_account_id,
        default_account_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn register_user_creates_defaults_and_logs_atomically() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                nickname TEXT,
                default_account_id INTEGER,
                language TEXT DEFAULT 'zh_Hans',
                default_currency TEXT DEFAULT 'CNY',
                first_day_of_week INTEGER DEFAULT 1,
                cash_account_id INTEGER,
                is_active INTEGER DEFAULT 1,
                email_verified INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                type INTEGER DEFAULT 1,
                main_category TEXT NOT NULL,
                sub_category TEXT NOT NULL,
                description TEXT,
                priority INTEGER DEFAULT 0,
                keywords TEXT,
                hidden INTEGER DEFAULT 0,
                icon TEXT,
                color TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(user_id, main_category, sub_category)
            );
            CREATE TABLE category_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                category_id INTEGER NOT NULL,
                name TEXT NOT NULL DEFAULT '',
                priority INTEGER NOT NULL DEFAULT 100,
                rule_expression TEXT NOT NULL,
                regex_enabled INTEGER DEFAULT 0,
                enabled INTEGER DEFAULT 1,
                applied_count INTEGER DEFAULT 0,
                last_applied_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                type INTEGER NOT NULL,
                category INTEGER,
                currency TEXT DEFAULT 'CNY',
                icon TEXT,
                color TEXT,
                balance REAL DEFAULT 0,
                initial_balance REAL DEFAULT 0,
                hidden INTEGER DEFAULT 0,
                display_order INTEGER DEFAULT 0,
                comment TEXT,
                aliases TEXT,
                parent_id INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE auth_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER,
                username TEXT,
                event_type TEXT NOT NULL,
                ip_address TEXT,
                user_agent TEXT,
                success INTEGER NOT NULL,
                error_message TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL
            );
            "#,
        )?;

        let result = create_registered_user_with_defaults(
            &connection,
            &RegisterUserDraft {
                username: "alice".to_string(),
                email: "alice@example.test".to_string(),
                password_hash: "bcrypt-hash".to_string(),
                nickname: "Alice".to_string(),
                language: "zh_Hans".to_string(),
                default_currency: "CNY".to_string(),
                first_day_of_week: 1,
                email_verified: false,
                created_at: "2026-05-09T00:00:00".to_string(),
            },
            &[RegisterPresetCategory {
                name: "自定义".to_string(),
                type_code: 3,
                icon: "custom".to_string(),
                color: "123456".to_string(),
                sub_categories: vec![RegisterPresetSubCategory {
                    name: "子类".to_string(),
                    icon: "".to_string(),
                    color: "".to_string(),
                }],
            }],
            &AuthLogDraft {
                user_id: None,
                username: "alice".to_string(),
                event_type: "register_success".to_string(),
                ip_address: "127.0.0.1".to_string(),
                user_agent: "Mozilla".to_string(),
                success: true,
                error_message: None,
                metadata: None,
                created_at: "2026-05-09T00:00:00".to_string(),
            },
        )?;

        assert_eq!(result.user_id, 1);
        assert!(result.preset_categories_saved);
        assert!(result.preset_accounts_saved);
        assert_eq!(result.default_account_id, Some(1));
        assert_eq!(result.cash_account_id, Some(1));
        assert!(result.default_seed.categories_created > 80);
        assert_eq!(
            result.default_seed.rules_created,
            DEFAULT_DAILY_CATEGORY_RULES.len() as i64
        );
        assert!(auth_username_exists(&connection, "alice")?);
        assert!(auth_email_exists(&connection, "alice@example.test")?);
        assert_eq!(
            connection.query_row("SELECT email_verified FROM users WHERE id = 1", [], |row| {
                row.get::<_, i64>(0)
            })?,
            0
        );
        assert_eq!(
            connection.query_row(
                "SELECT COUNT(*) FROM categories WHERE user_id = 1 AND main_category = '自定义'",
                [],
                |row| row.get::<_, i64>(0),
            )?,
            2
        );
        assert_eq!(
            connection.query_row(
                "SELECT type FROM categories WHERE user_id = 1 AND main_category = '账户互转' AND sub_category = ''",
                [],
                |row| row.get::<_, i64>(0),
            )?,
            TRANSFER
        );
        assert_eq!(
            connection.query_row(
                "SELECT name, aliases FROM accounts WHERE id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?,
            (
                "现金".to_string(),
                json!(["现金", "现金钱包", "cash"]).to_string()
            )
        );
        assert_eq!(
            connection.query_row(
                "SELECT event_type FROM auth_logs WHERE user_id = 1",
                [],
                |row| { row.get::<_, String>(0) }
            )?,
            "register_success"
        );
        Ok(())
    }
}
