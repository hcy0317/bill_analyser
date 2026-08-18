pub(crate) fn redact_database_error(message: &str, database_url: Option<&str>) -> String {
    let Some(database_url) = database_url.filter(|value| !value.is_empty()) else {
        return message.to_string();
    };
    let mut redacted = message.replace(database_url, "[REDACTED_DATABASE_URL]");
    if let Some(authority) = database_url
        .split_once("://")
        .map(|(_, remainder)| remainder)
        .and_then(|remainder| remainder.split_once('@').map(|(authority, _)| authority))
    {
        if let Some(password) = authority.split_once(':').map(|(_, password)| password) {
            if !password.is_empty() {
                redacted = redacted.replace(password, "[REDACTED_DATABASE_PASSWORD]");
            }
        }
    }
    redacted
}
