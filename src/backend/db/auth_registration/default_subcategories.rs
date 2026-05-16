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
