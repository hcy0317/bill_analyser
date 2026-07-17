const IMPORT_PARSE_REQUEST_MAX_MATERIALIZED_BYTES: usize =
    MAX_SPREADSHEET_TOTAL_CELL_BYTES;

#[derive(Debug)]
struct ImportParseRequestBudget {
    materialized_bytes: usize,
    max_materialized_bytes: usize,
}

impl ImportParseRequestBudget {
    fn new(max_materialized_bytes: usize) -> Self {
        Self {
            materialized_bytes: 0,
            max_materialized_bytes,
        }
    }

    fn consume_bills(
        &mut self,
        bills: &[StandardBill],
    ) -> Result<(), ImportV2RouteResponse> {
        let additional_bytes = bills.iter().fold(0usize, |total, bill| {
            total.saturating_add(standard_bill_materialized_bytes(bill))
        });
        let materialized_bytes = self.materialized_bytes.saturating_add(additional_bytes);
        if materialized_bytes > self.max_materialized_bytes {
            return Err(import_v2_error_response(
                413,
                "Import parser request exceeds the materialized output limit",
            ));
        }
        self.materialized_bytes = materialized_bytes;
        Ok(())
    }
}

fn standard_bill_materialized_bytes(bill: &StandardBill) -> usize {
    let string_bytes = [
        &bill.date,
        &bill.transaction_type,
        &bill.description,
        &bill.source_account_id,
        &bill.counterparty,
        &bill.payment_method,
        &bill.original_type,
        &bill.original_category,
        &bill.transaction_id,
        &bill.merchant_id,
        &bill.status,
        &bill.main_category,
        &bill.sub_category,
    ]
    .into_iter()
    .fold(0usize, |total, value| total.saturating_add(value.len()));
    let tag_bytes = bill.parser_tags.iter().fold(0usize, |total, value| {
        total.saturating_add(std::mem::size_of::<String>())
            .saturating_add(value.len())
    });
    std::mem::size_of::<StandardBill>()
        .saturating_add(string_bytes)
        .saturating_add(tag_bytes)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn parse_multipart_import_files_bounded(
    file_parts: Vec<MultipartPart>,
    requested_parser: &str,
) -> Result<Vec<ImportMultipartFileParseResult>, ImportV2RouteResponse> {
    let mut budget = ImportParseRequestBudget::new(IMPORT_PARSE_REQUEST_MAX_MATERIALIZED_BYTES);
    let mut results = Vec::with_capacity(file_parts.len());
    for (index, part) in file_parts.into_iter().enumerate() {
        let input = ImportMultipartFileParseInput {
            index,
            original_name: part
                .filename
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "import-file.csv".to_string()),
            body: part.body,
            requested_parser: requested_parser.to_string(),
        };
        let result = tokio::task::spawn_blocking(move || parse_multipart_import_file(input))
            .await
            .map_err(|error| {
                import_v2_error_response(500, &format!("Import parser worker failed: {error}"))
            })?;
        if let Some(parsed) = &result.parsed {
            budget.consume_bills(&parsed.bills)?;
        }
        results.push(result);
    }
    Ok(results)
}

fn unmatched_import_file_payload(
    original_name: &str,
    temp_path: &str,
    parser_id: &str,
    decision: &DedicatedParserDecision,
) -> Value {
    let error_code = decision.error_code();
    json!({
        "original_name": original_name,
        "originalName": original_name,
        "filename": original_name,
        "temp_path": temp_path,
        "tempPath": temp_path,
        "parser_id": parser_id,
        "reason": decision.reason,
        "error_code": error_code,
        "errorCode": error_code,
        "parser_decision": parser_decision_json(decision),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_multipart_import_file(
    input: ImportMultipartFileParseInput,
) -> ImportMultipartFileParseResult {
    let started_at = Instant::now();
    let selected = parse_dedicated_import_bytes_with_decision(
        &input.original_name,
        &input.body,
        &input.requested_parser,
    );
    let decision = selected.decision;
    let parsed = selected
        .parsed
        .filter(|parsed| !parsed.bills.is_empty())
        .map(|parsed| ImportMultipartParsedFile {
            source_index: input.index,
            parser_id: parsed.parser_id,
            parsed_count: parsed.bills.len(),
            delimiter: parsed.delimiter,
            bills: parsed.bills,
            decision: parsed.decision,
        });
    ImportMultipartFileParseResult {
        index: input.index,
        original_name: input.original_name,
        body: input.body,
        parsed,
        decision,
        _elapsed_ms: import_stage_elapsed_ms(started_at),
    }
}
