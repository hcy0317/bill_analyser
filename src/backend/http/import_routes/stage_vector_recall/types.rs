#[derive(Debug, Clone, Default)]
struct ImportLearningVectorRecallStats {
    status: String,
    recalled: usize,
}

#[derive(Debug, Clone)]
struct ImportLearningVectorRecallResult {
    draft_index: usize,
    scope_order: usize,
    hits: Vec<WeaviateImportLearningRecallHit>,
}

const IMPORT_VECTOR_RECALL_MAX_CONCURRENCY: usize = 8;
