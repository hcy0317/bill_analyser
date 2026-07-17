mod worksheet;
mod xml;

use std::{collections::BTreeSet, io::Read};

use self::{
    worksheet::{parse_shared_strings, parse_worksheet_rows},
    xml::{find_start_tag, xml_attribute},
};
use super::{
    SpreadsheetPreviewRows, SpreadsheetValidationError, MAX_SPREADSHEET_ARCHIVE_ENTRIES,
    MAX_SPREADSHEET_EXPANDED_BYTES, MAX_SPREADSHEET_WORKSHEET_BYTES,
};

pub(super) fn parse_preview_rows(
    bytes: &[u8],
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    parse_rows(bytes, super::SPREADSHEET_PREVIEW_SAMPLE_ROWS)
}

pub(super) fn parse_rows(
    bytes: &[u8],
    materialized_row_limit: usize,
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    let entry_names = validate_archive(bytes)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| invalid())?;
    let workbook = read_text_entry(&mut archive, "xl/workbook.xml")?;
    let relationships = read_text_entry(&mut archive, "xl/_rels/workbook.xml.rels")?;
    let worksheet_path = first_worksheet_path(&workbook, &relationships)?;
    if !entry_names.contains(&worksheet_path) {
        return Err(SpreadsheetValidationError::invalid(
            "Import preview spreadsheet worksheet is missing",
        ));
    }
    let shared_strings = if entry_names.contains("xl/sharedStrings.xml") {
        parse_shared_strings(&read_text_entry(&mut archive, "xl/sharedStrings.xml")?)?
    } else {
        Vec::new()
    };
    let worksheet = read_text_entry(&mut archive, &worksheet_path)?;
    parse_worksheet_rows(&worksheet, &shared_strings, materialized_row_limit)
}

fn validate_archive(bytes: &[u8]) -> Result<BTreeSet<String>, SpreadsheetValidationError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| invalid())?;
    if archive.len() > MAX_SPREADSHEET_ARCHIVE_ENTRIES {
        return Err(expansion_limit());
    }
    let mut expanded = 0u64;
    let mut names = BTreeSet::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|_| invalid())?;
        expanded = expanded
            .checked_add(file.size())
            .ok_or_else(expansion_limit)?;
        if expanded > MAX_SPREADSHEET_EXPANDED_BYTES {
            return Err(expansion_limit());
        }
        let name = file.name().replace('\\', "/");
        if name.starts_with('/')
            || name.split('/').any(|component| component == "..")
            || !names.insert(name.clone())
        {
            return Err(SpreadsheetValidationError::invalid(
                "Import preview spreadsheet contains an invalid archive entry",
            ));
        }
        if name.starts_with("xl/worksheets/")
            && name.ends_with(".xml")
            && file.size() > MAX_SPREADSHEET_WORKSHEET_BYTES
        {
            return Err(SpreadsheetValidationError::too_large(
                "Import preview worksheet exceeds the expansion limit",
            ));
        }
    }
    if !names.contains("[Content_Types].xml")
        || !names.contains("xl/workbook.xml")
        || !names.contains("xl/_rels/workbook.xml.rels")
    {
        return Err(invalid());
    }
    Ok(names)
}

fn read_text_entry(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
) -> Result<String, SpreadsheetValidationError> {
    let file = archive.by_name(name).map_err(|_| invalid())?;
    if file.size() > MAX_SPREADSHEET_WORKSHEET_BYTES {
        return Err(SpreadsheetValidationError::too_large(
            "Import preview spreadsheet entry exceeds the expansion limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_SPREADSHEET_WORKSHEET_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_SPREADSHEET_WORKSHEET_BYTES {
        return Err(SpreadsheetValidationError::too_large(
            "Import preview spreadsheet entry exceeds the expansion limit",
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        SpreadsheetValidationError::invalid("Import preview spreadsheet XML is not valid UTF-8")
    })
}

fn first_worksheet_path(
    workbook: &str,
    relationships: &str,
) -> Result<String, SpreadsheetValidationError> {
    let (start, end, _) = find_start_tag(workbook, "sheet", 0).ok_or_else(|| {
        SpreadsheetValidationError::invalid("Import preview spreadsheet has no worksheet")
    })?;
    let relationship_id = xml_attribute(&workbook[start..=end], "r:id").ok_or_else(|| {
        SpreadsheetValidationError::invalid("Import preview spreadsheet worksheet is invalid")
    })?;
    let mut cursor = 0usize;
    while let Some((start, end, _)) = find_start_tag(relationships, "Relationship", cursor) {
        let tag = &relationships[start..=end];
        if xml_attribute(tag, "Id") == Some(relationship_id) {
            let target = xml_attribute(tag, "Target").ok_or_else(|| {
                SpreadsheetValidationError::invalid(
                    "Import preview spreadsheet worksheet is invalid",
                )
            })?;
            return normalize_worksheet_target(target);
        }
        cursor = end.saturating_add(1);
    }
    Err(SpreadsheetValidationError::invalid(
        "Import preview spreadsheet worksheet relationship is missing",
    ))
}

fn normalize_worksheet_target(target: &str) -> Result<String, SpreadsheetValidationError> {
    let target = target.trim().replace('\\', "/");
    let target = if target.starts_with('/') {
        target.trim_start_matches('/').to_string()
    } else {
        format!("xl/{target}")
    };
    if target
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
        || !target.starts_with("xl/worksheets/")
        || !target.ends_with(".xml")
    {
        return Err(SpreadsheetValidationError::invalid(
            "Import preview spreadsheet worksheet target is invalid",
        ));
    }
    Ok(target)
}

fn invalid() -> SpreadsheetValidationError {
    SpreadsheetValidationError::invalid("Import preview spreadsheet content is invalid")
}

fn expansion_limit() -> SpreadsheetValidationError {
    SpreadsheetValidationError::too_large("Import preview spreadsheet exceeds the expansion limit")
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::*;

    #[test]
    fn worksheet_targets_stay_inside_the_xlsx_worksheet_directory() {
        assert_eq!(
            normalize_worksheet_target("worksheets/sheet1.xml").unwrap(),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(
            normalize_worksheet_target("/xl/worksheets/sheet2.xml").unwrap(),
            "xl/worksheets/sheet2.xml"
        );
        for invalid in [
            "../sheet.xml",
            "worksheets/../sheet.xml",
            "theme/theme1.xml",
        ] {
            assert!(normalize_worksheet_target(invalid).is_err());
        }
    }

    #[test]
    fn parses_minimal_archive_with_shared_inline_and_numeric_cells() {
        let bytes = archive(&[
            ("[Content_Types].xml", b"<Types/>"),
            (
                "xl/workbook.xml",
                br#"<workbook><sheet r:id="rId1"/></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                br#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
            (
                "xl/sharedStrings.xml",
                br#"<sst><si><t>shared</t></si></sst>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                br#"<worksheet><dimension ref="A1:C1"/><sheetData><row><c r="A1" t="s"><v>0</v></c><c r="B1" t="inlineStr"><is><t>inline</t></is></c><c r="C1"><v>42</v></c></row></sheetData></worksheet>"#,
            ),
        ]);
        let preview = parse_preview_rows(&bytes).expect("minimal XLSX preview");
        assert_eq!(preview.total_rows, 1);
        assert_eq!(preview.rows[0], ["shared", "inline", "42"]);
    }

    #[test]
    fn archive_and_relationship_errors_fail_closed() {
        assert!(validate_archive(b"not zip").is_err());
        assert!(validate_archive(&archive(&[("[Content_Types].xml", b"<Types/>")])).is_err());
        assert!(validate_archive(&archive(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("xl/workbook.xml", b"<workbook/>"),
            ("xl/_rels/workbook.xml.rels", b"<Relationships/>"),
            ("../escape.xml", b"<x/>"),
        ]))
        .is_err());

        assert!(first_worksheet_path("<workbook/>", "<Relationships/>").is_err());
        assert!(first_worksheet_path("<workbook><sheet/></workbook>", "<Relationships/>").is_err());
        assert!(first_worksheet_path(
            r#"<workbook><sheet r:id="missing"/></workbook>"#,
            r#"<Relationships><Relationship Id="other" Target="worksheets/sheet1.xml"/></Relationships>"#
        )
        .is_err());
        assert!(first_worksheet_path(
            r#"<workbook><sheet r:id="rId1"/></workbook>"#,
            r#"<Relationships><Relationship Id="rId1"/></Relationships>"#
        )
        .is_err());
    }

    #[test]
    fn text_entries_require_utf8_and_existing_worksheets() {
        let bytes = archive(&[("bad.xml", &[0xff, 0xfe])]);
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        assert!(read_text_entry(&mut zip, "bad.xml").is_err());
        assert!(read_text_entry(&mut zip, "missing.xml").is_err());

        let missing_sheet = archive(&[
            ("[Content_Types].xml", b"<Types/>"),
            (
                "xl/workbook.xml",
                br#"<workbook><sheet r:id="rId1"/></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                br#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
        ]);
        assert!(parse_preview_rows(&missing_sheet).is_err());
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for (name, contents) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
}
