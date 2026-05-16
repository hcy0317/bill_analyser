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
