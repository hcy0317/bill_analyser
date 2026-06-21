/// 返回统计汇率功能支持的 provider 元数据，用于前端展示和后端校验。
#[tracing::instrument(level = "debug", skip_all)]
pub fn exchange_rate_provider_options() -> BTreeMap<String, ExchangeRateProviderOption> {
    BTreeMap::from([
        (
            "auto".to_string(),
            ExchangeRateProviderOption {
                label: "自动选择".to_string(),
                reference_url: String::new(),
                region: "mixed".to_string(),
            },
        ),
        (
            "boc_cn".to_string(),
            ExchangeRateProviderOption {
                label: "中国银行外汇牌价".to_string(),
                reference_url: "https://www.boc.cn/sourcedb/whpj/".to_string(),
                region: "domestic".to_string(),
            },
        ),
        (
            "cmb_cn".to_string(),
            ExchangeRateProviderOption {
                label: "招商银行实时汇率".to_string(),
                reference_url: "https://fx.cmbchina.com/hq/".to_string(),
                region: "domestic".to_string(),
            },
        ),
        (
            "ecb".to_string(),
            ExchangeRateProviderOption {
                label: "ECB (欧洲央行)".to_string(),
                reference_url: "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml"
                    .to_string(),
                region: "foreign".to_string(),
            },
        ),
        (
            "rba".to_string(),
            ExchangeRateProviderOption {
                label: "RBA (澳大利亚储备银行)".to_string(),
                reference_url: "https://www.rba.gov.au/rss/rss-cb-exchange-rates.xml".to_string(),
                region: "foreign".to_string(),
            },
        ),
    ])
}

/// 规范化用户请求的汇率 provider，空值统一为 auto。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_requested_exchange_rate_provider(raw: Option<&str>) -> String {
    let provider = raw.unwrap_or("auto").trim().to_lowercase();
    if provider.is_empty() {
        "auto".to_string()
    } else {
        provider
    }
}

/// 根据请求 provider 生成回退候选顺序，auto 使用默认链路。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_provider_candidate_order(requested_provider: &str) -> Vec<String> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_provider_candidate_order",
        "business operation entered"
    );
    if requested_provider == "auto" {
        return DEFAULT_EXCHANGE_RATE_PROVIDER_ORDER
            .iter()
            .map(|provider| (*provider).to_string())
            .collect();
    }

    if !exchange_rate_provider_options().contains_key(requested_provider) {
        return Vec::new();
    }

    let mut order = vec![requested_provider.to_string()];
    for provider in DEFAULT_EXCHANGE_RATE_PROVIDER_ORDER {
        if provider != requested_provider {
            order.push(provider.to_string());
        }
    }
    order
}

/// 将用户自定义汇率列表组装为接口结果，并用有效日期推导更新时间。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_user_custom_exchange_rates_result(
    base_currency: &str,
    custom_rates: &[UserCustomExchangeRateInput],
    now_timestamp: i64,
) -> ExchangeRatesResult {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_user_custom_exchange_rates_result",
        "business operation entered"
    );
    let mut exchange_rates = vec![ExchangeRateItem {
        currency: base_currency.to_string(),
        rate: "1.0".to_string(),
    }];
    let mut update_time = now_timestamp;
    for rate in custom_rates {
        exchange_rates.push(ExchangeRateItem {
            currency: rate.to_currency.clone(),
            rate: rate.rate.clone(),
        });
        if let Some(effective) = rate
            .effective_timestamp
            .or_else(|| parse_effective_date_timestamp(rate.effective_date.as_deref()))
        {
            update_time = update_time.max(effective);
        }
    }

    ExchangeRatesResult {
        provider_key: "user_custom".to_string(),
        requested_provider: "auto".to_string(),
        fallback_used: false,
        data_source: "user_custom".to_string(),
        reference_url: String::new(),
        update_time,
        base_currency: base_currency.to_string(),
        exchange_rates,
    }
}

/// 将外部 provider 汇率表转换为统一接口结果，并标记是否发生 provider 回退。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_provider_exchange_rates_result(
    base_currency: &str,
    requested_provider: &str,
    provider_key: &str,
    rates: &BTreeMap<String, f64>,
    update_time: i64,
) -> ExchangeRatesResult {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_provider_exchange_rates_result",
        "business operation entered"
    );
    let option = exchange_rate_provider_options()
        .get(provider_key)
        .cloned()
        .unwrap_or_else(|| ExchangeRateProviderOption {
            label: provider_key.to_string(),
            reference_url: String::new(),
            region: String::new(),
        });
    let mut exchange_rates = vec![ExchangeRateItem {
        currency: base_currency.to_string(),
        rate: "1.0".to_string(),
    }];
    for (currency, rate) in rates {
        exchange_rates.push(ExchangeRateItem {
            currency: currency.clone(),
            rate: format_rate_text(*rate),
        });
    }

    ExchangeRatesResult {
        provider_key: provider_key.to_string(),
        requested_provider: requested_provider.to_string(),
        fallback_used: requested_provider != "auto" && requested_provider != provider_key,
        data_source: option.label,
        reference_url: option.reference_url,
        update_time,
        base_currency: base_currency.to_string(),
        exchange_rates,
    }
}

/// 构建内置 CNY 基准汇率回退结果，保证外部 provider 不可用时仍有可展示数据。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_builtin_fallback_exchange_rates(
    base_currency: &str,
    update_time: i64,
) -> ExchangeRatesResult {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_builtin_fallback_exchange_rates",
        "business operation entered"
    );
    let cny_rates = builtin_cny_based_rates();
    let mut exchange_rates = Vec::new();
    if base_currency == "CNY" {
        exchange_rates.push(ExchangeRateItem {
            currency: "CNY".to_string(),
            rate: "1.0".to_string(),
        });
        for (currency, rate) in cny_rates {
            exchange_rates.push(ExchangeRateItem {
                currency: currency.to_string(),
                rate: format_rate_text(rate),
            });
        }
    } else if let Some(base_rate) = builtin_cny_based_rates()
        .into_iter()
        .find_map(|(currency, rate)| (currency == base_currency).then_some(rate))
    {
        exchange_rates.push(ExchangeRateItem {
            currency: base_currency.to_string(),
            rate: "1.0".to_string(),
        });
        exchange_rates.push(ExchangeRateItem {
            currency: "CNY".to_string(),
            rate: format_rate_text(1.0 / base_rate),
        });
        for (currency, rate) in builtin_cny_based_rates() {
            if currency != base_currency {
                exchange_rates.push(ExchangeRateItem {
                    currency: currency.to_string(),
                    rate: format_rate_text(rate / base_rate),
                });
            }
        }
    }

    ExchangeRatesResult {
        provider_key: "fallback".to_string(),
        requested_provider: "auto".to_string(),
        fallback_used: true,
        data_source: "内置汇率数据 (回退)".to_string(),
        reference_url: String::new(),
        update_time,
        base_currency: base_currency.to_string(),
        exchange_rates,
    }
}

/// 将“中国外汇牌价每 100 外币兑人民币”的报价表转换为目标基准汇率。
#[tracing::instrument(level = "debug", skip_all)]
pub fn convert_cny_quote_map_to_rates(
    quote_map: &BTreeMap<String, f64>,
    base_currency: &str,
    target_currencies: &[String],
) -> BTreeMap<String, f64> {
    let mut result = BTreeMap::new();
    if base_currency == "CNY" {
        for currency in target_currencies {
            if let Some(quote) = quote_map
                .get(currency)
                .copied()
                .filter(|quote| *quote > 0.0)
            {
                result.insert(currency.clone(), 100.0 / quote);
            }
        }
        return result;
    }

    let Some(base_quote) = quote_map
        .get(base_currency)
        .copied()
        .filter(|quote| *quote > 0.0)
    else {
        return result;
    };

    for currency in target_currencies {
        if currency == "CNY" {
            result.insert(currency.clone(), base_quote / 100.0);
        } else if let Some(target_quote) = quote_map
            .get(currency)
            .copied()
            .filter(|quote| *quote > 0.0)
        {
            result.insert(currency.clone(), base_quote / target_quote);
        }
    }
    result
}

/// 将 provider 原始基准币汇率转换为目标基准币和目标币种集合。
#[tracing::instrument(level = "debug", skip_all)]
pub fn convert_provider_base_currency(
    rates: &BTreeMap<String, f64>,
    original_base: &str,
    target_base: &str,
    target_currencies: &[String],
    rate_format: &str,
) -> BTreeMap<String, f64> {
    if original_base == target_base {
        return target_currencies
            .iter()
            .filter_map(|currency| rates.get(currency).map(|rate| (currency.clone(), *rate)))
            .collect();
    }

    let Some(base_rate) = rates.get(target_base).copied().filter(|rate| *rate != 0.0) else {
        return BTreeMap::new();
    };

    let mut result = BTreeMap::new();
    for currency in target_currencies {
        if currency == target_base {
            result.insert(currency.clone(), 1.0);
        } else if currency == original_base {
            if rate_format == "target_to_base" {
                result.insert(currency.clone(), base_rate);
            } else {
                result.insert(currency.clone(), 1.0 / base_rate);
            }
        } else if let Some(rate) = rates.get(currency).copied() {
            if rate_format == "target_to_base" {
                result.insert(currency.clone(), base_rate / rate);
            } else {
                result.insert(currency.clone(), rate / base_rate);
            }
        }
    }
    result
}
