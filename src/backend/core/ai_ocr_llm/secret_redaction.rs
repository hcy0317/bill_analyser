use serde_json::{json, Map, Value};

pub(super) fn object_has_non_empty_secret(object: &Map<String, Value>) -> bool {
    object.iter().any(|(key, value)| {
        let current_key_has_secret = is_secret_key(key) && secret_value_present(value);
        current_key_has_secret || value_has_non_empty_secret(value)
    })
}

fn value_has_non_empty_secret(value: &Value) -> bool {
    match value {
        Value::Object(object) => object_has_non_empty_secret(object),
        Value::Array(items) => items.iter().any(value_has_non_empty_secret),
        _ => false,
    }
}

pub(super) fn redact_secrets_in_map(object: &mut Map<String, Value>) {
    for (key, value) in object.iter_mut() {
        if is_secret_key(key) {
            let replacement = if secret_value_present(value) {
                "********"
            } else {
                ""
            };
            *value = json!(replacement);
        } else {
            redact_secrets_in_value(value);
        }
    }
}

fn redact_secrets_in_value(value: &mut Value) {
    match value {
        Value::Object(object) => redact_secrets_in_map(object),
        Value::Array(items) => {
            for item in items {
                redact_secrets_in_value(item);
            }
        }
        _ => {}
    }
}

fn secret_value_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(object) => !object.is_empty(),
        _ => true,
    }
}

fn is_secret_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let exact_alias = matches!(
        normalized.as_str(),
        "apikey"
            | "authorization"
            | "xapikey"
            | "apisecret"
            | "secretkey"
            | "credential"
            | "credentials"
            | "proxyauthorization"
            | "subscriptionkey"
            | "ocpapimsubscriptionkey"
            | "accesstoken"
            | "refreshtoken"
            | "bearertoken"
            | "idtoken"
            | "privatekey"
            | "token"
            | "password"
            | "clientsecret"
    );
    if exact_alias {
        return true;
    }

    const SECRET_KEY_SUFFIXES: [&str; 15] = [
        "apikey",
        "xapikey",
        "apisecret",
        "secretkey",
        "authorizationheader",
        "proxyauthorization",
        "subscriptionkey",
        "accesstoken",
        "refreshtoken",
        "bearertoken",
        "idtoken",
        "privatekey",
        "password",
        "clientsecret",
        "credentials",
    ];

    SECRET_KEY_SUFFIXES
        .iter()
        .any(|suffix| normalized.len() > suffix.len() && normalized.ends_with(suffix))
}
