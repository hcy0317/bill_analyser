use std::{collections::BTreeMap, time::Duration};

use bill_analyser_core::statistics::{
    build_provider_candidate_order, convert_cny_quote_map_to_rates, convert_provider_base_currency,
    normalize_chinese_currency_name, TARGET_EXCHANGE_CURRENCIES,
};
use serde_json::Value;

#[cfg(test)]
static TEST_PROVIDER_BASE_URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub(super) fn target_exchange_currencies(base_currency: &str) -> Vec<String> {
    TARGET_EXCHANGE_CURRENCIES
        .iter()
        .copied()
        .filter(|currency| *currency != base_currency)
        .map(str::to_string)
        .collect()
}

pub(super) async fn fetch_exchange_rates_from_providers(
    base_currency: &str,
    target_currencies: &[String],
    requested_provider: &str,
) -> Option<(String, BTreeMap<String, f64>)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .ok()?;
    for provider_key in build_provider_candidate_order(requested_provider) {
        let rates = fetch_exchange_rates_from_provider(
            &client,
            &provider_key,
            base_currency,
            target_currencies,
        )
        .await;
        if let Some(rates) = rates.filter(|rates| !rates.is_empty()) {
            return Some((provider_key, rates));
        }
    }
    None
}

async fn fetch_exchange_rates_from_provider(
    client: &reqwest::Client,
    provider_key: &str,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    match provider_key {
        "boc_cn" => fetch_boc_china_rates(client, base_currency, target_currencies).await,
        "cmb_cn" => fetch_cmb_china_rates(client, base_currency, target_currencies).await,
        "ecb" => fetch_ecb_rates(client, base_currency, target_currencies).await,
        "rba" => fetch_rba_rates(client, base_currency, target_currencies).await,
        _ => None,
    }
}

async fn fetch_boc_china_rates(
    client: &reqwest::Client,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    let html = fetch_text(
        client,
        &provider_url("https://www.boc.cn/sourcedb/whpj/", "/boc"),
    )
    .await?;
    let quote_map = parse_boc_quote_map(&html);
    Some(convert_cny_quote_map_to_rates(
        &quote_map,
        base_currency,
        target_currencies,
    ))
}

async fn fetch_cmb_china_rates(
    client: &reqwest::Client,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    let quote_map = if let Some(payload) = fetch_text(
        client,
        &provider_url("https://fx.cmbchina.com/api/v1/fx/rate", "/cmb/api"),
    )
    .await
    {
        serde_json::from_str::<Value>(&payload)
            .ok()
            .map(|value| parse_cmb_quote_map_from_api(&value))
            .unwrap_or_default()
    } else {
        BTreeMap::new()
    };
    let quote_map = if quote_map.is_empty() {
        fetch_text(
            client,
            &provider_url("https://fx.cmbchina.com/hq/", "/cmb/html"),
        )
        .await
        .map(|html| parse_cmb_quote_map_from_html(&html))
        .unwrap_or_default()
    } else {
        quote_map
    };
    Some(convert_cny_quote_map_to_rates(
        &quote_map,
        base_currency,
        target_currencies,
    ))
}

async fn fetch_ecb_rates(
    client: &reqwest::Client,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    let xml = fetch_text(
        client,
        &provider_url(
            "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml",
            "/ecb",
        ),
    )
    .await?;
    let rates = parse_ecb_base_rates(&xml);
    Some(convert_provider_base_currency(
        &rates,
        "EUR",
        base_currency,
        target_currencies,
        "base_to_target",
    ))
}

async fn fetch_rba_rates(
    client: &reqwest::Client,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    let xml = fetch_text(
        client,
        &provider_url(
            "https://www.rba.gov.au/rss/rss-cb-exchange-rates.xml",
            "/rba",
        ),
    )
    .await?;
    let rates = parse_rba_base_rates(&xml);
    Some(convert_provider_base_currency(
        &rates,
        "AUD",
        base_currency,
        target_currencies,
        "base_to_target",
    ))
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Option<String> {
    client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()
}

fn provider_url(default_url: &str, _test_path: &str) -> String {
    #[cfg(test)]
    if let Some(base_url) = TEST_PROVIDER_BASE_URL.get() {
        return format!("{base_url}{_test_path}");
    }

    default_url.to_string()
}

fn parse_boc_quote_map(html: &str) -> BTreeMap<String, f64> {
    let mut quote_map = BTreeMap::new();
    for cells in html_table_rows(html) {
        let Some(currency) = cells
            .first()
            .map(|value| normalize_chinese_currency_name(value))
        else {
            continue;
        };
        if currency.is_empty() || currency == "CNY" {
            continue;
        }
        let numeric_values = numeric_values_from_cells(&cells[1..]);
        if let Some(quote) = numeric_values.last().copied().filter(|value| *value > 0.0) {
            quote_map.insert(currency, quote);
        }
    }
    quote_map
}

fn parse_cmb_quote_map_from_api(payload: &Value) -> BTreeMap<String, f64> {
    let mut quote_map = BTreeMap::new();
    let Some(rows) = payload.get("body").and_then(Value::as_array) else {
        return quote_map;
    };
    for row in rows {
        let currency = cmb_currency_from_api_row(row);
        if currency.is_empty() || currency == "CNY" {
            continue;
        }
        let quotes = ["rthOfr", "rthBid", "rtcOfr", "rtcBid", "rtbBid"]
            .iter()
            .filter_map(|field| row.get(*field).and_then(json_number))
            .filter(|value| *value > 0.0)
            .collect::<Vec<_>>();
        if !quotes.is_empty() {
            quote_map.insert(currency, quotes.iter().sum::<f64>() / quotes.len() as f64);
        }
    }
    quote_map
}

fn parse_cmb_quote_map_from_html(html: &str) -> BTreeMap<String, f64> {
    let mut quote_map = BTreeMap::new();
    for cells in html_table_rows(html) {
        let Some(currency) = cells
            .first()
            .map(|value| normalize_chinese_currency_name(value))
        else {
            continue;
        };
        if currency.is_empty() || currency == "CNY" {
            continue;
        }
        let numeric_values = numeric_values_from_cells(&cells[1..]);
        let quote_candidates =
            if numeric_values.len() >= 5 && (numeric_values[0] - 100.0).abs() < 0.001 {
                numeric_values[1..5].to_vec()
            } else {
                numeric_values.iter().copied().take(4).collect()
            };
        let valid_quotes = quote_candidates
            .into_iter()
            .filter(|value| *value > 0.0)
            .collect::<Vec<_>>();
        if !valid_quotes.is_empty() {
            quote_map.insert(
                currency,
                valid_quotes.iter().sum::<f64>() / valid_quotes.len() as f64,
            );
        }
    }
    quote_map
}

fn parse_ecb_base_rates(xml: &str) -> BTreeMap<String, f64> {
    let mut rates = BTreeMap::from([("EUR".to_string(), 1.0)]);
    for fragment in xml.split("<Cube").skip(1) {
        let Some(currency) = xml_attr(fragment, "currency") else {
            continue;
        };
        let Some(rate) = xml_attr(fragment, "rate").and_then(|value| value.parse::<f64>().ok())
        else {
            continue;
        };
        rates.insert(currency, rate);
    }
    rates
}

fn parse_rba_base_rates(xml: &str) -> BTreeMap<String, f64> {
    let mut rates = BTreeMap::from([("AUD".to_string(), 1.0)]);
    for item in xml.split("<item").skip(1) {
        let Some(currency) = between(item, "<cb:targetCurrency>", "</cb:targetCurrency>") else {
            continue;
        };
        let Some(rate) =
            between(item, "<cb:value>", "</cb:value>").and_then(|value| value.parse::<f64>().ok())
        else {
            continue;
        };
        rates.insert(currency, rate);
    }
    rates
}

fn cmb_currency_from_api_row(row: &Value) -> String {
    if let Some(text) = row.get("ccyNbrEng").and_then(Value::as_str) {
        for token in text.split_whitespace().rev() {
            let token = token.trim();
            if token.len() == 3 && token.chars().all(|ch| ch.is_ascii_uppercase()) {
                return token.to_string();
            }
        }
    }
    row.get("ccyNbr")
        .and_then(Value::as_str)
        .map(normalize_chinese_currency_name)
        .unwrap_or_default()
}

fn html_table_rows(html: &str) -> Vec<Vec<String>> {
    html.split("<tr")
        .skip(1)
        .filter_map(|row| row.split("</tr>").next())
        .map(|row| {
            let mut cells = html_cells(row, "td");
            if cells.is_empty() {
                cells = html_cells(row, "th");
            }
            cells
        })
        .filter(|cells| !cells.is_empty())
        .collect()
}

fn html_cells(row: &str, tag: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut rest = row;
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    while let Some(start) = rest.find(&open) {
        rest = &rest[start + open.len()..];
        let Some(content_start) = rest.find('>') else {
            break;
        };
        rest = &rest[content_start + 1..];
        let Some(content_end) = rest.find(&close) else {
            break;
        };
        cells.push(clean_html_cell(&rest[..content_end]));
        rest = &rest[content_end + close.len()..];
    }
    cells
}

fn clean_html_cell(value: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

fn numeric_values_from_cells(cells: &[String]) -> Vec<f64> {
    cells
        .iter()
        .filter_map(|value| {
            let text = value.trim();
            if text.is_empty() || matches!(text, "-" | "--" | "nan" | "NaN") || text.contains(':') {
                return None;
            }
            let normalized = text.replace(',', "");
            if normalized
                .chars()
                .all(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '.'))
            {
                normalized.parse::<f64>().ok()
            } else {
                None
            }
        })
        .collect()
}

fn xml_attr(fragment: &str, attr: &str) -> Option<String> {
    let double = format!("{attr}=\"");
    let single = format!("{attr}='");
    if let Some(start) = fragment.find(&double) {
        let value = &fragment[start + double.len()..];
        return value.split('"').next().map(ToString::to_string);
    }
    let start = fragment.find(&single)?;
    let value = &fragment[start + single.len()..];
    value.split('\'').next().map(ToString::to_string)
}

fn between(value: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    let start = value.find(start_marker)? + start_marker.len();
    let rest = &value[start..];
    let end = rest.find(end_marker)?;
    Some(rest[..end].trim().to_string())
}

pub(super) fn json_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.replace(',', "").trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../../../tests/backend/http/internal/statistics_exchange_providers.rs"]
mod tests;
