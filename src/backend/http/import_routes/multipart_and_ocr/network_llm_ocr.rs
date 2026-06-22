use super::*;

pub(super) async fn run_network_llm_ocr(
    config: &OcrConfigContract,
    image_bytes: Vec<u8>,
    mime: String,
) -> Result<OcrProviderTextResult, AiRouteResponse> {
    let base_url = config.base_url.trim_end_matches('/');
    if base_url.is_empty() {
        return Err(build_ocr_error_response(
            "provider_unconfigured",
            Some("OCR provider base URL is required"),
        ));
    }
    validate_llm_vision_base_url(base_url).map_err(|_| {
        build_ocr_error_response(
            "provider_unconfigured",
            Some("OCR provider base URL is not allowed"),
        )
    })?;
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| build_ocr_error_response("provider_unconfigured", Some("OCR provider unavailable")))?;
    let mut credential_config = config.credential_config.clone();
    if provider_auth_is_expired(&credential_config, Utc::now()) {
        credential_config = refresh_provider_auth_profile(&client, &credential_config, &config.base_url)
            .await
            .map_err(|_| {
                build_ocr_error_response(
                    "provider_relogin_required",
                    Some("OCR provider authorization expired; sign in again or refresh credentials"),
                )
            })?;
    }
    let Some(access_token) = provider_auth_access_token(&credential_config) else {
        return Err(build_ocr_error_response(
            "provider_relogin_required",
            Some("OCR provider authorization is missing; sign in again or refresh credentials"),
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
        .map_err(|_| build_ocr_error_response("provider_unconfigured", Some("OCR provider request failed")))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(build_ocr_error_response(
            "provider_relogin_required",
            Some("OCR provider authorization expired; sign in again or refresh credentials"),
        ));
    }
    if !response.status().is_success() {
        return Err(build_ocr_error_response(
            "provider_unconfigured",
            Some("OCR provider returned an error"),
        ));
    }
    let raw_text = read_limited_llm_provider_body(response)
        .await
        .map_err(|_| build_ocr_error_response("parse_error", Some("OCR provider returned invalid response")))?;
    let raw_response = serde_json::from_str::<Value>(&raw_text)
        .map_err(|_| build_ocr_error_response("parse_error", Some("OCR provider returned invalid JSON")))?;
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
        return Err(build_ocr_error_response(
            "parse_error",
            Some("OCR provider returned empty text"),
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
