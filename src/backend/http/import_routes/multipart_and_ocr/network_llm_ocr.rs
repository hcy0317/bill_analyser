use super::{
    multipart_ocr_provider_runtime::OcrProviderFailure,
    multipart_provider_auth_refresh::refresh_provider_auth_profile,
    normalize_ocr_llm_max_tokens, read_limited_llm_provider_body,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::{
    provider_auth_access_token, provider_auth_is_expired, validate_llm_vision_base_url,
    OcrConfigContract, OcrProviderTextResult, NETWORK_OCR_PROVIDER_NAME,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::time::Duration as StdDuration;

/// 调用网络 LLM vision OCR provider，包含 base_url 校验、auth 刷新、payload cap 和响应解析。
pub(super) async fn run_network_llm_ocr(
    config: &OcrConfigContract,
    image_bytes: Vec<u8>,
    mime: String,
) -> Result<OcrProviderTextResult, OcrProviderFailure> {
    let base_url = config.base_url.trim_end_matches('/');
    if base_url.is_empty() {
        return Err(OcrProviderFailure::unavailable(
            "OCR provider base URL is required",
        ));
    }
    validate_llm_vision_base_url(base_url)
        .map_err(|_| OcrProviderFailure::unavailable("OCR provider base URL is not allowed"))?;
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| OcrProviderFailure::unavailable("OCR provider unavailable"))?;
    let mut credential_config = config.credential_config.clone();
    if provider_auth_is_expired(&credential_config, Utc::now()) {
        credential_config = refresh_provider_auth_profile(&client, &credential_config, &config.base_url)
            .await
            .map_err(|_| {
                OcrProviderFailure::reauthentication_required(
                    "OCR provider authorization expired; sign in again or refresh credentials",
                )
            })?;
    }
    let Some(access_token) = provider_auth_access_token(&credential_config) else {
        return Err(OcrProviderFailure::reauthentication_required(
            "OCR provider authorization is missing; sign in again or refresh credentials",
        ));
    };
    let url = format!("{base_url}/chat/completions");
    let encoded = general_purpose::STANDARD.encode(&image_bytes);
    let model = if config.model.trim().is_empty() {
        "gpt-4o-mini".to_string()
    } else {
        config.model.clone()
    };
    let payload = json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "Extract all visible receipt or payment screenshot text. Return plain text only."},
                {"type": "image_url", "image_url": {"url": format!("data:{mime};base64,{encoded}")}}
            ]
        }],
        "temperature": config.parameters.get("temperature").and_then(Value::as_f64).unwrap_or(0.0),
        "max_tokens": normalize_ocr_llm_max_tokens(&config.parameters),
    });
    let response = client
        .post(&url)
        .header("authorization", format!("Bearer {access_token}"))
        .header("content-type", "application/json")
        .body(payload.to_string())
        .send()
        .await
        .map_err(|_| OcrProviderFailure::unavailable("OCR provider request failed"))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(OcrProviderFailure::reauthentication_required(
            "OCR provider authorization expired; sign in again or refresh credentials",
        ));
    }
    if !response.status().is_success() {
        return Err(OcrProviderFailure::unavailable(
            "OCR provider returned an error",
        ));
    }
    let raw_text = read_limited_llm_provider_body(response)
        .await
        .map_err(|_| OcrProviderFailure::invalid_output("OCR provider returned invalid response"))?;
    let raw_response = serde_json::from_str::<Value>(&raw_text)
        .map_err(|_| OcrProviderFailure::invalid_output("OCR provider returned invalid JSON"))?;
    let text = raw_response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(OcrProviderFailure::invalid_output(
            "OCR provider returned empty text",
        ));
    }
    Ok(OcrProviderTextResult {
        text,
        confidence: 0.8,
        model,
        raw_provider_response: json!({
            "engine": NETWORK_OCR_PROVIDER_NAME,
            "mime": mime,
            "provider": raw_response,
        }),
        lines: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex as AsyncMutex,
        task::JoinHandle,
    };

    static ALLOWLIST_ENV_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

    struct AllowlistEnvRestore(Option<String>);

    impl AllowlistEnvRestore {
        fn capture() -> Self {
            Self(env::var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST").ok())
        }
    }

    impl Drop for AllowlistEnvRestore {
        fn drop(&mut self) {
            if let Some(value) = &self.0 {
                env::set_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST", value);
            } else {
                env::remove_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST");
            }
        }
    }

    #[tokio::test]
    async fn allowlist_restore_reinstates_existing_value() {
        let _guard = ALLOWLIST_ENV_LOCK.lock().await;
        let original = AllowlistEnvRestore::capture();
        env::set_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST", "existing-value");

        let restore = AllowlistEnvRestore::capture();
        env::set_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST", "temporary-value");
        drop(restore);

        assert_eq!(
            env::var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST").as_deref(),
            Ok("existing-value")
        );
        drop(original);
    }

    fn config(base_url: String, credential_config: Value) -> OcrConfigContract {
        OcrConfigContract {
            provider: NETWORK_OCR_PROVIDER_NAME.to_string(),
            lang: "chi_sim+eng".to_string(),
            model: "vision-test".to_string(),
            base_url,
            parameters: json!({}),
            credential_config,
        }
    }

    async fn spawn_response(status: &str, body: &str) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock OCR provider");
        let address = listener.local_addr().expect("mock provider address");
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept OCR request");
            let mut request = vec![0_u8; 16 * 1024];
            let _ = stream.read(&mut request).await.expect("read OCR request");
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write OCR response");
        });
        (format!("http://{address}"), server)
    }

    #[tokio::test]
    async fn network_adapter_fails_closed_before_external_request() {
        let _lock = ALLOWLIST_ENV_LOCK.lock().await;
        let _restore = AllowlistEnvRestore::capture();
        let missing_url = run_network_llm_ocr(
            &config(String::new(), json!({"access_token": "token"})),
            vec![1],
            "image/png".to_string(),
        )
        .await
        .expect_err("missing base URL");
        assert_eq!(
            missing_url,
            OcrProviderFailure::unavailable("OCR provider base URL is required")
        );

        env::set_var(
            "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST",
            "http://127.0.0.1:9",
        );
        let missing_token = run_network_llm_ocr(
            &config("http://127.0.0.1:9".to_string(), json!({})),
            vec![1],
            "image/png".to_string(),
        )
        .await
        .expect_err("missing provider token");
        assert_eq!(
            missing_token,
            OcrProviderFailure::reauthentication_required(
                "OCR provider authorization is missing; sign in again or refresh credentials"
            )
        );
    }

    #[tokio::test]
    async fn network_adapter_classifies_http_and_output_failures() {
        let _lock = ALLOWLIST_ENV_LOCK.lock().await;
        let _restore = AllowlistEnvRestore::capture();
        let cases = [
            (
                "401 Unauthorized",
                "{}",
                OcrProviderFailure::reauthentication_required(
                    "OCR provider authorization expired; sign in again or refresh credentials",
                ),
            ),
            (
                "500 Internal Server Error",
                "{}",
                OcrProviderFailure::unavailable("OCR provider returned an error"),
            ),
            (
                "200 OK",
                "not-json",
                OcrProviderFailure::invalid_output("OCR provider returned invalid JSON"),
            ),
            (
                "200 OK",
                r#"{"choices":[{"message":{"content":" "}}]}"#,
                OcrProviderFailure::invalid_output("OCR provider returned empty text"),
            ),
        ];

        for (status, body, expected) in cases {
            let (base_url, server) = spawn_response(status, body).await;
            env::set_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST", &base_url);
            let failure = run_network_llm_ocr(
                &config(base_url, json!({"access_token": "token"})),
                vec![1, 2, 3],
                "image/png".to_string(),
            )
            .await
            .expect_err("mock provider failure");
            server.await.expect("mock provider task");
            assert_eq!(failure, expected);
        }
    }

    #[tokio::test]
    async fn network_adapter_returns_typed_text_observation() {
        let _lock = ALLOWLIST_ENV_LOCK.lock().await;
        let _restore = AllowlistEnvRestore::capture();
        let (base_url, server) = spawn_response(
            "200 OK",
            r#"{"choices":[{"message":{"content":"Coffee 12.34"}}]}"#,
        )
        .await;
        env::set_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST", &base_url);
        let result = run_network_llm_ocr(
            &config(base_url, json!({"access_token": "token"})),
            vec![1, 2, 3],
            "image/png".to_string(),
        )
        .await
        .expect("mock provider success");
        server.await.expect("mock provider task");

        assert_eq!(result.text, "Coffee 12.34");
        assert_eq!(result.model, "vision-test");
        assert_eq!(result.confidence, 0.8);
        assert_eq!(result.raw_provider_response["engine"], NETWORK_OCR_PROVIDER_NAME);
    }
}
