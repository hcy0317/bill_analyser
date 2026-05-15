use super::*;
use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.000_001,
        "{actual} != {expected}"
    );
}

struct ProviderServer {
    addr: SocketAddr,
}

impl ProviderServer {
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

async fn spawn_provider_server() -> ProviderServer {
    let app = Router::new().fallback(any(provider_handler));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("provider listener binds");
    let addr = listener.local_addr().expect("provider addr");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("provider server serves");
    });

    ProviderServer { addr }
}

async fn provider_handler(request: Request) -> Response {
    match request.uri().path() {
        "/boc" => (
            StatusCode::OK,
            r#"<table><tr><td>美元</td><td>728.00</td><td>720.00</td><td>730.00</td></tr></table>"#,
        )
            .into_response(),
        "/cmb/api" => (StatusCode::OK, r#"{"body":[]}"#).into_response(),
        "/cmb/html" => (
            StatusCode::OK,
            r#"<table><tr><td>港币</td><td>91</td><td>92</td><td>93</td><td>94</td></tr></table>"#,
        )
            .into_response(),
        "/ecb" => (
            StatusCode::OK,
            r#"<Cube><Cube currency='USD' rate='1.10'/><Cube currency="CNY" rate="7.80"/></Cube>"#,
        )
            .into_response(),
        "/rba" => (
            StatusCode::OK,
            r#"<rss><item><cb:targetCurrency>USD</cb:targetCurrency><cb:value>0.66</cb:value></item></rss>"#,
        )
            .into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

#[test]
fn parses_boc_and_cmb_provider_quote_tables_without_network() {
    let boc_html = r#"
        <table>
          <tr><th>货币名称</th><th>现汇买入价</th><th>现钞买入价</th><th>卖出价</th></tr>
          <tr><td>美元</td><td>728.00</td><td>720.00</td><td>730.00</td></tr>
          <tr><td>人民币</td><td>100.00</td><td>100.00</td><td>100.00</td></tr>
          <tr><td>欧元</td><td>--</td><td>08:30</td><td>800.50</td></tr>
        </table>
    "#;
    let boc = parse_boc_quote_map(boc_html);
    assert_eq!(boc.len(), 2);
    assert_close(boc["USD"], 730.0);
    assert_close(boc["EUR"], 800.5);

    let cmb_html = r#"
        <table>
          <tr><td>美元</td><td>100</td><td>710</td><td>720</td><td>730</td><td>740</td></tr>
          <tr><td>港币</td><td>91</td><td>92</td><td>93</td><td>94</td></tr>
        </table>
    "#;
    let cmb = parse_cmb_quote_map_from_html(cmb_html);
    assert_close(cmb["USD"], 725.0);
    assert_close(cmb["HKD"], 92.5);
}

#[tokio::test]
async fn fetchers_parse_mock_provider_payloads_without_real_network() {
    let server = spawn_provider_server().await;
    let _ = TEST_PROVIDER_BASE_URL.set(server.url());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("client builds");
    let targets = vec!["USD".to_string(), "HKD".to_string(), "CNY".to_string()];

    let boc = fetch_exchange_rates_from_provider(&client, "boc_cn", "CNY", &targets)
        .await
        .expect("boc rates");
    assert_close(boc["USD"], 100.0 / 730.0);

    let cmb = fetch_exchange_rates_from_provider(&client, "cmb_cn", "CNY", &targets)
        .await
        .expect("cmb rates");
    assert_close(cmb["HKD"], 100.0 / 92.5);

    let ecb = fetch_exchange_rates_from_provider(&client, "ecb", "EUR", &targets)
        .await
        .expect("ecb rates");
    assert_close(ecb["USD"], 1.10);
    assert_close(ecb["CNY"], 7.80);

    let rba = fetch_exchange_rates_from_provider(&client, "rba", "AUD", &targets)
        .await
        .expect("rba rates");
    assert_close(rba["USD"], 0.66);

    let first_candidate = fetch_exchange_rates_from_providers("CNY", &targets, "auto")
        .await
        .expect("provider candidate");
    assert_eq!(first_candidate.0, "boc_cn");
    assert!(
        fetch_exchange_rates_from_provider(&client, "unknown", "CNY", &targets)
            .await
            .is_none()
    );
}

#[test]
fn parses_cmb_json_rows_and_currency_fallbacks() {
    let payload = json!({
        "body": [
            {
                "ccyNbrEng": "United States Dollar USD",
                "rthOfr": "730.00",
                "rthBid": "720.00",
                "rtcOfr": null,
                "rtcBid": "0",
                "rtbBid": "NaN"
            },
            {
                "ccyNbr": "欧元",
                "rthOfr": 800.0,
                "rthBid": "802.00"
            },
            {
                "ccyNbrEng": "Chinese Yuan CNY",
                "rthOfr": 100.0
            }
        ]
    });
    let quotes = parse_cmb_quote_map_from_api(&payload);
    assert_eq!(quotes.len(), 2);
    assert_close(quotes["USD"], 725.0);
    assert_close(quotes["EUR"], 801.0);
}

#[test]
fn parses_ecb_and_rba_xml_payloads_without_network() {
    let ecb = parse_ecb_base_rates(
        r#"<Cube><Cube currency='USD' rate='1.10'/><Cube currency="CNY" rate="7.80"/></Cube>"#,
    );
    assert_close(ecb["EUR"], 1.0);
    assert_close(ecb["USD"], 1.10);
    assert_close(ecb["CNY"], 7.80);

    let rba = parse_rba_base_rates(
        r#"<rss><item><cb:targetCurrency>USD</cb:targetCurrency><cb:value>0.66</cb:value></item><item><cb:targetCurrency>JPY</cb:targetCurrency><cb:value>95.5</cb:value></item></rss>"#,
    );
    assert_close(rba["AUD"], 1.0);
    assert_close(rba["USD"], 0.66);
    assert_close(rba["JPY"], 95.5);
}

#[test]
fn parser_helpers_reject_non_numeric_and_extract_xml_attributes() {
    assert_eq!(
        html_table_rows("<tr><td>A&amp;B</td><td><b>1,234.50</b></td><td>08:30</td></tr>"),
        vec![vec![
            "A&B".to_string(),
            "1,234.50".to_string(),
            "08:30".to_string()
        ]]
    );
    assert_eq!(
        numeric_values_from_cells(&[
            "1,234.50".to_string(),
            "08:30".to_string(),
            "--".to_string(),
            "text".to_string(),
            "0.75".to_string(),
        ]),
        vec![1234.5, 0.75]
    );
    assert_eq!(
        xml_attr(" currency='USD' rate=\"1.2\"", "currency"),
        Some("USD".to_string())
    );
    assert_eq!(
        between("x<cb:value>0.66</cb:value>y", "<cb:value>", "</cb:value>"),
        Some("0.66".to_string())
    );
    assert_close(
        json_number(&json!("1,234.50")).expect("numeric string"),
        1234.5,
    );
    assert!(json_number(&json!({"bad": true})).is_none());
}
