# pylint: disable=wildcard-import,unused-wildcard-import
"""Exchange-rate statistics route handlers."""

from .support import *  # noqa: F403

@bp.route("/exchange-rates", methods=["GET"])
@log_method
@require_auth
def get_exchange_rates():
    """
    获取最新汇率数据（v6.79：从网络获取实时汇率，支持 3 个以上数据源）

    数据源优先级:
        1. ECB (欧洲央行) - 支持30+货币，包括CNY
        2. BOC (加拿大银行) - 支持CNY
        3. RBA (澳大利亚储备银行) - 支持CNY
        4. NBP (波兰国家银行) - 支持主要欧洲货币
        5. SNB (瑞士国家银行) - 支持主要货币

    查询参数：
        base_currency: 基准货币代码，默认为CNY

    返回：
        JSON响应，包含最新汇率信息

    响应格式：
        {
            "success": true,
            "result": {
                "dataSource": "ECB (欧洲央行)",
                "referenceUrl": "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml",
                "updateTime": 1700000000,
                "baseCurrency": "CNY",
                "exchangeRates": [
                    {"currency": "USD", "rate": "0.139"},
                    {"currency": "EUR", "rate": "0.128"}
                ]
            }
        }
    """
    base_currency = "CNY"
    try:
        logger.info("[汇率API] 开始处理汇率数据请求")

        db = get_app_context()
        user_id = _get_request_user_id()

        # 获取请求参数
        base_currency = _get_request_base_currency(db)
        requested_provider = _normalize_requested_exchange_rate_provider()
        logger.debug(
            "[汇率API] 请求参数: base_currency=%s, provider=%s",
            base_currency,
            requested_provider,
        )

        if requested_provider not in EXCHANGE_RATE_PROVIDER_OPTIONS:
            return jsonify(
                {
                    "success": False,
                    "error": f"Unsupported exchange rate provider: {requested_provider}",
                },
            ), 400

        custom_rates = _run_async(db.get_user_custom_exchange_rates(base_currency, user_id))

        if custom_rates and requested_provider == "auto":
            result = _build_user_custom_exchange_rates_result(base_currency, custom_rates)
            logger.info(
                "[汇率API] 返回用户自定义汇率: user_id=%s, base=%s, count=%d",
                user_id,
                base_currency,
                len(result["exchangeRates"]),
            )
            return jsonify({"success": True, "result": result})

        # 获取当前时间戳
        update_time = int(time.time())

        # 定义要获取的目标货币列表
        target_currencies = [
            "USD",
            "EUR",
            "GBP",
            "JPY",
            "HKD",
            "KRW",
            "AUD",
            "CAD",
            "SGD",
            "TWD",
            "MYR",
            "THB",
            "VND",
            "CHF",
            "NZD",
            "CNY",
        ]

        # 移除基准货币（不需要自己对自己的汇率）
        if base_currency in target_currencies:
            target_currencies.remove(base_currency)

        # 创建事件循环获取汇率
        rates_data = _run_async(
            _fetch_exchange_rates_from_providers(
                base_currency,
                target_currencies,
                requested_provider,
            ),
        )

        # 构建汇率列表
        exchange_rates_list = []

        # v6.86: 始终返回基准币种本身（汇率=1.0），避免前端在基准币换算时找不到币种
        exchange_rates_list.append({"currency": base_currency, "rate": "1.0"})

        for currency, rate in rates_data["rates"].items():
            exchange_rates_list.append({"currency": currency, "rate": str(round(rate, 6))})

        # 构建响应数据
        result = {
            "providerKey": rates_data["provider_key"],
            "requestedProvider": requested_provider,
            "fallbackUsed": rates_data["fallback_used"],
            "dataSource": rates_data["source"],
            "referenceUrl": rates_data["url"],
            "updateTime": update_time,
            "baseCurrency": base_currency,
            "exchangeRates": exchange_rates_list,
        }

        logger.info(
            "[汇率API] 成功获取汇率: 来源=%s, 基准=%s, 汇率数=%d",
            rates_data["source"],
            base_currency,
            len(exchange_rates_list),
        )

        return jsonify({"success": True, "result": result})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("[汇率API] 获取汇率数据失败: %s", exc, exc_info=True)

        # 失败时回退到内置汇率
        logger.warning("[汇率API] 回退到内置汇率数据")
        return _get_fallback_exchange_rates(base_currency)


@bp.route("/exchange-rates/custom", methods=["PUT"])
@log_method
@require_auth
def update_user_custom_exchange_rate():
    """更新当前用户自定义汇率。"""
    try:
        data = request.get_json(silent=True) or {}
        currency = (data.get("currency") or "").strip().upper()
        rate_raw = data.get("rate")
        if not currency or rate_raw in [None, ""]:
            return jsonify(
                {
                    "success": False,
                    "error": "Invalid request",
                    "message": "currency and rate are required",
                }
            ), 400

        rate = float(str(rate_raw))
        if rate <= 0:
            return jsonify(
                {
                    "success": False,
                    "error": "Invalid request",
                    "message": "rate must be greater than 0",
                }
            ), 400

        db = get_app_context()
        user_id = _get_request_user_id()
        user = _run_async(db.get_user_by_id(user_id)) or {}
        base_currency = (user.get("default_currency") or "CNY").upper()
        result = _run_async(
            db.upsert_user_custom_exchange_rate(base_currency, currency, rate, user_id),
        )

        if not result.get("success"):
            return jsonify(
                {
                    "success": False,
                    "error": "Internal Server Error",
                    "message": result.get(
                        "message",
                        "Failed to update user custom exchange rate",
                    ),
                }
            ), 500

        return jsonify(
            {
                "success": True,
                "result": {
                    "currency": currency,
                    "rate": str(rate),
                    "updateTime": result.get("update_time", int(time.time())),
                },
            }
        )

    except ValueError:
        return jsonify(
            {"success": False, "error": "Invalid request", "message": "rate must be numeric"},
        ), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新用户自定义汇率失败: %s", exc, exc_info=True)
        return jsonify(
            {"success": False, "error": "Internal Server Error", "message": str(exc)},
        ), 500


@bp.route("/exchange-rates/custom/<currency>", methods=["DELETE"])
@log_method
@require_auth
def delete_user_custom_exchange_rate(currency: str):
    """删除当前用户自定义汇率。"""
    try:
        normalized_currency = (currency or "").strip().upper()
        if not normalized_currency:
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "currency is required"},
            ), 400

        db = get_app_context()
        user_id = _get_request_user_id()
        user = _run_async(db.get_user_by_id(user_id)) or {}
        base_currency = (user.get("default_currency") or "CNY").upper()
        deleted = _run_async(
            db.delete_user_custom_exchange_rate(base_currency, normalized_currency, user_id),
        )

        return jsonify({"success": True, "result": deleted})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("删除用户自定义汇率失败: %s", exc, exc_info=True)
        return jsonify(
            {"success": False, "error": "Internal Server Error", "message": str(exc)},
        ), 500


async def _fetch_exchange_rates_from_providers(
    base_currency: str, target_currencies: list, requested_provider: str = "auto"
) -> dict:
    """
    从多个数据源获取汇率 (v6.79)

    按优先级尝试各个提供者，成功获取后立即返回。

    Args:
        base_currency: 基准货币
        target_currencies: 目标货币列表

    Returns:
        dict: {
            'rates': {currency: rate, ...},
            'source': '数据源名称',
            'url': '参考URL',
            'provider_key': 'provider key',
            'fallback_used': bool
        }
    """
    provider_instances = {
        "boc_cn": BOCChinaProvider(),
        "cmb_cn": CMBChinaProvider(),
        "ecb": ECBProvider(),
        "rba": RBAProvider(),
    }

    candidate_order = _build_provider_candidate_order(requested_provider)
    if not candidate_order:
        raise RuntimeError(f"Unsupported exchange rate provider: {requested_provider}")

    last_error = None

    for provider_key in candidate_order:
        provider_meta = EXCHANGE_RATE_PROVIDER_OPTIONS[provider_key]
        provider = provider_instances[provider_key]
        source_name = provider_meta["label"]
        reference_url = provider_meta["reference_url"]

        try:
            logger.info("[汇率API] 尝试从 %s 获取汇率...", source_name)

            # 调用提供者的fetch_rates方法
            rates = await provider.fetch_rates(base_currency, target_currencies)

            if rates and len(rates) > 0:
                logger.info("[汇率API] 成功从 %s 获取 %d 个汇率", source_name, len(rates))
                return {
                    "rates": rates,
                    "source": source_name,
                    "url": reference_url,
                    "provider_key": provider_key,
                    "fallback_used": requested_provider not in {"auto", provider_key},
                }

            logger.warning("[汇率API] %s 返回空汇率数据", source_name)

        except Exception as exc:  # pylint: disable=broad-exception-caught
            last_error = exc
            logger.warning("[汇率API] 从 %s 获取汇率失败: %s", source_name, exc)
            continue

    # 所有提供者都失败，抛出异常
    raise RuntimeError(f"所有汇率数据源都无法获取数据: {last_error}")


def _get_fallback_exchange_rates(base_currency: str):
    """
    获取回退的内置汇率数据 (v6.79)

    当网络获取失败时使用内置的静态汇率作为回退。

    Args:
        base_currency: 基准货币

    Returns:
        Flask JSON响应
    """
    update_time = int(time.time())

    # 内置汇率（以CNY为基准）- 仅作为回退
    cny_based_rates = {
        "USD": 0.139,  # 1 CNY = 0.139 USD
        "EUR": 0.128,  # 1 CNY = 0.128 EUR
        "GBP": 0.110,  # 1 CNY = 0.110 GBP
        "JPY": 20.76,  # 1 CNY = 20.76 JPY
        "HKD": 1.087,  # 1 CNY = 1.087 HKD
        "KRW": 183.33,  # 1 CNY = 183.33 KRW
        "AUD": 0.211,  # 1 CNY = 0.211 AUD
        "CAD": 0.189,  # 1 CNY = 0.189 CAD
        "SGD": 0.186,  # 1 CNY = 0.186 SGD
        "TWD": 4.35,  # 1 CNY = 4.35 TWD
        "MYR": 0.646,  # 1 CNY = 0.646 MYR
        "THB": 4.86,  # 1 CNY = 4.86 THB
        "VND": 3425.0,  # 1 CNY = 3425 VND
        "CHF": 0.123,  # 1 CNY = 0.123 CHF
        "NZD": 0.231,  # 1 CNY = 0.231 NZD
    }

    exchange_rates_list = []

    if base_currency == "CNY":
        exchange_rates_list.append({"currency": "CNY", "rate": "1.0"})
        for currency, rate in cny_based_rates.items():
            exchange_rates_list.append({"currency": currency, "rate": str(round(rate, 6))})
    elif base_currency in cny_based_rates:
        base_rate = cny_based_rates[base_currency]
        exchange_rates_list.append({"currency": base_currency, "rate": "1.0"})
        exchange_rates_list.append(
            {"currency": "CNY", "rate": str(round(1.0 / base_rate, 6))},
        )
        for currency, rate in cny_based_rates.items():
            if currency != base_currency:
                cross_rate = rate / base_rate
                exchange_rates_list.append(
                    {"currency": currency, "rate": str(round(cross_rate, 6))},
                )

    result = {
        "providerKey": "fallback",
        "requestedProvider": "auto",
        "fallbackUsed": True,
        "dataSource": "内置汇率数据 (回退)",
        "referenceUrl": "",
        "updateTime": update_time,
        "baseCurrency": base_currency,
        "exchangeRates": exchange_rates_list,
    }

    logger.info(
        "[汇率API] 使用内置回退汇率: 基准=%s, 汇率数=%d",
        base_currency,
        len(exchange_rates_list),
    )

    return jsonify({"success": True, "result": result})


# ==================== V1 统计分析 API（兼容 ezBookkeeping） ====================



__all__ = [name for name in globals() if not name.startswith("__")]
