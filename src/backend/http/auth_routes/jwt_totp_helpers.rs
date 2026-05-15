fn invalid_login_credentials_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        401,
        "Invalid credentials",
        "Invalid username or password",
    ))
}

fn log_login_failure(
    connection: &rusqlite::Connection,
    user: &AuthLoginUserRow,
    ip_address: &str,
    user_agent: &str,
    error_message: &str,
) -> bill_analyser_db::DbResult<i64> {
    log_auth_event(
        connection,
        AuthEvent {
            user_id: Some(user.profile.id),
            username: &user.profile.username,
            event_type: "login_failed",
            ip_address,
            user_agent,
            success: false,
            error_message: Some(error_message.to_string()),
            metadata: None,
        },
    )
}

fn login_lock_is_active(locked_until: &str) -> bool {
    let value = locked_until.trim();
    if value.is_empty() {
        return false;
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"))
        .map(|datetime| Utc::now().naive_utc() < datetime)
        .unwrap_or(false)
}

struct IssuedAccessToken {
    access_token: String,
    expires_at: String,
}

struct IssuedSessionTokens {
    access_token: String,
    refresh_token: String,
    expires_at: String,
    refresh_expires_at: String,
}

fn validate_refresh_jwt(
    token: &str,
    state: &HttpAppState,
) -> RouteResult<bill_analyser_core::auth::RefreshTokenClaims> {
    let secret = state
        .config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            Box::new(auth_rest_error_response(AuthRestError::new(
                503,
                "Service Unavailable",
                "Rust auth token runtime requires BILL_ANALYSER_AUTH_JWT_SECRET or JWT_SECRET_KEY",
            )))
        })?;
    let mut parts = token.split('.');
    let encoded_header = parts.next().ok_or_else(invalid_refresh_token_box)?;
    let encoded_payload = parts.next().ok_or_else(invalid_refresh_token_box)?;
    let encoded_signature = parts.next().ok_or_else(invalid_refresh_token_box)?;
    if parts.next().is_some() {
        return Err(invalid_refresh_token_box());
    }

    let header = decode_jwt_part(encoded_header)?;
    let payload = decode_jwt_part(encoded_payload)?;
    let configured_algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let header_algorithm = normalize_jwt_algorithm(
        header
            .get("alg")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if header_algorithm != configured_algorithm {
        return Err(invalid_refresh_token_box());
    }
    let hmac_algorithm = jwt_hmac_algorithm(&configured_algorithm).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.message,
        )))
    })?;
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    verify_hmac_signature(
        secret,
        hmac_algorithm,
        signing_input.as_bytes(),
        encoded_signature,
    )?;
    let exp = payload
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or_else(invalid_refresh_token_box)?;
    if exp <= Utc::now().timestamp() {
        return Err(Box::new(refresh_token_expired_response()));
    }
    validate_refresh_token_claims(&payload)
        .map_err(|error| Box::new(auth_rest_error_response(error)))
}

fn validate_action_jwt(
    token: &str,
    state: &HttpAppState,
    expected_type: &str,
    invalid_message: &'static str,
) -> RouteResult<Value> {
    validate_action_jwt_with_invalid(token, state, expected_type, || {
        invalid_action_token_box(invalid_message)
    })
}

fn validate_pending_two_factor_jwt(token: &str, state: &HttpAppState) -> RouteResult<Value> {
    validate_action_jwt_with_invalid(token, state, "pending_2fa", invalid_pending_two_factor_box)
}

fn validate_action_jwt_with_invalid<F>(
    token: &str,
    state: &HttpAppState,
    expected_type: &str,
    invalid_response: F,
) -> RouteResult<Value>
where
    F: Fn() -> Box<Response> + Copy,
{
    let secret = state
        .config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            Box::new(auth_rest_error_response(AuthRestError::new(
                503,
                "Service Unavailable",
                "Rust auth token runtime requires BILL_ANALYSER_AUTH_JWT_SECRET or JWT_SECRET_KEY",
            )))
        })?;
    let mut parts = token.split('.');
    let encoded_header = parts.next().ok_or_else(invalid_response)?;
    let encoded_payload = parts.next().ok_or_else(invalid_response)?;
    let encoded_signature = parts.next().ok_or_else(invalid_response)?;
    if parts.next().is_some() {
        return Err(invalid_response());
    }

    let header = decode_action_jwt_part(encoded_header, invalid_response)?;
    let payload = decode_action_jwt_part(encoded_payload, invalid_response)?;
    let configured_algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let header_algorithm = normalize_jwt_algorithm(
        header
            .get("alg")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if header_algorithm != configured_algorithm {
        return Err(invalid_response());
    }
    let hmac_algorithm = jwt_hmac_algorithm(&configured_algorithm).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.message,
        )))
    })?;
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded_signature))
        .map_err(|_| invalid_response())?;
    let key = hmac::Key::new(hmac_algorithm, secret.as_bytes());
    hmac::verify(&key, signing_input.as_bytes(), &signature).map_err(|_| invalid_response())?;
    let exp = payload
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or_else(invalid_response)?;
    if exp <= Utc::now().timestamp() {
        return Err(invalid_response());
    }
    if payload.get("type").and_then(Value::as_str) != Some(expected_type) {
        return Err(invalid_response());
    }
    Ok(payload)
}

fn decode_action_jwt_part<F>(encoded: &str, invalid_response: F) -> RouteResult<Value>
where
    F: Fn() -> Box<Response> + Copy,
{
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| invalid_response())?;
    serde_json::from_slice(&decoded).map_err(|_| invalid_response())
}

fn action_user_id(payload: &Value, invalid_message: &'static str) -> RouteResult<UserId> {
    action_user_id_with_invalid(payload, || invalid_action_token_box(invalid_message))
}

fn pending_two_factor_user_id(payload: &Value) -> RouteResult<UserId> {
    action_user_id_with_invalid(payload, invalid_pending_two_factor_box)
}

fn action_user_id_with_invalid<F>(payload: &Value, invalid_response: F) -> RouteResult<UserId>
where
    F: Fn() -> Box<Response> + Copy,
{
    let raw_user_id = payload
        .get("user_id")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_response)?;
    UserId::new(raw_user_id).map_err(|_| invalid_response())
}

fn issue_session_tokens(
    user_id: UserId,
    username: &str,
    state: &HttpAppState,
) -> RouteResult<IssuedSessionTokens> {
    let now = Local::now();
    let access_expires_at = now + ChronoDuration::days(state.config.auth_jwt_expiration_days);
    let refresh_expires_at =
        now + ChronoDuration::days(state.config.auth_refresh_token_expiration_days);
    let access_payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "access",
        "iat": now.timestamp(),
        "exp": access_expires_at.timestamp(),
        "nonce": random_nonce_hex()?,
    });
    let refresh_payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "refresh",
        "iat": now.timestamp(),
        "exp": refresh_expires_at.timestamp(),
        "nonce": random_nonce_hex()?,
    });

    Ok(IssuedSessionTokens {
        access_token: sign_jwt(&access_payload, state)?,
        refresh_token: sign_jwt(&refresh_payload, state)?,
        expires_at: access_expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
        refresh_expires_at: refresh_expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
    })
}

fn issue_action_token(
    user: &AuthLoginUserRow,
    token_type: &str,
    expires_in_hours: i64,
    state: &HttpAppState,
) -> RouteResult<String> {
    let now = Local::now();
    let expires_at = now + ChronoDuration::hours(expires_in_hours);
    let payload = json!({
        "user_id": user.profile.id.get(),
        "username": user.profile.username.clone(),
        "email": user.profile.email.clone(),
        "type": token_type,
        "iat": now.timestamp(),
        "exp": expires_at.timestamp(),
        "nonce": random_nonce_hex()?,
    });
    sign_jwt(&payload, state)
}

fn issue_access_token(
    user_id: bill_analyser_core::UserId,
    username: &str,
    state: &HttpAppState,
    token_kind: TokenKind,
    expires_in_seconds: i64,
) -> RouteResult<IssuedAccessToken> {
    let now = Local::now();
    let expires_at = if expires_in_seconds > 0 {
        now + ChronoDuration::seconds(expires_in_seconds)
    } else {
        now + ChronoDuration::days(365 * 100)
    };
    let nonce = random_nonce_hex()?;
    let payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "access",
        "token_kind": token_kind.as_str(),
        "iat": now.timestamp(),
        "exp": expires_at.timestamp(),
        "nonce": nonce,
    });
    Ok(IssuedAccessToken {
        access_token: sign_jwt(&payload, state)?,
        expires_at: expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
    })
}

fn sign_jwt(payload: &Value, state: &HttpAppState) -> RouteResult<String> {
    let secret = state
        .config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            Box::new(auth_rest_error_response(AuthRestError::new(
                503,
                "Service Unavailable",
                "Rust auth token runtime requires BILL_ANALYSER_AUTH_JWT_SECRET or JWT_SECRET_KEY",
            )))
        })?;
    let algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let hmac_algorithm = jwt_hmac_algorithm(&algorithm).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.message,
        )))
    })?;
    let header = json!({ "alg": algorithm, "typ": "JWT" });
    let encoded_header = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&header).map_err(|_| Box::new(db_error_response()))?);
    let encoded_payload = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(payload).map_err(|_| Box::new(db_error_response()))?);
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac_algorithm, secret.as_bytes());
    let signature = hmac::sign(&key, signing_input.as_bytes());
    let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
    Ok(format!("{signing_input}.{encoded_signature}"))
}

fn decode_jwt_part(encoded: &str) -> RouteResult<Value> {
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| invalid_refresh_token_box())?;
    serde_json::from_slice(&decoded).map_err(|_| invalid_refresh_token_box())
}

fn verify_hmac_signature(
    secret: &str,
    algorithm: hmac::Algorithm,
    signing_input: &[u8],
    encoded_signature: &str,
) -> RouteResult<()> {
    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded_signature))
        .map_err(|_| invalid_refresh_token_box())?;
    let key = hmac::Key::new(algorithm, secret.as_bytes());
    hmac::verify(&key, signing_input, &signature).map_err(|_| invalid_refresh_token_box())
}

fn invalid_refresh_token_box() -> Box<Response> {
    Box::new(invalid_refresh_token_response())
}

fn invalid_action_token_box(message: &'static str) -> Box<Response> {
    Box::new(auth_rest_error_response(AuthRestError::new(
        400,
        "Invalid token",
        message,
    )))
}

fn invalid_pending_two_factor_box() -> Box<Response> {
    Box::new(auth_rest_error_response(AuthRestError::new(
        401,
        "Unauthorized",
        "Invalid or expired 2FA token",
    )))
}

fn invalid_refresh_token_response() -> Response {
    auth_rest_error_response(AuthRestError::invalid_token(401, "Invalid refresh token"))
}

fn refresh_token_expired_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        401,
        "Token expired",
        "Refresh token has expired",
    ))
}

fn refresh_session_is_expired(expires_at: &str) -> bool {
    let normalized = expires_at.trim();
    if normalized.is_empty() {
        return true;
    }
    NaiveDateTime::parse_from_str(normalized, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(normalized, "%Y-%m-%d %H:%M:%S%.f"))
        .map(|datetime| Local::now().naive_local() > datetime)
        .unwrap_or(true)
}

fn random_nonce_hex() -> RouteResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 16];
    rng.fill(&mut bytes).map_err(|_| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime random generation failed",
        )))
    })?;
    Ok(bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn random_base32_secret() -> RouteResult<String> {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 20];
    rng.fill(&mut bytes).map_err(|_| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust 2FA runtime random generation failed",
        )))
    })?;

    let mut buffer = 0_u32;
    let mut bit_count = 0_u8;
    let mut output = String::with_capacity(32);
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bit_count += 8;
        while bit_count >= 5 {
            bit_count -= 5;
            let index = ((buffer >> bit_count) & 0x1f) as usize;
            output.push(char::from(ALPHABET[index]));
        }
    }
    if bit_count > 0 {
        let index = ((buffer << (5 - bit_count)) & 0x1f) as usize;
        output.push(char::from(ALPHABET[index]));
    }
    Ok(output)
}

fn generate_two_factor_recovery_codes() -> RouteResult<Vec<String>> {
    let rng = SystemRandom::new();
    let mut codes = Vec::with_capacity(8);
    let mut seen = HashSet::new();
    while codes.len() < 8 {
        let mut bytes = [0_u8; 4];
        rng.fill(&mut bytes).map_err(|_| {
            Box::new(auth_rest_error_response(AuthRestError::new(
                500,
                "Internal Server Error",
                "Rust 2FA runtime random generation failed",
            )))
        })?;
        let code = format!(
            "{:02X}{:02X}-{:02X}{:02X}",
            bytes[0], bytes[1], bytes[2], bytes[3]
        );
        if seen.insert(code.clone()) {
            codes.push(code);
        }
    }
    Ok(codes)
}

fn two_factor_provisioning_uri(username: &str, secret: &str) -> String {
    let issuer = "Bill Analyser";
    let label = format!("{issuer}:{username}");
    format!(
        "otpauth://totp/{}?secret={}&issuer={}",
        percent_encode_otpauth_component(&label),
        secret,
        percent_encode_otpauth_component(issuer)
    )
}

fn qrcode_png_data_url(value: &str) -> RouteResult<String> {
    let qr = QrCode::encode_text(value, QrCodeEcc::Medium).map_err(|_| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            500,
            "Internal Server Error",
            "Failed to generate two-factor QR code",
        )))
    })?;
    let qr_size = qr.size();
    let scale = 4_i32;
    let border = 4_i32;
    let image_size = (qr_size + border * 2) * scale;
    let image_size_usize =
        usize::try_from(image_size).map_err(|_| Box::new(db_error_response()))?;
    let mut pixels = vec![255_u8; image_size_usize * image_size_usize];
    for y in 0..image_size {
        for x in 0..image_size {
            let module_x = x / scale - border;
            let module_y = y / scale - border;
            let dark = (0..qr_size).contains(&module_x)
                && (0..qr_size).contains(&module_y)
                && qr.get_module(module_x, module_y);
            if dark {
                let index = usize::try_from(y).map_err(|_| Box::new(db_error_response()))?
                    * image_size_usize
                    + usize::try_from(x).map_err(|_| Box::new(db_error_response()))?;
                pixels[index] = 0;
            }
        }
    }

    let png_bytes = encode_grayscale_png(
        u32::try_from(image_size).map_err(|_| Box::new(db_error_response()))?,
        u32::try_from(image_size).map_err(|_| Box::new(db_error_response()))?,
        &pixels,
    )?;

    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(png_bytes)
    ))
}

fn encode_grayscale_png(width: u32, height: u32, pixels: &[u8]) -> RouteResult<Vec<u8>> {
    let width_usize = usize::try_from(width).map_err(|_| Box::new(db_error_response()))?;
    let height_usize = usize::try_from(height).map_err(|_| Box::new(db_error_response()))?;
    let expected_len = width_usize
        .checked_mul(height_usize)
        .ok_or_else(|| Box::new(db_error_response()))?;
    if pixels.len() != expected_len {
        return Err(Box::new(db_error_response()));
    }

    let mut image_data = Vec::with_capacity(
        height_usize
            .checked_mul(width_usize + 1)
            .ok_or_else(|| Box::new(db_error_response()))?,
    );
    for row in pixels.chunks(width_usize) {
        image_data.push(0);
        image_data.extend_from_slice(row);
    }

    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1A\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8);
    ihdr.push(0);
    ihdr.push(0);
    ihdr.push(0);
    ihdr.push(0);
    append_png_chunk(&mut png, *b"IHDR", &ihdr);
    append_png_chunk(&mut png, *b"IDAT", &zlib_store_blocks(&image_data));
    append_png_chunk(&mut png, *b"IEND", &[]);
    Ok(png)
}

fn zlib_store_blocks(data: &[u8]) -> Vec<u8> {
    let mut output = vec![0x78, 0x01];
    for (index, chunk) in data.chunks(u16::MAX as usize).enumerate() {
        let final_block = index == data.len().saturating_sub(1) / (u16::MAX as usize);
        output.push(if final_block { 0x01 } else { 0x00 });
        let length = chunk.len() as u16;
        output.extend_from_slice(&length.to_le_bytes());
        output.extend_from_slice(&(!length).to_le_bytes());
        output.extend_from_slice(chunk);
    }
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

fn append_png_chunk(output: &mut Vec<u8>, chunk_type: [u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(&chunk_type);
    output.extend_from_slice(data);
    let crc = png_crc32(&chunk_type, data);
    output.extend_from_slice(&crc.to_be_bytes());
}

fn png_crc32(chunk_type: &[u8; 4], data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in chunk_type.iter().chain(data.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a = 1_u32;
    let mut b = 0_u32;
    for byte in data {
        a = (a + u32::from(*byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

fn percent_encode_otpauth_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn verify_totp_passcode(secret: &str, passcode: &str, timestamp: i64) -> bool {
    let passcode = passcode.trim();
    if passcode.is_empty() {
        return false;
    }
    let counter = timestamp.max(0) / 30;
    (-1_i64..=1).any(|offset| {
        let Some(candidate_counter) = counter.checked_add(offset) else {
            return false;
        };
        if candidate_counter < 0 {
            return false;
        }
        totp_passcode(secret, candidate_counter as u64)
            .is_some_and(|candidate| constant_time_eq(candidate.as_bytes(), passcode.as_bytes()))
    })
}

fn totp_passcode(secret: &str, counter: u64) -> Option<String> {
    let key_bytes = base32_decode_secret(secret)?;
    if key_bytes.is_empty() {
        return None;
    }
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, &key_bytes);
    let digest = hmac::sign(&key, &counter.to_be_bytes());
    let bytes = digest.as_ref();
    let offset = usize::from(bytes[bytes.len() - 1] & 0x0f);
    let binary = ((u32::from(bytes[offset]) & 0x7f) << 24)
        | (u32::from(bytes[offset + 1]) << 16)
        | (u32::from(bytes[offset + 2]) << 8)
        | u32::from(bytes[offset + 3]);
    Some(format!("{:06}", binary % 1_000_000))
}

fn base32_decode_secret(secret: &str) -> Option<Vec<u8>> {
    let mut buffer = 0_u32;
    let mut bit_count = 0_u8;
    let mut output = Vec::new();
    for byte in secret.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a',
            b'2'..=b'7' => byte - b'2' + 26,
            b'=' => continue,
            _ => return None,
        };
        buffer = (buffer << 5) | u32::from(value);
        bit_count += 5;
        while bit_count >= 8 {
            bit_count -= 8;
            output.push(((buffer >> bit_count) & 0xff) as u8);
        }
    }
    Some(output)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (left_byte, right_byte) in left.iter().zip(right.iter()) {
        diff |= left_byte ^ right_byte;
    }
    diff == 0
}

fn authenticated_user(headers: &HeaderMap, state: &HttpAppState) -> RouteResult<AuthenticatedUser> {
    resolve_authenticated_user_from_headers(headers, &state.config, TRUSTED_USER_SECRET_HEADER)
        .map_err(|error| Box::new(auth_error_response(error)))
}

fn verify_sensitive_operation_password_with_policy(
    connection: &rusqlite::Connection,
    user: &AuthLoginUserRow,
    password: &str,
    operation_password_policy: OperationPasswordPolicy,
) -> bill_analyser_db::DbResult<bool> {
    if password.is_empty() {
        return Ok(false);
    }
    if bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
        return Ok(true);
    }
    verify_operation_password(connection, password, operation_password_policy)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationPasswordPolicy {
    AllowUnset,
    RequireConfigured,
}

fn verify_operation_password(
    connection: &rusqlite::Connection,
    password: &str,
    operation_password_policy: OperationPasswordPolicy,
) -> bill_analyser_db::DbResult<bool> {
    if let Some(env_password) = std::env::var("BILL_ANALYSER_OPERATION_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Ok(password == env_password);
    }

    init_app_settings_schema(connection)?;
    let stored_password = get_app_setting(connection, "operation_password")?;
    Ok(
        match stored_password.as_deref().filter(|value| !value.is_empty()) {
            Some(value) => password == value,
            None => operation_password_policy == OperationPasswordPolicy::AllowUnset,
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SensitiveTwoFactorAuthMode {
    Password,
    StepUp,
}

impl SensitiveTwoFactorAuthMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::StepUp => "step_up",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SensitiveTwoFactorAuthError {
    Missing,
    Invalid,
    Db,
}

fn resolve_sensitive_two_factor_auth(
    connection: &rusqlite::Connection,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    resolve_sensitive_two_factor_auth_with_policy(
        connection,
        body,
        state,
        user,
        OperationPasswordPolicy::AllowUnset,
    )
}

fn resolve_destructive_user_data_auth(
    connection: &rusqlite::Connection,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    resolve_sensitive_two_factor_auth_with_policy(
        connection,
        body,
        state,
        user,
        OperationPasswordPolicy::RequireConfigured,
    )
}

fn resolve_sensitive_two_factor_auth_with_policy(
    connection: &rusqlite::Connection,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
    operation_password_policy: OperationPasswordPolicy,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    let step_up_token = body
        .get("stepUpToken")
        .or_else(|| body.get("step_up_token"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if !step_up_token.is_empty() {
        let payload = validate_action_jwt(step_up_token, state, "step_up", "Invalid step-up token")
            .map_err(|_| SensitiveTwoFactorAuthError::Invalid)?;
        let token_user_id = payload
            .get("user_id")
            .and_then(Value::as_u64)
            .ok_or(SensitiveTwoFactorAuthError::Invalid)?;
        if token_user_id == user.profile.id.get() {
            return Ok(SensitiveTwoFactorAuthMode::StepUp);
        }
        return Err(SensitiveTwoFactorAuthError::Invalid);
    }

    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if password.is_empty() {
        return Err(SensitiveTwoFactorAuthError::Missing);
    }
    match verify_sensitive_operation_password_with_policy(
        connection,
        user,
        password,
        operation_password_policy,
    ) {
        Ok(true) => Ok(SensitiveTwoFactorAuthMode::Password),
        Ok(false) => Err(SensitiveTwoFactorAuthError::Invalid),
        Err(_) => Err(SensitiveTwoFactorAuthError::Db),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserDataExportType {
    Csv,
    Tsv,
}
