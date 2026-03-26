"""Investment recognition settings and default dictionaries.

将投资识别用到的可编辑关键词与默认抽取规则集中管理，
避免散落在业务代码中硬编码。
"""

from __future__ import annotations

import json
from typing import Any, Dict, List, Optional, Sequence, Tuple

DEFAULT_INVESTMENT_PLATFORM_KEYWORDS: List[str] = [
    "蚂蚁财富",
    "天天基金",
    "理财通",
    "东方财富",
    "同花顺",
    "雪球",
    "基金销售平台",
    "证券账户",
    "余额宝",
    "招财宝",
    "国债逆回购",
    "京东金融",
    "肯特瑞",
    "且慢",
    "蛋卷基金",
    "陆金所",
    "度小满理财",
    "招银理财",
    "工银理财",
    "建信理财",
    "中银理财",
    "农银理财",
    "华泰证券",
    "招商证券",
    "中信证券",
    "广发证券",
    "国泰君安",
    "富途证券",
    "老虎证券",
]

DEFAULT_INVESTMENT_PRODUCT_KEYWORDS: List[str] = [
    "基金",
    "理财",
    "定投",
    "申购",
    "赎回",
    "买入",
    "卖出",
    "货币基金",
    "指数基金",
    "债券基金",
    "混合基金",
    "REITs",
    "REIT",
    "ETF",
    "LOF",
    "QDII",
    "债券",
    "股票",
    "黄金",
    "可转债",
    "固收+",
    "现金管理",
    "养老目标",
    "国债",
    "逆回购",
    "组合",
    "计划",
]

DEFAULT_INVESTMENT_EXCLUDE_KEYWORDS: List[str] = [
    "还款",
    "借呗",
    "花呗",
    "贷款",
    "信用卡",
    "房贷",
    "车贷",
    "待还款",
    "分期",
    "账单",
    "生活缴费",
    "水费",
    "电费",
    "话费",
]

DEFAULT_INVESTMENT_PLATFORM_ALIASES: List[Tuple[str, List[str]]] = [
    ("蚂蚁财富", ["蚂蚁财富", "蚂蚁（杭州）基金销售有限公司", "蚂蚁基金"]),
    ("天天基金", ["天天基金", "上海天天基金销售有限公司"]),
    ("理财通", ["理财通", "微信理财通", "腾讯理财通"]),
    ("东方财富", ["东方财富", "东方财富证券"]),
    ("同花顺", ["同花顺"]),
    ("雪球", ["雪球"]),
    ("余额宝", ["余额宝"]),
    ("招财宝", ["招财宝"]),
    ("国债逆回购", ["国债逆回购", "逆回购"]),
    ("证券账户", ["证券", "证券账户"]),
    ("基金销售平台", ["基金销售有限公司", "基金销售"]),
    ("京东金融", ["京东金融", "京东小金库", "京东肯特瑞", "肯特瑞"]),
    ("且慢", ["且慢", "盈米基金", "盈米财富"]),
    ("蛋卷基金", ["蛋卷基金", "雪球基金", "蛋卷"]),
    ("陆金所", ["陆金所", "陆基金", "陆金所基金"]),
    ("度小满理财", ["度小满", "度小满理财", "百度理财"]),
    ("招银理财", ["招银理财", "招商银行理财", "朝朝宝"]),
    ("工银理财", ["工银理财", "工商银行理财", "工银瑞信"]),
    ("建信理财", ["建信理财", "建设银行理财", "建信基金"]),
    ("中银理财", ["中银理财", "中国银行理财"]),
    ("农银理财", ["农银理财", "农业银行理财"]),
    ("华泰证券", ["华泰证券", "涨乐财富通"]),
    ("招商证券", ["招商证券", "智远一户通"]),
    ("中信证券", ["中信证券", "信e投"]),
    ("广发证券", ["广发证券", "易淘金"]),
    ("国泰君安", ["国泰君安", "君弘"]),
    ("富途证券", ["富途", "富途证券"]),
    ("老虎证券", ["老虎证券", "tiger trade"]),
]

DEFAULT_INVESTMENT_PRODUCT_PATTERNS: List[Tuple[str, List[str]]] = [
    ("货币基金", ["货币基金"]),
    ("指数基金", ["指数基金", "指数增强", "宽基指数"]),
    ("债券基金", ["债券基金"]),
    ("混合基金", ["混合基金"]),
    ("REITs", ["reits", "reit"]),
    ("ETF", ["etf"]),
    ("LOF", ["lof"]),
    ("QDII", ["qdii"]),
    ("基金", ["基金"]),
    ("理财", ["理财", "理财产品"]),
    ("债券", ["债券"]),
    ("股票", ["股票"]),
    ("黄金", ["黄金"]),
    ("定投", ["定投"]),
    ("可转债", ["可转债"]),
    ("固收+", ["固收+", "固收"]),
    ("现金管理", ["现金管理"]),
    ("养老目标基金", ["养老目标", "养老fof"]),
    ("国债逆回购", ["国债逆回购", "逆回购"]),
    ("组合", ["组合", "策略组合"]),
    ("计划", ["计划", "资管计划"]),
]

DEFAULT_INVESTMENT_NAMED_PRODUCT_PATTERNS: List[str] = [
    r"([A-Za-z0-9\u4e00-\u9fa5·（）()]{2,80}(?:基金|ETF|LOF|REITs|REIT|理财(?:产品)?|组合|计划))(?:买入|卖出|申购|赎回|定投|扣款|自动定投|转入|转出)",
    r"([A-Za-z0-9\u4e00-\u9fa5·（）()]{2,80}(?:基金|ETF|LOF|REITs|REIT|理财(?:产品)?|资管计划|计划|组合|债券|股票|黄金))",
    r"([A-Za-z0-9\u4e00-\u9fa5·（）()]{2,80}(?:联接A|联接C|A类|C类|D类|E类|F类))",
    r"(?:买入|卖出|申购|赎回|定投|扣款|自动定投|转入|转出)[-－:：\s]*([A-Za-z0-9\u4e00-\u9fa5·（）()]{2,80}(?:基金|ETF|LOF|REITs|REIT|理财(?:产品)?|组合|计划))",
    r"([A-Za-z0-9\u4e00-\u9fa5·（）()]{2,80}(?:号|号计划|精选组合|策略组合))",
]


def _dedupe_keywords(keywords: Sequence[str]) -> List[str]:
    """清理并去重关键词列表。"""
    result: List[str] = []
    seen = set()
    for keyword in keywords:
        text = str(keyword or "").strip()
        if not text:
            continue
        lowered = text.lower()
        if lowered in seen:
            continue
        seen.add(lowered)
        result.append(text)
    return result


def normalize_keyword_list(raw_value: Any, fallback: Optional[Sequence[str]] = None) -> List[str]:
    """将任意输入标准化为关键词列表。"""
    if raw_value is None or raw_value == "":
        return _dedupe_keywords(fallback or [])

    if isinstance(raw_value, (list, tuple, set)):
        return _dedupe_keywords(list(raw_value))

    if isinstance(raw_value, str):
        text = raw_value.strip()
        if not text:
            return _dedupe_keywords(fallback or [])

        parsed = None
        if text.startswith("["):
            try:
                parsed = json.loads(text)
            except TypeError, ValueError, json.JSONDecodeError:
                parsed = None

        if isinstance(parsed, list):
            return _dedupe_keywords(parsed)

        for separator in ["\n", ",", "，", "|", "、", ";", "；"]:
            text = text.replace(separator, "\n")
        return _dedupe_keywords(text.split("\n"))

    return _dedupe_keywords(fallback or [])


def serialize_keyword_list(raw_value: Any) -> str:
    """将关键词列表序列化为 JSON 字符串。"""
    return json.dumps(normalize_keyword_list(raw_value, []), ensure_ascii=False)


def build_user_investment_keyword_settings(user: Optional[Dict[str, Any]]) -> Dict[str, List[str]]:
    """构建用户有效的投资识别关键词设置。"""
    user = user or {}
    return {
        "platform_keywords": normalize_keyword_list(
            user.get("investment_platform_keywords"), DEFAULT_INVESTMENT_PLATFORM_KEYWORDS
        ),
        "product_keywords": normalize_keyword_list(
            user.get("investment_product_keywords"), DEFAULT_INVESTMENT_PRODUCT_KEYWORDS
        ),
        "exclude_keywords": normalize_keyword_list(
            user.get("investment_exclude_keywords"), DEFAULT_INVESTMENT_EXCLUDE_KEYWORDS
        ),
    }
