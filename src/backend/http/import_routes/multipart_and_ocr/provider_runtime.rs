use super::{
    multipart_network_llm_ocr::run_network_llm_ocr,
    multipart_ocr_local_json::run_local_json_ocr,
    multipart_ocr_tesseract::run_tesseract_ocr, OcrConfigContract, OcrProviderTextResult,
    NETWORK_OCR_PROVIDER_NAME,
};

/// OCR provider 执行失败的 transport-neutral 分类；HTTP 状态与 JSON 由 response adapter 投影。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum OcrProviderFailure {
    Unavailable { message: String },
    TimedOut { message: String },
    InvalidOutput { message: String },
    ReauthenticationRequired { message: String },
}

impl OcrProviderFailure {
    pub(super) fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable {
            message: message.into(),
        }
    }

    pub(super) fn timed_out(message: impl Into<String>) -> Self {
        Self::TimedOut {
            message: message.into(),
        }
    }

    pub(super) fn invalid_output(message: impl Into<String>) -> Self {
        Self::InvalidOutput {
            message: message.into(),
        }
    }

    pub(super) fn reauthentication_required(message: impl Into<String>) -> Self {
        Self::ReauthenticationRequired {
            message: message.into(),
        }
    }
}

/// 静态分发到三个已配置 OCR adapter；未知 provider 失败关闭且不创建动态 registry。
#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn run_ocr_provider(
    config: &OcrConfigContract,
    image_bytes: Vec<u8>,
    mime: String,
) -> Result<OcrProviderTextResult, OcrProviderFailure> {
    match config.provider.as_str() {
        "cloud_stub" => Err(OcrProviderFailure::unavailable(
            "cloud_ocr_not_configured",
        )),
        "tesseract" => run_tesseract_ocr(config.lang.clone(), image_bytes, mime).await,
        "local_json_ocr" => run_local_json_ocr(image_bytes, mime).await,
        NETWORK_OCR_PROVIDER_NAME => run_network_llm_ocr(config, image_bytes, mime).await,
        _ => Err(OcrProviderFailure::unavailable(
            "ocr provider not configured",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config(provider: &str) -> OcrConfigContract {
        OcrConfigContract {
            provider: provider.to_string(),
            lang: "chi_sim+eng".to_string(),
            model: String::new(),
            base_url: String::new(),
            parameters: json!({}),
            credential_config: json!({}),
        }
    }

    #[tokio::test]
    async fn dispatcher_fails_closed_stub_and_unknown_providers() {
        let cloud = run_ocr_provider(
            &config("cloud_stub"),
            Vec::new(),
            "image/png".to_string(),
        )
        .await
        .expect_err("cloud stub stays unavailable");
        assert_eq!(
            cloud,
            OcrProviderFailure::unavailable("cloud_ocr_not_configured")
        );

        let unknown = run_ocr_provider(
            &config("future_provider"),
            Vec::new(),
            "image/png".to_string(),
        )
        .await
        .expect_err("unknown provider fails closed");
        assert_eq!(
            unknown,
            OcrProviderFailure::unavailable("ocr provider not configured")
        );
    }

    #[test]
    fn failure_constructors_preserve_typed_messages() {
        assert_eq!(
            OcrProviderFailure::timed_out("timeout"),
            OcrProviderFailure::TimedOut {
                message: "timeout".to_string()
            }
        );
        assert_eq!(
            OcrProviderFailure::invalid_output("invalid"),
            OcrProviderFailure::InvalidOutput {
                message: "invalid".to_string()
            }
        );
        assert_eq!(
            OcrProviderFailure::reauthentication_required("login"),
            OcrProviderFailure::ReauthenticationRequired {
                message: "login".to_string()
            }
        );
    }
}
