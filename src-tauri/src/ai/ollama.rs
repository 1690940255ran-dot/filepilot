//! Ollama 适配器（规格 6.4）。
//!
//! 走 `/api/chat`，与 OpenAI 兼容端点共用同一套系统提示、载荷构造与输出
//! 校验（都在 [`super::provider`] 里）——**只有请求与响应的形状不同**。
//!
//! 规格要求「兼容适配器和 Ollama 适配器分别实现同一个
//! `suggest(batch, cancellation) -> Result<Vec<Proposal>>` 语义」，
//! 这里的 [`suggest`] 与 [`super::compatible::suggest`] 就是那两个实现。
//!
//! ## 结构化输出用的是 `format: "json"`
//!
//! Ollama 没有 OpenAI 的 `response_format`，它用 `format`。这与
//! `compatible` 里那个字段是**同一个能力**的两种叫法，所以两边都发，
//! 也都不因此省掉客户端的校验。
//!
//! ## 它只连本机
//!
//! 端点在 [`super::provider::ProviderContext::from_config`] 里已经按
//! 「本地提供商的端点只允许 loopback」校验过，且 `localhost` 那时就被
//! 换成了 `127.0.0.1`。所以这个适配器**没有**「连到别人机器上」这条路。

use super::http::Cancellation;
use super::prompt;
use super::provider::{self, ProviderContext, SuggestionBatch};
use crate::domain::errors::{codes, AppError};
use crate::domain::types::Proposal;

/// 请求体。
fn request_body(context: &ProviderContext, batch: &SuggestionBatch) -> String {
    let body = serde_json::json!({
        "model": context.model,
        "messages": [
            { "role": "system", "content": prompt::system_prompt() },
            { "role": "user", "content": prompt::user_message(batch) },
        ],
        "stream": false,
        // Ollama 的结构化输出开关。与 OpenAI 的 `response_format` 同一个能力。
        "format": "json",
        "options": { "temperature": 0 },
    });

    body.to_string()
}

/// 从响应体里取出助手回复的内容。
fn extract_content(body: &str) -> Result<String, AppError> {
    let parsed: serde_json::Value = serde_json::from_str(body).map_err(|error| {
        AppError::new(
            codes::MODEL_INVALID_OUTPUT,
            format!("Ollama 返回的不是合法 JSON：{error}"),
        )
    })?;

    // Ollama 的模型不存在时回的是 `{"error": "model 'x' not found"}`。
    // 那句原话对用户最有用——他要拿它去改设置页里的模型名。
    if let Some(message) = parsed.get("error").and_then(|error| error.as_str()) {
        return Err(AppError::new(
            codes::MODEL_INVALID_OUTPUT,
            format!("Ollama 返回了错误：{message}"),
        ));
    }

    parsed
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            AppError::new(
                codes::MODEL_INVALID_OUTPUT,
                "响应里没有 message.content：这可能不是 Ollama 服务。",
            )
        })
}

/// 请模型给出一批建议。
pub fn suggest(
    context: &ProviderContext,
    batch: &SuggestionBatch,
    cancellation: &dyn Cancellation,
) -> Result<Vec<Proposal>, AppError> {
    let body = request_body(context, batch);
    let response = provider::post(context, "api/chat", &body, cancellation)?;
    provider::ensure_success(response.status, &response.body)?;

    let content = extract_content(&response.body)?;
    let known: Vec<String> = batch
        .items
        .iter()
        .map(|item| item.file_id.clone())
        .collect();
    provider::parse_proposals(&content, &known)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> ProviderContext {
        let config = provider::ProviderConfig {
            id: "p".to_owned(),
            kind: provider::ProviderKind::Local,
            endpoint: "http://localhost:11434".to_owned(),
            model: "qwen2.5:7b".to_owned(),
            credential_ref: None,
        };
        ProviderContext::from_config(&config, None).expect("应能构造上下文")
    }

    fn batch() -> SuggestionBatch {
        SuggestionBatch {
            instruction: "按主题分类".to_owned(),
            items: vec![
                provider::SuggestionItem::new("f1", "笔记", "md", "会议记录").expect("构造"),
            ],
        }
    }

    #[test]
    fn the_request_body_asks_for_json_without_streaming() {
        let body = request_body(&context(), &batch());
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("应是合法 JSON");

        assert_eq!(parsed["model"], "qwen2.5:7b");
        assert_eq!(parsed["stream"], false, "流式响应会打乱「一次读完」的模型");
        assert_eq!(parsed["format"], "json");
        assert_eq!(parsed["options"]["temperature"], 0);
    }

    #[test]
    fn the_local_endpoint_is_normalised_to_an_ip_literal() {
        // `localhost` 在构造上下文时就被换掉了——这里的断言说明
        // 「本地请求不经过名字解析」这条在适配器这一层也成立。
        assert_eq!(context().endpoint.host, "127.0.0.1");
    }

    #[test]
    fn a_normal_response_yields_its_content() {
        let body =
            r#"{"model":"qwen2.5:7b","message":{"role":"assistant","content":"[]"},"done":true}"#;
        assert_eq!(extract_content(body).expect("应能取出内容"), "[]");
    }

    #[test]
    fn an_unknown_model_error_is_quoted_back_to_the_user() {
        // 这句原话是用户改模型名的依据，不能被换成一句笼统的「失败」。
        let body = r#"{"error":"model 'qwen9' not found, try pulling it first"}"#;
        let error = extract_content(body).expect_err("错误必须被认出来");
        assert_eq!(error.code, codes::MODEL_INVALID_OUTPUT);
        assert!(error.message.contains("qwen9"), "{}", error.message);
    }

    #[test]
    fn a_missing_message_field_is_reported_as_invalid_output() {
        let error = extract_content(r#"{"done":true}"#).expect_err("没有 message 必须报错");
        assert!(error.message.contains("Ollama"), "{}", error.message);
    }

    #[test]
    fn a_non_json_body_is_reported_as_invalid_output() {
        // Ollama 没起来时，中间可能有一个别的服务在应答。
        let error = extract_content("<html>404</html>").expect_err("非 JSON 必须报错");
        assert_eq!(error.code, codes::MODEL_INVALID_OUTPUT);
    }
}
