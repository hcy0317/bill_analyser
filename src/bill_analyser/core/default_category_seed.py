"""Default daily-life categories and recognition rules."""

# pylint: disable=too-many-arguments

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from bill_analyser.utils.constants import TransactionType


@dataclass(frozen=True)
class DefaultSubCategory:
    """One default secondary category."""

    name: str
    icon: str
    color: str


@dataclass(frozen=True)
class DefaultCategory:
    """One default primary category and its secondary categories."""

    type: int
    name: str
    icon: str
    color: str
    priority: int
    sub_categories: tuple[DefaultSubCategory, ...]


@dataclass(frozen=True)
class DefaultCategoryRule:
    """One default category recognition rule."""

    name: str
    main_category: str
    sub_category: str
    rule_expression: str
    priority: int


DEFAULT_DAILY_CATEGORIES: tuple[DefaultCategory, ...] = (
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "餐饮",
        "1",
        "ff6b22",
        100,
        (
            DefaultSubCategory("早餐", "2", "ff6b22"),
            DefaultSubCategory("午餐", "2", "ff6b22"),
            DefaultSubCategory("晚餐", "2", "ff6b22"),
            DefaultSubCategory("咖啡奶茶", "30", "ff6b22"),
            DefaultSubCategory("零食饮料", "70", "ff6b22"),
            DefaultSubCategory("外卖", "2", "ff6b22"),
            DefaultSubCategory("聚餐", "540", "ff6b22"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "食品日用",
        "210",
        "4caf50",
        200,
        (
            DefaultSubCategory("超市便利", "210", "4caf50"),
            DefaultSubCategory("菜场生鲜", "70", "4caf50"),
            DefaultSubCategory("粮油调味", "2", "4caf50"),
            DefaultSubCategory("日用品", "210", "4caf50"),
            DefaultSubCategory("清洁纸品", "210", "4caf50"),
            DefaultSubCategory("宠物", "580", "4caf50"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "居住家庭",
        "200",
        "607d8b",
        300,
        (
            DefaultSubCategory("房租", "290", "607d8b"),
            DefaultSubCategory("房贷", "290", "607d8b"),
            DefaultSubCategory("物业", "200", "607d8b"),
            DefaultSubCategory("燃气", "270", "607d8b"),
            DefaultSubCategory("水费", "270", "607d8b"),
            DefaultSubCategory("电费", "270", "607d8b"),
            DefaultSubCategory("网络电视", "430", "607d8b"),
            DefaultSubCategory("家具家电", "230", "607d8b"),
            DefaultSubCategory("维修", "250", "607d8b"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "交通出行",
        "300",
        "009688",
        400,
        (
            DefaultSubCategory("公交地铁", "310", "009688"),
            DefaultSubCategory("打车网约车", "320", "009688"),
            DefaultSubCategory("共享单车", "310", "009688"),
            DefaultSubCategory("高铁火车", "370", "009688"),
            DefaultSubCategory("机票", "390", "009688"),
            DefaultSubCategory("加油充电", "330", "009688"),
            DefaultSubCategory("停车过路", "330", "009688"),
            DefaultSubCategory("保养保险", "330", "009688"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "医疗健康",
        "800",
        "ff3b30",
        500,
        (
            DefaultSubCategory("挂号诊疗", "840", "ff3b30"),
            DefaultSubCategory("药品", "860", "ff3b30"),
            DefaultSubCategory("体检", "840", "ff3b30"),
            DefaultSubCategory("口腔", "840", "ff3b30"),
            DefaultSubCategory("眼镜", "890", "ff3b30"),
            DefaultSubCategory("运动健身", "510", "ff3b30"),
            DefaultSubCategory("保健护理", "890", "ff3b30"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "服饰美妆",
        "100",
        "673ab7",
        600,
        (
            DefaultSubCategory("服装", "110", "673ab7"),
            DefaultSubCategory("鞋包", "110", "673ab7"),
            DefaultSubCategory("护肤彩妆", "180", "673ab7"),
            DefaultSubCategory("美发美甲", "190", "673ab7"),
            DefaultSubCategory("配饰", "170", "673ab7"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "数码办公",
        "230",
        "3f51b5",
        700,
        (
            DefaultSubCategory("手机电脑", "230", "3f51b5"),
            DefaultSubCategory("软件工具", "230", "3f51b5"),
            DefaultSubCategory("办公用品", "210", "3f51b5"),
            DefaultSubCategory("维修配件", "250", "3f51b5"),
            DefaultSubCategory("云服务", "430", "3f51b5"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "教育成长",
        "600",
        "cddc39",
        800,
        (
            DefaultSubCategory("学费", "600", "cddc39"),
            DefaultSubCategory("课程培训", "660", "cddc39"),
            DefaultSubCategory("图书资料", "610", "cddc39"),
            DefaultSubCategory("考试证书", "680", "cddc39"),
            DefaultSubCategory("儿童教育", "660", "cddc39"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "娱乐休闲",
        "500",
        "ff2d55",
        900,
        (
            DefaultSubCategory("电影演出", "550", "ff2d55"),
            DefaultSubCategory("游戏", "560", "ff2d55"),
            DefaultSubCategory("会员订阅", "570", "ff2d55"),
            DefaultSubCategory("旅游门票", "590", "ff2d55"),
            DefaultSubCategory("棋牌桌游", "560", "ff2d55"),
            DefaultSubCategory("兴趣爱好", "560", "ff2d55"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "旅行住宿",
        "590",
        "00bcd4",
        1000,
        (
            DefaultSubCategory("酒店民宿", "590", "00bcd4"),
            DefaultSubCategory("景点", "590", "00bcd4"),
            DefaultSubCategory("旅行团", "590", "00bcd4"),
            DefaultSubCategory("签证保险", "950", "00bcd4"),
            DefaultSubCategory("行李用品", "110", "00bcd4"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "人情社交",
        "700",
        "4cd964",
        1100,
        (
            DefaultSubCategory("红包转账", "710", "4cd964"),
            DefaultSubCategory("礼物", "710", "4cd964"),
            DefaultSubCategory("请客", "540", "4cd964"),
            DefaultSubCategory("婚丧喜庆", "710", "4cd964"),
            DefaultSubCategory("捐赠公益", "780", "4cd964"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "金融保险",
        "900",
        "ff9500",
        1200,
        (
            DefaultSubCategory("保险", "950", "ff9500"),
            DefaultSubCategory("贷款利息", "970", "ff9500"),
            DefaultSubCategory("手续费", "930", "ff9500"),
            DefaultSubCategory("税费罚款", "910", "ff9500"),
            DefaultSubCategory("投资支出", "810", "ff9500"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.EXPENSE),
        "其他支出",
        "1000",
        "8e8e93",
        1300,
        (
            DefaultSubCategory("无法归类", "1010", "8e8e93"),
            DefaultSubCategory("临时杂项", "1010", "8e8e93"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.INCOME),
        "工作收入",
        "2000",
        "ff6b22",
        2000,
        (
            DefaultSubCategory("工资", "2010", "ff6b22"),
            DefaultSubCategory("奖金", "2020", "ff6b22"),
            DefaultSubCategory("补贴", "231", "ff6b22"),
            DefaultSubCategory("报销", "920", "ff6b22"),
            DefaultSubCategory("兼职", "2080", "ff6b22"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.INCOME),
        "经营收入",
        "2080",
        "4caf50",
        2100,
        (
            DefaultSubCategory("销售收入", "2080", "4caf50"),
            DefaultSubCategory("服务收入", "2080", "4caf50"),
            DefaultSubCategory("佣金", "2080", "4caf50"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.INCOME),
        "投资收益",
        "2100",
        "ff9500",
        2200,
        (
            DefaultSubCategory("利息", "970", "ff9500"),
            DefaultSubCategory("股息", "2100", "ff9500"),
            DefaultSubCategory("基金股票", "810", "ff9500"),
            DefaultSubCategory("理财收益", "830", "ff9500"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.INCOME),
        "生活收入",
        "710",
        "4cd964",
        2300,
        (
            DefaultSubCategory("退款", "920", "4cd964"),
            DefaultSubCategory("报销入账", "920", "4cd964"),
            DefaultSubCategory("红包礼金", "710", "4cd964"),
            DefaultSubCategory("二手售卖", "2080", "4cd964"),
            DefaultSubCategory("租金收入", "290", "4cd964"),
        ),
    ),
    DefaultCategory(
        int(TransactionType.INCOME),
        "其他收入",
        "1000",
        "8e8e93",
        2400,
        (DefaultSubCategory("其他", "3010", "8e8e93"),),
    ),
    DefaultCategory(
        int(TransactionType.TRANSFER),
        "账户互转",
        "4000",
        "2196f3",
        3000,
        (
            DefaultSubCategory("银行卡互转", "900", "2196f3"),
            DefaultSubCategory("余额充值", "981", "2196f3"),
            DefaultSubCategory("信用卡还款", "980", "2196f3"),
            DefaultSubCategory("提现", "981", "2196f3"),
            DefaultSubCategory("借还款", "930", "2196f3"),
        ),
    ),
)


DEFAULT_DAILY_CATEGORY_RULES: tuple[DefaultCategoryRule, ...] = (
    DefaultCategoryRule(
        "default:餐饮/外卖",
        "餐饮",
        "外卖",
        "(OR={美团外卖,饿了么,外卖,饭团}/REGEX={(美团|饿了么).*(外卖|订单)})+NOT={退款,退货,取消,冲正}",
        100,
    ),
    DefaultCategoryRule(
        "default:餐饮/咖啡奶茶",
        "餐饮",
        "咖啡奶茶",
        "OR={瑞幸,星巴克,库迪,奈雪,喜茶,蜜雪冰城,霸王茶姬,沪上阿姨,茶百道,咖啡,奶茶}",
        110,
    ),
    DefaultCategoryRule(
        "default:餐饮/早餐",
        "餐饮",
        "早餐",
        "REGEX={(早餐|早饭|包子|豆浆|油条|粥店)}",
        120,
    ),
    DefaultCategoryRule(
        "default:食品日用/超市便利",
        "食品日用",
        "超市便利",
        "OR={盒马,山姆,沃尔玛,永辉,华润万家,便利蜂,罗森,全家,7-11,超市,便利店}",
        200,
    ),
    DefaultCategoryRule(
        "default:食品日用/菜场生鲜",
        "食品日用",
        "菜场生鲜",
        "OR={叮咚买菜,朴朴,每日优鲜,菜市场,生鲜,水果,蔬菜,肉铺}",
        210,
    ),
    DefaultCategoryRule(
        "default:居住家庭/水费",
        "居住家庭",
        "水费",
        "OR={自来水,水务}/REGEX={水费}",
        300,
    ),
    DefaultCategoryRule(
        "default:居住家庭/电费",
        "居住家庭",
        "电费",
        "OR={国家电网,南方电网,供电}/REGEX={电费}",
        310,
    ),
    DefaultCategoryRule(
        "default:居住家庭/燃气",
        "居住家庭",
        "燃气",
        "OR={燃气,天然气,煤气}",
        320,
    ),
    DefaultCategoryRule(
        "default:居住家庭/房租",
        "居住家庭",
        "房租",
        "(OR={房租,租金,公寓}/REGEX={(房租|租金).*(支付|转账|缴费)})+NOT={退款,退回}",
        330,
    ),
    DefaultCategoryRule(
        "default:交通出行/打车网约车",
        "交通出行",
        "打车网约车",
        "OR={滴滴,高德打车,曹操出行,T3出行,花小猪,出租车}",
        400,
    ),
    DefaultCategoryRule(
        "default:交通出行/公交地铁",
        "交通出行",
        "公交地铁",
        "OR={地铁,公交,交通卡,一卡通,乘车码}",
        410,
    ),
    DefaultCategoryRule(
        "default:交通出行/高铁火车",
        "交通出行",
        "高铁火车",
        "OR={铁路12306,12306,火车票,高铁票,动车票}",
        420,
    ),
    DefaultCategoryRule(
        "default:交通出行/机票",
        "交通出行",
        "机票",
        "OR={航旅纵横,机票,航空,机场,携程,飞猪,同程,去哪儿}",
        430,
    ),
    DefaultCategoryRule(
        "default:交通出行/加油充电",
        "交通出行",
        "加油充电",
        "OR={中石化,中石油,壳牌,加油,充电桩,特来电,星星充电}",
        440,
    ),
    DefaultCategoryRule(
        "default:交通出行/停车过路",
        "交通出行",
        "停车过路",
        "OR={停车,ETCP,高速费,过路费,路桥费}",
        450,
    ),
    DefaultCategoryRule(
        "default:医疗健康/挂号诊疗",
        "医疗健康",
        "挂号诊疗",
        "OR={医院,诊所,挂号,门诊,急诊,体检,口腔}",
        500,
    ),
    DefaultCategoryRule(
        "default:医疗健康/药品",
        "医疗健康",
        "药品",
        "OR={药房,药店,阿里健康,京东健康,叮当快药,药品}",
        510,
    ),
    DefaultCategoryRule(
        "default:服饰美妆/护肤彩妆",
        "服饰美妆",
        "护肤彩妆",
        "OR={屈臣氏,丝芙兰,护肤,彩妆,化妆品,美妆}",
        600,
    ),
    DefaultCategoryRule(
        "default:数码办公/软件工具",
        "数码办公",
        "软件工具",
        "OR={Apple,App Store,微软,Adobe,JetBrains,Notion,飞书,钉钉,软件,订阅,云服务}",
        700,
    ),
    DefaultCategoryRule(
        "default:教育成长/课程培训",
        "教育成长",
        "课程培训",
        "OR={学费,培训,课程,得到,知识星球,考试,报名费,教材}",
        800,
    ),
    DefaultCategoryRule(
        "default:娱乐休闲/会员订阅",
        "娱乐休闲",
        "会员订阅",
        "OR={腾讯视频,爱奇艺,优酷,网易云音乐,QQ音乐,B站大会员,Netflix,Spotify,会员,订阅}",
        900,
    ),
    DefaultCategoryRule(
        "default:娱乐休闲/游戏",
        "娱乐休闲",
        "游戏",
        "OR={Steam,PlayStation,Nintendo,腾讯游戏,网易游戏,米哈游,游戏}",
        910,
    ),
    DefaultCategoryRule(
        "default:旅行住宿/酒店民宿",
        "旅行住宿",
        "酒店民宿",
        "OR={酒店,民宿,携程酒店,飞猪酒店,美团酒店,Booking,Airbnb}",
        1000,
    ),
    DefaultCategoryRule(
        "default:人情社交/红包转账",
        "人情社交",
        "红包转账",
        "(OR={红包,礼金,份子钱}/REGEX={(微信|支付宝).*(红包|转账)})+NOT={退款,退回,还款}",
        1100,
    ),
    DefaultCategoryRule(
        "default:金融保险/保险",
        "金融保险",
        "保险",
        "OR={保险,保费,众安,平安保险,太平洋保险,医保,社保}",
        1200,
    ),
    DefaultCategoryRule(
        "default:金融保险/手续费",
        "金融保险",
        "手续费",
        "OR={手续费,服务费,年费,管理费}",
        1210,
    ),
    DefaultCategoryRule(
        "default:工作收入/工资",
        "工作收入",
        "工资",
        "REGEX={(工资|薪资|薪金|工资代发|工资发放)}",
        2000,
    ),
    DefaultCategoryRule(
        "default:工作收入/奖金",
        "工作收入",
        "奖金",
        "REGEX={(奖金|绩效|年终奖)}",
        2010,
    ),
    DefaultCategoryRule(
        "default:生活收入/退款",
        "生活收入",
        "退款",
        "REGEX={(退款|退货|冲正|原路退回)}",
        2300,
    ),
    DefaultCategoryRule(
        "default:工作收入/报销",
        "工作收入",
        "报销",
        "REGEX={报销}",
        2310,
    ),
    DefaultCategoryRule(
        "default:账户互转/信用卡还款",
        "账户互转",
        "信用卡还款",
        "OR={信用卡还款,还信用卡}",
        3000,
    ),
    DefaultCategoryRule(
        "default:账户互转/银行卡互转",
        "账户互转",
        "银行卡互转",
        "REGEX={(银行卡转入|银行卡转出|转账到银行卡|银行卡入账)}",
        3010,
    ),
    DefaultCategoryRule(
        "default:账户互转/余额充值",
        "账户互转",
        "余额充值",
        "REGEX={(充值|余额宝转入|零钱通转入)}",
        3020,
    ),
    DefaultCategoryRule(
        "default:账户互转/提现",
        "账户互转",
        "提现",
        "REGEX={(提现|余额宝转出|零钱通转出)}",
        3030,
    ),
)


async def ensure_default_category_seed(db: Any, user_id: int = 1) -> dict[str, Any]:
    """Create missing daily-life default categories and rules for one user."""

    category_result = await ensure_default_categories(db, user_id=user_id)
    rule_result = await ensure_default_category_rules(db, user_id=user_id)
    return {
        "categories": category_result,
        "rules": rule_result,
    }


async def ensure_default_categories(db: Any, user_id: int = 1) -> dict[str, int]:
    """Create missing primary and secondary categories without overwriting custom data."""

    category_payloads = _default_category_payloads()
    ensure_categories = getattr(db, "ensure_categories", None)
    if callable(ensure_categories):
        return await ensure_categories(category_payloads, user_id=user_id)

    created = 0
    skipped = 0

    for payload in category_payloads:
        created_id = await db.create_category(payload, user_id=user_id)
        if created_id is not None:
            created += 1
        else:
            skipped += 1

    return {"created": created, "skipped": skipped}


async def ensure_default_category_rules(db: Any, user_id: int = 1) -> dict[str, int]:
    """Create missing default recognition rules in canonical ``category_rules``."""

    created = 0
    skipped = 0
    missing_categories = 0
    categories_by_name = await _load_category_lookup(db, user_id=user_id)
    existing_rule_names = await _load_existing_rule_names(db, user_id=user_id)

    for rule in DEFAULT_DAILY_CATEGORY_RULES:
        category = categories_by_name.get((rule.main_category, rule.sub_category))
        if not category:
            missing_categories += 1
            continue

        if rule.name in existing_rule_names:
            skipped += 1
            continue

        rule_id = await db.create_category_rule(
            {
                "category_id": category["id"],
                "name": rule.name,
                "priority": rule.priority,
                "rule_expression": rule.rule_expression,
                "regex_enabled": False,
                "enabled": True,
            },
            user_id=user_id,
        )
        if rule_id is None:
            skipped += 1
        else:
            created += 1
            existing_rule_names.add(rule.name)

    return {
        "created": created,
        "skipped": skipped,
        "missingCategories": missing_categories,
    }


def _default_category_payloads() -> list[dict[str, Any]]:
    """Build default category rows in the same order as the seed constants."""
    payloads: list[dict[str, Any]] = []
    for category in DEFAULT_DAILY_CATEGORIES:
        payloads.append(
            {
                "type": category.type,
                "main_category": category.name,
                "sub_category": "",
                "description": "",
                "priority": category.priority,
                "keywords": "",
                "hidden": False,
                "icon": category.icon,
                "color": category.color,
            }
        )
        for offset, sub_category in enumerate(category.sub_categories, start=1):
            payloads.append(
                {
                    "type": category.type,
                    "main_category": category.name,
                    "sub_category": sub_category.name,
                    "description": "",
                    "priority": category.priority + offset,
                    "keywords": "",
                    "hidden": False,
                    "icon": sub_category.icon,
                    "color": sub_category.color,
                }
            )
    return payloads


async def _load_category_lookup(db: Any, *, user_id: int) -> dict[tuple[str, str], dict[str, Any]]:
    categories = await db.get_all_categories(user_id=user_id)
    return {
        (
            str(category.get("main_category") or ""),
            str(category.get("sub_category") or ""),
        ): category
        for category in categories
        if int(category.get("id", 0) or 0) > 0
    }


async def _load_existing_rule_names(db: Any, *, user_id: int) -> set[str]:
    conn = await db._get_connection()  # pylint: disable=protected-access
    async with conn.execute(
        "SELECT name FROM category_rules WHERE user_id = ?",
        (user_id,),
    ) as cursor:
        rows = await cursor.fetchall()
    return {str(row[0]) for row in rows}
