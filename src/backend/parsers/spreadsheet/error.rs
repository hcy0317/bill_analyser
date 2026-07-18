#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpreadsheetValidationErrorKind {
    Invalid,
    TooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpreadsheetValidationError {
    kind: SpreadsheetValidationErrorKind,
    message: &'static str,
}

impl SpreadsheetValidationError {
    pub(super) fn invalid(message: &'static str) -> Self {
        Self {
            kind: SpreadsheetValidationErrorKind::Invalid,
            message,
        }
    }

    pub(super) fn too_large(message: &'static str) -> Self {
        Self {
            kind: SpreadsheetValidationErrorKind::TooLarge,
            message,
        }
    }

    pub fn kind(&self) -> SpreadsheetValidationErrorKind {
        self.kind
    }

    pub fn message(&self) -> &'static str {
        self.message
    }
}
