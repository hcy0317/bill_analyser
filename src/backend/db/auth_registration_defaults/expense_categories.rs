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
