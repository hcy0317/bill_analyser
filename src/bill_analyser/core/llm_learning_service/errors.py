"""Stable error types for LLM import-session analysis."""


class LLMImportSessionAnalysisError(ValueError):
    """Stable import-session LLM analysis error for API/UI branching."""

    def __init__(self, code: str, message: str, status_code: int = 400) -> None:
        super().__init__(message)
        self.code = code
        self.status_code = status_code
