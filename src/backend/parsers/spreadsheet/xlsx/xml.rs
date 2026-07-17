use super::super::SpreadsheetValidationError;

pub(super) fn find_start_tag(
    xml: &str,
    tag_name: &str,
    from: usize,
) -> Option<(usize, usize, bool)> {
    let marker = format!("<{tag_name}");
    let mut cursor = from.min(xml.len());
    loop {
        let relative = xml.get(cursor..)?.find(&marker)?;
        let start = cursor.checked_add(relative)?;
        let boundary = xml
            .as_bytes()
            .get(start.checked_add(marker.len())?)
            .copied()?;
        if matches!(boundary, b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/') {
            let end = xml.get(start..)?.find('>')?.checked_add(start)?;
            return Some((start, end, xml.get(start..end)?.trim_end().ends_with('/')));
        }
        cursor = start.checked_add(marker.len())?;
    }
}

pub(super) fn first_text_node(
    xml: &str,
    tag_name: &str,
) -> Result<Option<String>, SpreadsheetValidationError> {
    let Some((_, tag_end, self_closing)) = find_start_tag(xml, tag_name, 0) else {
        return Ok(None);
    };
    if self_closing {
        return Ok(Some(String::new()));
    }
    let close_marker = format!("</{tag_name}>");
    let body_start = tag_end.saturating_add(1);
    let close = xml[body_start..]
        .find(&close_marker)
        .map(|offset| body_start.saturating_add(offset))
        .ok_or_else(xml_invalid)?;
    decode_text(&xml[body_start..close]).map(Some)
}

pub(super) fn collect_text_nodes(xml: &str) -> Result<String, SpreadsheetValidationError> {
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some((_, tag_end, self_closing)) = find_start_tag(xml, "t", cursor) {
        if self_closing {
            cursor = tag_end.saturating_add(1);
            continue;
        }
        let body_start = tag_end.saturating_add(1);
        let close = xml[body_start..]
            .find("</t>")
            .map(|offset| body_start.saturating_add(offset))
            .ok_or_else(|| {
                SpreadsheetValidationError::invalid("Import preview spreadsheet text is invalid")
            })?;
        output.push_str(&decode_text(&xml[body_start..close])?);
        cursor = close.saturating_add(4);
    }
    Ok(output)
}

fn decode_text(value: &str) -> Result<String, SpreadsheetValidationError> {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('&') {
        output.push_str(&rest[..start]);
        let tail = &rest[start.saturating_add(1)..];
        let end = tail.find(';').ok_or_else(entity_invalid)?;
        let entity = &tail[..end];
        let decoded = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                char::from_u32(u32::from_str_radix(&entity[2..], 16).map_err(|_| entity_invalid())?)
                    .ok_or_else(entity_invalid)?
            }
            _ if entity.starts_with('#') => {
                char::from_u32(entity[1..].parse::<u32>().map_err(|_| entity_invalid())?)
                    .ok_or_else(entity_invalid)?
            }
            _ => return Err(entity_invalid()),
        };
        output.push(decoded);
        rest = &tail[end.saturating_add(1)..];
    }
    output.push_str(rest);
    Ok(output)
}

pub(super) fn xml_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let marker = format!("{name}={quote}");
        let mut cursor = 0usize;
        while let Some(relative) = tag.get(cursor..)?.find(&marker) {
            let marker_start = cursor.checked_add(relative)?;
            let has_attribute_boundary = marker_start == 0
                || tag
                    .as_bytes()
                    .get(marker_start.saturating_sub(1))
                    .is_some_and(u8::is_ascii_whitespace)
                || tag.as_bytes().get(marker_start.saturating_sub(1)) == Some(&b'<');
            if !has_attribute_boundary {
                cursor = marker_start.checked_add(marker.len())?;
                continue;
            }
            let start = marker_start.saturating_add(marker.len());
            let end = tag[start..].find(quote)?.saturating_add(start);
            return tag.get(start..end);
        }
    }
    None
}

pub(super) fn parse_cell_reference(value: &str) -> Option<(usize, usize)> {
    let value = value.trim().trim_matches('$');
    let split = value.find(|character: char| character.is_ascii_digit())?;
    let (column_text, row_text) = value.split_at(split);
    if column_text.is_empty()
        || row_text.is_empty()
        || !column_text
            .chars()
            .all(|character| character.is_ascii_alphabetic())
        || !row_text.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let column = column_text.chars().try_fold(0usize, |value, character| {
        value
            .checked_mul(26)?
            .checked_add((character.to_ascii_uppercase() as u8 - b'A' + 1) as usize)
    })?;
    let row = row_text.parse::<usize>().ok()?;
    (row > 0 && column > 0).then_some((row, column))
}

fn xml_invalid() -> SpreadsheetValidationError {
    SpreadsheetValidationError::invalid("Import preview spreadsheet XML is invalid")
}

fn entity_invalid() -> SpreadsheetValidationError {
    SpreadsheetValidationError::invalid("Import preview spreadsheet XML entity is invalid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_references_and_xml_entities() {
        assert_eq!(parse_cell_reference("aa12"), Some((12, 27)));
        assert_eq!(
            parse_cell_reference("XFD1048576"),
            Some((1_048_576, 16_384))
        );
        for invalid in ["", "1A", "A0", "A", "1", "A-1"] {
            assert_eq!(parse_cell_reference(invalid), None, "reference={invalid}");
        }
        assert_eq!(collect_text_nodes("<t/><t>a&amp;b</t>").unwrap(), "a&b");
        assert!(collect_text_nodes("<t>x").is_err());
    }

    #[test]
    fn xml_helpers_cover_quotes_numeric_entities_and_tag_boundaries() {
        assert_eq!(xml_attribute("<c r='B2'>", "r"), Some("B2"));
        assert_eq!(xml_attribute("<c other=\"x\">", "r"), None);
        assert_eq!(first_text_node("<v/>", "v").unwrap(), Some(String::new()));
        assert_eq!(first_text_node("<x/>", "v").unwrap(), None);
        assert_eq!(
            first_text_node("<v>&lt;&gt;&quot;&apos;&#65;&#x42;</v>", "v").unwrap(),
            Some("<>\"'AB".to_string())
        );
        assert_eq!(
            find_start_tag("<sheetData/><sheet/>", "sheet", 0)
                .unwrap()
                .0,
            12
        );
        assert!(first_text_node("<v>x", "v").is_err());
        for invalid in [
            "<t>&unknown;</t>",
            "<t>&#xZZ;</t>",
            "<t>&#99999999;</t>",
            "<t>&amp</t>",
        ] {
            assert!(collect_text_nodes(invalid).is_err(), "xml={invalid}");
        }
    }
}
