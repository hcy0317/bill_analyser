async fn upload_transaction_picture_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> Response {
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return *response;
    }

    loop {
        let next_field = match multipart.next_field().await {
            Ok(value) => value,
            Err(error) => {
                return route_contract_response(transaction_picture_internal_error_response(
                    error.to_string(),
                ));
            }
        };
        let Some(field) = next_field else {
            return route_contract_response(missing_transaction_picture_file_response());
        };
        if field.name() != Some("picture") {
            continue;
        }

        let Some(filename) = field
            .file_name()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
        else {
            return route_contract_response(invalid_transaction_picture_file_response());
        };
        if !is_allowed_transaction_picture_filename(&filename) {
            return route_contract_response(unsupported_transaction_picture_type_response());
        }

        let picture_bytes = match field.bytes().await {
            Ok(value) => value,
            Err(error) => {
                return route_contract_response(transaction_picture_internal_error_response(
                    error.to_string(),
                ));
            }
        };
        let picture_uuid = match random_picture_uuid_hex() {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let picture_id = match transaction_picture_upload_id(&picture_uuid, &filename) {
            Ok(value) => value,
            Err(error) => return bad_request(error.to_string()),
        };
        let upload_root = FsPath::new(&state.config.uploads_dir);
        if let Err(error) = fs::create_dir_all(upload_root) {
            return route_contract_response(transaction_picture_internal_error_response(
                error.to_string(),
            ));
        }
        let file_path = upload_root.join(&picture_id);
        if let Err(error) = fs::write(&file_path, picture_bytes.as_ref()) {
            return route_contract_response(transaction_picture_internal_error_response(
                error.to_string(),
            ));
        }

        let encoded = general_purpose::STANDARD.encode(picture_bytes.as_ref());
        let original_url = transaction_picture_data_url_from_base64(&picture_id, encoded);
        return route_contract_response(transaction_picture_upload_success_response(
            picture_id,
            original_url,
        ));
    }
}

async fn remove_unused_transaction_picture_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return *response;
    }

    let payload = if body.is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_slice(&body).unwrap_or_else(|_| Value::Object(Map::new()))
    };
    let Some(picture_id) = picture_id_from_payload(&payload) else {
        return route_contract_response(missing_unused_transaction_picture_id_response());
    };

    let file_path =
        transaction_picture_delete_path(FsPath::new(&state.config.uploads_dir), &picture_id);
    if file_path.is_file() {
        if let Err(error) = fs::remove_file(&file_path) {
            return route_contract_response(transaction_picture_internal_error_response(
                error.to_string(),
            ));
        }
    }

    route_contract_response(remove_unused_transaction_picture_success_response())
}

fn random_picture_uuid_hex() -> RouteResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 16];
    rng.fill(&mut bytes).map_err(|_| {
        Box::new(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Rust bills picture runtime random generation failed",
        ))
    })?;

    let mut hex = String::with_capacity(32);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(hex)
}

fn picture_id_from_payload(payload: &Value) -> Option<String> {
    let value = payload.as_object()?.get("id")?;
    let raw_value = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    };
    let picture_id = raw_value.trim().to_string();
    (!picture_id.is_empty()).then_some(picture_id)
}

#[cfg(test)]
mod picture_helper_tests {
    use super::*;

    #[test]
    fn picture_helpers_cover_payload_and_uuid_edges() {
        let picture_id = random_picture_uuid_hex().expect("uuid hex");
        assert_eq!(picture_id.len(), 32);
        assert!(picture_id.chars().all(|value| value.is_ascii_hexdigit()));

        assert_eq!(
            picture_id_from_payload(&json!({"id": " receipt.png "})),
            Some("receipt.png".to_string())
        );
        assert_eq!(
            picture_id_from_payload(&json!({"id": false})),
            Some("false".to_string())
        );
        assert_eq!(picture_id_from_payload(&json!({"id": {}})), None);
        assert_eq!(picture_id_from_payload(&json!({})), None);
        assert_eq!(picture_id_from_payload(&Value::Null), None);
    }
}
