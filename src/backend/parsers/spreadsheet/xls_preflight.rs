// Minimal, read-only MS-CFB/BIFF preflight. It runs before calamine so hostile
// worksheet dimensions cannot trigger calamine's dense Range allocation first.

use std::collections::HashSet;

use super::{
    SpreadsheetValidationError, MAX_SPREADSHEET_ARCHIVE_ENTRIES, MAX_SPREADSHEET_CELLS,
    MAX_SPREADSHEET_COLUMNS, MAX_SPREADSHEET_INPUT_BYTES, MAX_SPREADSHEET_ROWS,
};

const END_OF_CHAIN: u32 = 0xffff_fffe;
const FREE_SECTOR: u32 = 0xffff_ffff;
const FAT_SECTOR: u32 = 0xffff_fffd;
const DIFAT_SECTOR: u32 = 0xffff_fffc;
const MINI_STREAM_CUTOFF: usize = 4096;
const MINI_SECTOR_SIZE: usize = 64;

pub(super) fn validate(bytes: &[u8]) -> Result<(), SpreadsheetValidationError> {
    let compound = CompoundFile::parse(bytes)?;
    let workbook = compound.workbook_stream()?;
    validate_biff_budget(&workbook)
}

struct CompoundFile<'a> {
    bytes: &'a [u8],
    sector_size: usize,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    mini_stream: Vec<u8>,
    directories: Vec<Directory>,
}

#[derive(Clone)]
struct Directory {
    name: String,
    start_sector: u32,
    stream_len: usize,
}

impl<'a> CompoundFile<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, SpreadsheetValidationError> {
        if bytes.len() < 512 || !bytes.starts_with(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1") {
            return Err(invalid());
        }
        if read_u16(bytes, 28)? != 0xfffe || read_u16(bytes, 32)? != 6 {
            return Err(invalid());
        }
        let major_version = read_u16(bytes, 26)?;
        let sector_size = match read_u16(bytes, 30)? {
            9 if major_version == 3 => 512,
            12 if major_version == 4 => 4096,
            _ => return Err(invalid()),
        };
        if bytes.len() < sector_size {
            return Err(invalid());
        }
        let physical_sector_count = (bytes.len() - sector_size) / sector_size;

        let fat_sector_count = read_u32(bytes, 44)? as usize;
        let first_directory_sector = read_u32(bytes, 48)?;
        let first_mini_fat_sector = read_u32(bytes, 60)?;
        let mini_fat_sector_count = read_u32(bytes, 64)? as usize;
        let first_difat_sector = read_u32(bytes, 68)?;
        let difat_sector_count = read_u32(bytes, 72)? as usize;
        // calamine 0.35 reads its initial DIFAT Vec capacity from bytes 62..66.
        // Bound that dependency-specific read before constructing Xls, even though
        // the normative MS-CFB DIFAT sector count lives at bytes 72..76.
        let calamine_difat_capacity = read_u32(bytes, 62)? as usize;
        if fat_sector_count > physical_sector_count
            || difat_sector_count > physical_sector_count
            || calamine_difat_capacity > MAX_SPREADSHEET_CELLS
        {
            return Err(too_large());
        }

        let mut difat = Vec::with_capacity(fat_sector_count);
        let mut seen_fat_sectors = HashSet::new();
        for chunk in bytes[76..512].chunks_exact(4) {
            push_fat_sector(
                u32::from_le_bytes(chunk.try_into().expect("four-byte DIFAT entry")),
                physical_sector_count,
                &mut seen_fat_sectors,
                &mut difat,
            )?;
        }
        let mut next_difat = first_difat_sector;
        let mut seen_difat = HashSet::new();
        for _ in 0..difat_sector_count {
            if !valid_regular_sector(next_difat) || !seen_difat.insert(next_difat) {
                return Err(invalid());
            }
            let sector = sector(bytes, sector_size, next_difat)?;
            let values = sector
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("four-byte chunk")))
                .collect::<Vec<_>>();
            let Some((&chain, entries)) = values.split_last() else {
                return Err(invalid());
            };
            for value in entries.iter().copied() {
                push_fat_sector(
                    value,
                    physical_sector_count,
                    &mut seen_fat_sectors,
                    &mut difat,
                )?;
            }
            next_difat = chain;
        }
        if difat.len() != fat_sector_count || next_difat != END_OF_CHAIN {
            return Err(invalid());
        }

        let mut fat = Vec::new();
        for fat_sector in difat.into_iter().take(fat_sector_count) {
            fat.extend(
                sector(bytes, sector_size, fat_sector)?
                    .chunks_exact(4)
                    .map(|chunk| {
                        u32::from_le_bytes(chunk.try_into().expect("four-byte FAT entry"))
                    }),
            );
        }
        let directory_bytes = read_regular_chain_to_end(
            bytes,
            sector_size,
            &fat,
            first_directory_sector,
            MAX_SPREADSHEET_INPUT_BYTES,
        )?;
        let directories = directory_bytes
            .chunks_exact(128)
            .map(|entry| parse_directory(entry, major_version))
            .collect::<Result<Vec<_>, _>>()?;
        let root = directories.first().ok_or_else(invalid)?;

        let (mini_fat, mini_stream) = if mini_fat_sector_count == 0 {
            (Vec::new(), Vec::new())
        } else {
            if root.stream_len > MAX_SPREADSHEET_INPUT_BYTES {
                return Err(too_large());
            }
            let mini_fat_bytes = read_regular_chain(
                bytes,
                sector_size,
                &fat,
                first_mini_fat_sector,
                mini_fat_sector_count
                    .checked_mul(sector_size)
                    .ok_or_else(too_large)?,
            )?;
            let mini_fat = mini_fat_bytes
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("four-byte mini FAT")))
                .collect::<Vec<_>>();
            let mini_stream =
                read_regular_chain(bytes, sector_size, &fat, root.start_sector, root.stream_len)?;
            (mini_fat, mini_stream)
        };

        Ok(Self {
            bytes,
            sector_size,
            fat,
            mini_fat,
            mini_stream,
            directories,
        })
    }

    fn workbook_stream(&self) -> Result<Vec<u8>, SpreadsheetValidationError> {
        let entry = ["Workbook", "Book", "WORKBOOK", "BOOK"]
            .into_iter()
            .find_map(|name| self.directories.iter().find(|entry| entry.name == name))
            .ok_or_else(invalid)?;
        if entry.stream_len > MAX_SPREADSHEET_INPUT_BYTES {
            return Err(too_large());
        }
        if entry.stream_len < MINI_STREAM_CUTOFF {
            read_chain_from_slice(
                &self.mini_stream,
                MINI_SECTOR_SIZE,
                &self.mini_fat,
                entry.start_sector,
                entry.stream_len,
            )
        } else {
            read_regular_chain(
                self.bytes,
                self.sector_size,
                &self.fat,
                entry.start_sector,
                entry.stream_len,
            )
        }
    }
}

fn parse_directory(
    entry: &[u8],
    major_version: u16,
) -> Result<Directory, SpreadsheetValidationError> {
    let units = entry[..64]
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes(chunk.try_into().expect("UTF-16 code unit")))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    let stream_len_u64 = if major_version == 3 {
        u64::from(read_u32(entry, 120)?)
    } else {
        read_u64(entry, 120)?
    };
    let stream_len = usize::try_from(stream_len_u64).map_err(|_| too_large())?;
    Ok(Directory {
        name: String::from_utf16_lossy(&units),
        start_sector: read_u32(entry, 116)?,
        stream_len,
    })
}

fn read_regular_chain(
    bytes: &[u8],
    sector_size: usize,
    fat: &[u32],
    start_sector: u32,
    len: usize,
) -> Result<Vec<u8>, SpreadsheetValidationError> {
    let data = bytes.get(sector_size..).ok_or_else(invalid)?;
    read_chain_from_slice(data, sector_size, fat, start_sector, len)
}

fn read_regular_chain_to_end(
    bytes: &[u8],
    sector_size: usize,
    fat: &[u32],
    start_sector: u32,
    max_len: usize,
) -> Result<Vec<u8>, SpreadsheetValidationError> {
    let data = bytes.get(sector_size..).ok_or_else(invalid)?;
    let mut sector_id = start_sector;
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    while sector_id != END_OF_CHAIN {
        let index = usize::try_from(sector_id).map_err(|_| invalid())?;
        if !seen.insert(sector_id) || index >= fat.len() {
            return Err(invalid());
        }
        let start = index.checked_mul(sector_size).ok_or_else(invalid)?;
        let end = start.checked_add(sector_size).ok_or_else(invalid)?;
        output.extend_from_slice(data.get(start..end).ok_or_else(invalid)?);
        if output.len() > max_len {
            return Err(too_large());
        }
        sector_id = fat[index];
    }
    if output.is_empty() {
        return Err(invalid());
    }
    Ok(output)
}

fn read_chain_from_slice(
    data: &[u8],
    sector_size: usize,
    fat: &[u32],
    mut sector_id: u32,
    len: usize,
) -> Result<Vec<u8>, SpreadsheetValidationError> {
    if len > MAX_SPREADSHEET_INPUT_BYTES {
        return Err(too_large());
    }
    let mut output = Vec::with_capacity(len);
    let mut seen = HashSet::new();
    while output.len() < len {
        if sector_id == END_OF_CHAIN {
            return Err(invalid());
        }
        let index = usize::try_from(sector_id).map_err(|_| invalid())?;
        if !seen.insert(sector_id) || index >= fat.len() {
            return Err(invalid());
        }
        let start = index.checked_mul(sector_size).ok_or_else(invalid)?;
        let end = start.checked_add(sector_size).ok_or_else(invalid)?;
        let source = data.get(start..end).ok_or_else(invalid)?;
        let remaining = len - output.len();
        output.extend_from_slice(&source[..remaining.min(sector_size)]);
        sector_id = fat[index];
    }
    Ok(output)
}

fn sector(
    bytes: &[u8],
    sector_size: usize,
    sector_id: u32,
) -> Result<&[u8], SpreadsheetValidationError> {
    let index = usize::try_from(sector_id).map_err(|_| invalid())?;
    let start = sector_size
        .checked_add(index.checked_mul(sector_size).ok_or_else(invalid)?)
        .ok_or_else(invalid)?;
    bytes
        .get(start..start.checked_add(sector_size).ok_or_else(invalid)?)
        .ok_or_else(invalid)
}

fn validate_biff_budget(stream: &[u8]) -> Result<(), SpreadsheetValidationError> {
    let globals = scan_biff_records(stream, true)?;
    if globals.sheet_offsets.len() > MAX_SPREADSHEET_ARCHIVE_ENTRIES {
        return Err(too_large());
    }
    let mut total_dense_cells = 0usize;
    for offset in globals.sheet_offsets {
        let sheet = stream.get(offset..).ok_or_else(invalid)?;
        let scanned = scan_biff_records(sheet, false)?;
        total_dense_cells = total_dense_cells
            .checked_add(scanned.dense_cells)
            .ok_or_else(too_large)?;
        if total_dense_cells > MAX_SPREADSHEET_CELLS {
            return Err(too_large());
        }
    }
    Ok(())
}

struct BiffScan {
    sheet_offsets: Vec<usize>,
    dense_cells: usize,
}

fn scan_biff_records(
    stream: &[u8],
    collect_sheet_offsets: bool,
) -> Result<BiffScan, SpreadsheetValidationError> {
    let mut offset = 0usize;
    let mut bounds = Bounds::default();
    let mut sheet_offsets = Vec::new();
    let mut has_formula = false;
    while offset < stream.len() {
        if stream.len() - offset < 4 && stream[offset..].iter().all(|byte| *byte == 0) {
            break;
        }
        let header = stream.get(offset..offset + 4).ok_or_else(|| {
            SpreadsheetValidationError::invalid("Import XLS BIFF record header is truncated")
        })?;
        let record_type = u16::from_le_bytes([header[0], header[1]]);
        let record_len = usize::from(u16::from_le_bytes([header[2], header[3]]));
        offset = offset.checked_add(4).ok_or_else(invalid)?;
        let data = stream
            .get(offset..offset.checked_add(record_len).ok_or_else(invalid)?)
            .ok_or_else(|| {
                SpreadsheetValidationError::invalid("Import XLS BIFF record is truncated")
            })?;
        match record_type {
            0x0085 if collect_sheet_offsets => {
                if data.len() < 6 {
                    return Err(invalid());
                }
                let sheet_offset = usize::try_from(read_u32(data, 0)?).map_err(|_| invalid())?;
                if sheet_offset >= stream.len() {
                    return Err(invalid());
                }
                sheet_offsets.push(sheet_offset);
            }
            0x0200 => validate_dimensions(data)?,
            0x0203 | 0x0204 | 0x0205 | 0x00fd | 0x027e | 0x0006 => {
                if data.len() < 4 {
                    return Err(SpreadsheetValidationError::invalid(
                        "Import XLS BIFF cell record is truncated",
                    ));
                }
                bounds.add(read_u16(data, 0)? as u32, read_u16(data, 2)? as u32)?;
                has_formula |= record_type == 0x0006;
            }
            0x00bd => {
                if data.len() < 6 {
                    return Err(invalid());
                }
                let row = read_u16(data, 0)? as u32;
                let first_column = read_u16(data, 2)? as u32;
                let last_column = read_u16(data, data.len() - 2)? as u32;
                bounds.add(row, first_column)?;
                bounds.add(row, last_column)?;
            }
            0x0809 if data.len() >= 4 && read_u16(data, 2)? == 0x0010 => {
                bounds = Bounds::default();
            }
            0x00e5 => validate_merge_cells(data)?,
            0x000a => break,
            _ => {}
        }
        offset += record_len;
    }
    let dense_cells = bounds
        .dense_cells()?
        .checked_mul(usize::from(has_formula) + 1)
        .ok_or_else(too_large)?;
    if dense_cells > MAX_SPREADSHEET_CELLS {
        return Err(too_large());
    }
    Ok(BiffScan {
        sheet_offsets,
        dense_cells,
    })
}

fn validate_merge_cells(data: &[u8]) -> Result<(), SpreadsheetValidationError> {
    let count = usize::from(read_u16(data, 0)?);
    let required = count
        .checked_mul(8)
        .and_then(|value| value.checked_add(2))
        .ok_or_else(too_large)?;
    if data.len() < required {
        return Err(invalid());
    }
    Ok(())
}

fn validate_dimensions(data: &[u8]) -> Result<(), SpreadsheetValidationError> {
    let (first_row, last_row, mut first_column, last_column) = match data.len() {
        10 => (
            read_u16(data, 0)? as u32,
            read_u16(data, 2)? as u32,
            read_u16(data, 4)? as u32,
            read_u16(data, 6)? as u32,
        ),
        14 => (
            read_u32(data, 0)?,
            read_u32(data, 4)?,
            read_u16(data, 8)? as u32,
            read_u16(data, 10)? as u32,
        ),
        _ => return Err(invalid()),
    };
    if first_column > 0xff || last_column < first_column {
        first_column = 0;
    }
    if (last_row != 0 && last_row <= first_row) || (last_column != 0 && last_column <= first_column)
    {
        return Err(invalid());
    }
    let rows = last_row.saturating_sub(first_row).max(1) as usize;
    let columns = last_column.saturating_sub(first_column).max(1) as usize;
    validate_shape(rows, columns)
}

#[derive(Default)]
struct Bounds {
    min_row: Option<u32>,
    max_row: u32,
    min_column: Option<u32>,
    max_column: u32,
    cells: usize,
}

impl Bounds {
    fn add(&mut self, row: u32, column: u32) -> Result<(), SpreadsheetValidationError> {
        self.min_row = Some(self.min_row.map_or(row, |value| value.min(row)));
        self.max_row = self.max_row.max(row);
        self.min_column = Some(self.min_column.map_or(column, |value| value.min(column)));
        self.max_column = self.max_column.max(column);
        self.cells = self.cells.checked_add(1).ok_or_else(too_large)?;
        if self.cells > MAX_SPREADSHEET_CELLS {
            return Err(too_large());
        }
        let rows = (self.max_row - self.min_row.expect("row bound set") + 1) as usize;
        let columns = (self.max_column - self.min_column.expect("column bound set") + 1) as usize;
        validate_shape(rows, columns)
    }

    fn dense_cells(&self) -> Result<usize, SpreadsheetValidationError> {
        let (Some(min_row), Some(min_column)) = (self.min_row, self.min_column) else {
            return Ok(0);
        };
        let rows = (self.max_row - min_row + 1) as usize;
        let columns = (self.max_column - min_column + 1) as usize;
        rows.checked_mul(columns).ok_or_else(too_large)
    }
}

fn validate_shape(rows: usize, columns: usize) -> Result<(), SpreadsheetValidationError> {
    let cells = rows.checked_mul(columns).ok_or_else(too_large)?;
    if rows > MAX_SPREADSHEET_ROWS
        || columns > MAX_SPREADSHEET_COLUMNS
        || cells > MAX_SPREADSHEET_CELLS
    {
        return Err(too_large());
    }
    Ok(())
}

fn valid_regular_sector(value: u32) -> bool {
    !matches!(
        value,
        FREE_SECTOR | END_OF_CHAIN | FAT_SECTOR | DIFAT_SECTOR
    )
}

fn push_fat_sector(
    value: u32,
    physical_sector_count: usize,
    seen: &mut HashSet<u32>,
    output: &mut Vec<u32>,
) -> Result<(), SpreadsheetValidationError> {
    if value == FREE_SECTOR {
        return Ok(());
    }
    let index = usize::try_from(value).map_err(|_| invalid())?;
    if !valid_regular_sector(value) || index >= physical_sector_count || !seen.insert(value) {
        return Err(invalid());
    }
    output.push(value);
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, SpreadsheetValidationError> {
    let value = bytes.get(offset..offset + 2).ok_or_else(invalid)?;
    Ok(u16::from_le_bytes(
        value.try_into().expect("two-byte integer"),
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, SpreadsheetValidationError> {
    let value = bytes.get(offset..offset + 4).ok_or_else(invalid)?;
    Ok(u32::from_le_bytes(
        value.try_into().expect("four-byte integer"),
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, SpreadsheetValidationError> {
    let value = bytes.get(offset..offset + 8).ok_or_else(invalid)?;
    Ok(u64::from_le_bytes(
        value.try_into().expect("eight-byte integer"),
    ))
}

fn invalid() -> SpreadsheetValidationError {
    SpreadsheetValidationError::invalid("Import preview spreadsheet content is invalid")
}

fn too_large() -> SpreadsheetValidationError {
    SpreadsheetValidationError::too_large("Import spreadsheet exceeds the resource limit")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compound_preflight_accepts_bounded_binary_xls() {
        let bytes =
            include_bytes!("../../../../tests/fixtures/import_samples/abc_statement_sample.xls");
        let compound = CompoundFile::parse(bytes).expect("compound file parses");
        let workbook = compound.workbook_stream().expect("workbook stream reads");
        validate_biff_budget(&workbook).expect("BIFF records stay within budget");
    }

    #[test]
    fn biff_preflight_rejects_hostile_dimensions_and_sparse_cells() {
        let mut dimensions = Vec::new();
        dimensions.extend_from_slice(&0x0200u16.to_le_bytes());
        dimensions.extend_from_slice(&14u16.to_le_bytes());
        dimensions.extend_from_slice(&0u32.to_le_bytes());
        dimensions.extend_from_slice(&65_536u32.to_le_bytes());
        dimensions.extend_from_slice(&0u16.to_le_bytes());
        dimensions.extend_from_slice(&256u16.to_le_bytes());
        dimensions.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(
            validate_biff_budget(&dimensions).unwrap_err().kind(),
            super::super::SpreadsheetValidationErrorKind::TooLarge
        );

        let mut reversed = dimensions.clone();
        reversed[4..8].copy_from_slice(&100u32.to_le_bytes());
        reversed[8..12].copy_from_slice(&50u32.to_le_bytes());
        assert_eq!(
            validate_biff_budget(&reversed).unwrap_err().kind(),
            super::super::SpreadsheetValidationErrorKind::Invalid
        );

        let mut sparse = Vec::new();
        for (row, column) in [(0u16, 0u16), (49_999u16, 127u16)] {
            sparse.extend_from_slice(&0x0203u16.to_le_bytes());
            sparse.extend_from_slice(&14u16.to_le_bytes());
            sparse.extend_from_slice(&row.to_le_bytes());
            sparse.extend_from_slice(&column.to_le_bytes());
            sparse.extend_from_slice(&[0; 10]);
        }
        assert!(validate_biff_budget(&sparse).is_err());
    }

    #[test]
    fn compound_header_fat_and_directory_guards_fail_closed() {
        let fixture =
            include_bytes!("../../../../tests/fixtures/import_samples/abc_statement_sample.xls");
        let mutate = |offset: usize, value: &[u8]| {
            let mut bytes = fixture.to_vec();
            bytes[offset..offset + value.len()].copy_from_slice(value);
            bytes
        };

        assert!(CompoundFile::parse(&mutate(28, &0u16.to_le_bytes())).is_err());
        assert!(CompoundFile::parse(&mutate(30, &10u16.to_le_bytes())).is_err());
        let mut short_v4 = mutate(26, &4u16.to_le_bytes());
        short_v4[30..32].copy_from_slice(&12u16.to_le_bytes());
        short_v4.truncate(512);
        assert!(CompoundFile::parse(&short_v4).is_err());

        let physical = (fixture.len() - 512) / 512;
        assert!(CompoundFile::parse(&mutate(
            44,
            &u32::try_from(physical + 1).unwrap().to_le_bytes()
        ))
        .is_err());
        assert!(CompoundFile::parse(&mutate(
            62,
            &u32::try_from(MAX_SPREADSHEET_CELLS + 1)
                .unwrap()
                .to_le_bytes()
        ))
        .is_err());
        let fat_sector = read_u32(fixture, 76).unwrap();
        assert!(CompoundFile::parse(&mutate(80, &fat_sector.to_le_bytes())).is_err());
        assert!(CompoundFile::parse(&mutate(44, &2u32.to_le_bytes())).is_err());
        assert!(CompoundFile::parse(&mutate(72, &1u32.to_le_bytes())).is_err());
        assert!(CompoundFile::parse(&mutate(76, &999u32.to_le_bytes())).is_err());
        assert!(CompoundFile::parse(&mutate(48, &END_OF_CHAIN.to_le_bytes())).is_err());

        let mut looped_directory = fixture.to_vec();
        let directory_sector = read_u32(&looped_directory, 48).unwrap();
        let fat_offset = 512 + usize::try_from(fat_sector).unwrap() * 512;
        let entry_offset = fat_offset + usize::try_from(directory_sector).unwrap() * 4;
        looped_directory[entry_offset..entry_offset + 4]
            .copy_from_slice(&directory_sector.to_le_bytes());
        assert!(CompoundFile::parse(&looped_directory).is_err());
    }

    #[test]
    fn compound_single_byte_mutations_always_fail_closed_or_stay_bounded() {
        let fixture =
            include_bytes!("../../../../tests/fixtures/import_samples/abc_statement_sample.xls");
        for index in 0..fixture.len() {
            for replacement in [0u8, 0xff] {
                if fixture[index] == replacement {
                    continue;
                }
                let mut mutated = fixture.to_vec();
                mutated[index] = replacement;
                let _ = validate(&mutated);
            }
        }
    }

    #[test]
    fn chain_directory_and_stream_helpers_cover_failure_boundaries() {
        assert!(read_regular_chain(&[], 512, &[], 0, 1).is_err());
        assert!(read_chain_from_slice(&[], 64, &[], 0, MAX_SPREADSHEET_INPUT_BYTES + 1).is_err());
        assert!(read_chain_from_slice(&[], 64, &[], END_OF_CHAIN, 1).is_err());
        assert!(read_chain_from_slice(&[0; 64], 64, &[], 0, 1).is_err());
        assert!(read_chain_from_slice(&[0; 64], 64, &[0], 0, 65).is_err());
        assert!(read_chain_from_slice(&[0; 63], 64, &[END_OF_CHAIN], 0, 1).is_err());
        assert_eq!(
            read_chain_from_slice(&[1; 64], 64, &[END_OF_CHAIN], 0, 3).unwrap(),
            [1, 1, 1]
        );
        assert!(read_regular_chain_to_end(&[0; 512], 512, &[], END_OF_CHAIN, 1).is_err());
        assert!(read_regular_chain_to_end(&[0; 1024], 512, &[END_OF_CHAIN], 0, 0).is_err());
        assert!(sector(&[0; 512], 512, 0).is_err());

        let mut directory = [0u8; 128];
        directory[64..66].copy_from_slice(&4u16.to_le_bytes());
        directory[..2].copy_from_slice(&('A' as u16).to_le_bytes());
        directory[116..120].copy_from_slice(&7u32.to_le_bytes());
        directory[120..128].copy_from_slice(&9u64.to_le_bytes());
        assert_eq!(parse_directory(&directory, 3).unwrap().stream_len, 9);
        assert_eq!(parse_directory(&directory, 4).unwrap().stream_len, 9);
        directory[64..66].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(parse_directory(&directory, 3).unwrap().name, "A");

        let exact = CompoundFile {
            bytes: &[],
            sector_size: 512,
            fat: vec![],
            mini_fat: vec![END_OF_CHAIN],
            mini_stream: vec![7; 64],
            directories: vec![
                Directory {
                    name: "workbook".to_string(),
                    start_sector: 99,
                    stream_len: 1,
                },
                Directory {
                    name: "Workbook".to_string(),
                    start_sector: 0,
                    stream_len: 3,
                },
            ],
        };
        assert_eq!(exact.workbook_stream().unwrap(), [7, 7, 7]);
        let missing = CompoundFile {
            directories: vec![],
            ..exact
        };
        assert!(missing.workbook_stream().is_err());
    }

    #[test]
    fn biff_record_and_numeric_helpers_fail_closed() {
        assert!(validate_biff_budget(&[1]).is_err());
        assert!(validate_biff_budget(&[1, 0, 8, 0]).is_err());
        assert!(validate_biff_budget(&[3, 2, 0, 0]).is_err());
        assert!(validate_biff_budget(&[0xbd, 0, 0, 0]).is_err());
        assert!(validate_biff_budget(&[0x85, 0, 0, 0]).is_err());
        assert!(validate_biff_budget(&[0xe5, 0, 2, 0, 1, 0]).is_err());

        let mut bound_sheet = Vec::new();
        bound_sheet.extend_from_slice(&0x0085u16.to_le_bytes());
        bound_sheet.extend_from_slice(&6u16.to_le_bytes());
        bound_sheet.extend_from_slice(&14u32.to_le_bytes());
        bound_sheet.extend_from_slice(&[0, 0]);
        bound_sheet.extend_from_slice(&0x000au16.to_le_bytes());
        bound_sheet.extend_from_slice(&0u16.to_le_bytes());
        bound_sheet.extend_from_slice(&0x0200u16.to_le_bytes());
        bound_sheet.extend_from_slice(&14u16.to_le_bytes());
        bound_sheet.extend_from_slice(&0u32.to_le_bytes());
        bound_sheet.extend_from_slice(&65_536u32.to_le_bytes());
        bound_sheet.extend_from_slice(&0u16.to_le_bytes());
        bound_sheet.extend_from_slice(&256u16.to_le_bytes());
        bound_sheet.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(
            validate_biff_budget(&bound_sheet).unwrap_err().kind(),
            super::super::SpreadsheetValidationErrorKind::TooLarge
        );
        bound_sheet[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_biff_budget(&bound_sheet).is_err());

        let mut mul_rk = Vec::new();
        mul_rk.extend_from_slice(&0x00bdu16.to_le_bytes());
        mul_rk.extend_from_slice(&6u16.to_le_bytes());
        mul_rk.extend_from_slice(&0u16.to_le_bytes());
        mul_rk.extend_from_slice(&0u16.to_le_bytes());
        mul_rk.extend_from_slice(&1u16.to_le_bytes());
        assert!(validate_biff_budget(&mul_rk).is_ok());

        let mut bof = Vec::new();
        bof.extend_from_slice(&0x0809u16.to_le_bytes());
        bof.extend_from_slice(&4u16.to_le_bytes());
        bof.extend_from_slice(&0u16.to_le_bytes());
        bof.extend_from_slice(&0x0010u16.to_le_bytes());
        assert!(validate_biff_budget(&bof).is_ok());
        assert!(validate_biff_budget(&[0, 0, 0]).is_ok());

        let dimensions10 = [0, 0, 2, 0, 0, 0, 2, 0, 0, 0];
        assert!(validate_dimensions(&dimensions10).is_ok());
        assert!(validate_dimensions(&[0; 3]).is_err());
        let reset_column = [0, 0, 2, 0, 0xff, 1, 2, 0, 0, 0];
        assert!(validate_dimensions(&reset_column).is_ok());

        let mut full = Bounds {
            cells: MAX_SPREADSHEET_CELLS,
            ..Bounds::default()
        };
        assert!(full.add(0, 0).is_err());
        assert!(validate_shape(usize::MAX, 2).is_err());
        assert!(validate_shape(MAX_SPREADSHEET_ROWS + 1, 1).is_err());
        assert!(validate_shape(1, MAX_SPREADSHEET_COLUMNS + 1).is_err());
        assert!(validate_shape(1_000, 1_001).is_err());

        let mut seen = HashSet::new();
        let mut output = Vec::new();
        assert!(push_fat_sector(FREE_SECTOR, 1, &mut seen, &mut output).is_ok());
        assert!(push_fat_sector(0, 1, &mut seen, &mut output).is_ok());
        assert!(push_fat_sector(0, 1, &mut seen, &mut output).is_err());
        assert!(push_fat_sector(FAT_SECTOR, 1, &mut seen, &mut output).is_err());
        assert!(read_u16(&[], 0).is_err());
        assert!(read_u32(&[], 0).is_err());
        assert!(read_u64(&[], 0).is_err());
    }
}
