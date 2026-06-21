fn transfer_source_label(sources: &[TransferSourceSnapshot]) -> String {
    let outgoing = sources
        .iter()
        .find(|source| source.role == "outgoing")
        .or_else(|| sources.first());
    let incoming = sources
        .iter()
        .find(|source| source.role == "incoming")
        .or_else(|| sources.get(1));
    let outgoing_label = outgoing
        .map(|source| parser_source_label(&source.parser_id).into_owned())
        .unwrap_or_default();
    let incoming_label = incoming
        .map(|source| parser_source_label(&source.parser_id).into_owned())
        .unwrap_or_default();
    format!("匹配 | {outgoing_label} | {incoming_label}")
}

fn merge_preview_text(left: &str, right: &str) -> String {
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() {
        return right.to_string();
    }
    if right.is_empty() || left == right || left.contains(right) {
        return left.to_string();
    }
    if right.contains(left) {
        return right.to_string();
    }
    let mut values = Vec::new();
    for value in [left, right] {
        for part in value
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if !values.iter().any(|existing: &String| {
                existing == part || existing.contains(part) || part.contains(existing)
            }) {
                values.push(part.to_string());
            }
        }
    }
    values.join(" | ")
}
