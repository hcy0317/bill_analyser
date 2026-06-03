use super::types::{ResponseEnvelopeFamily, ResponseEnvelopePolicy, RuntimeState};

pub(super) const RUST_ENVELOPE_CONTEXT: &[RuntimeState] = &[
    RuntimeState::RustImplemented,
    RuntimeState::RustOwnedVerified,
    RuntimeState::Retired,
];
pub(super) const CONTRACT_ENVELOPE_CONTEXT: &[RuntimeState] = &[RuntimeState::ContractOnly];

pub(super) const ENVELOPE_POLICIES: &[ResponseEnvelopePolicy] = &[
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::RustHttpShell,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "typed Rust health/runtime JSON",
        error_shape: "typed Rust shell error when applicable",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::RuntimeInfrastructureError,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "none",
        error_shape: "ApiResponse success=false error.code/message",
        runtime_may_wrap: true,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::CurrentSuccessData,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data emitted by current Rust routes",
        error_shape: "success=false error/code/message in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::CurrentSuccessResult,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result/message emitted by current Rust routes",
        error_shape: "success=false error/code/message in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::RawPassthrough,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "raw Rust file/download bodies and headers",
        error_shape: "current API file/download error response",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::BillsCrud,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result with frontend transaction DTO or page wrapper",
        error_shape: "success=false error in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::BudgetsCrud,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape:
            "success=true result/message with budget row/list/export/execution/forecast payload",
        error_shape: "success=false error in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::StatisticsRead,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result/data with DB-backed statistics aggregation payload",
        error_shape: "success=false error/message in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ImportV2Stage,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data with import stage payload",
        error_shape: "success=false error/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ImportPreviewAction,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data with preview projection",
        error_shape: "success=false error plus expectedState when stale",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data responseMode=preview-item",
        error_shape: "success=false code/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::LearningRoute,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data/result for learning routes",
        error_shape: "success=false code/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::LlmPreview,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data/result for LLM preview/memory",
        error_shape: "success=false code/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::OcrMl,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result for Rust OCR config and receipt recognition routes",
        error_shape: "success=false code/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ContractOracle,
        route_contexts: CONTRACT_ENVELOPE_CONTEXT,
        success_shape: "compile-time contract data only",
        error_shape: "none",
        runtime_may_wrap: false,
    },
];
