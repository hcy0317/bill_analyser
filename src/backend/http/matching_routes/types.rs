#[derive(Debug, Default, Deserialize)]
struct CalendarEventsQuery {
    start_date: Option<String>,
    end_date: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RecurringSuggestionsQuery {
    status: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MatchingCandidatesQuery {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "billId")]
    bill_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ReconciliationCandidatesQuery {
    #[serde(rename = "candidateType")]
    candidate_type: Option<String>,
    status: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "previewId")]
    preview_id: Option<String>,
    #[serde(rename = "billId")]
    bill_id: Option<String>,
    limit: Option<String>,
}
