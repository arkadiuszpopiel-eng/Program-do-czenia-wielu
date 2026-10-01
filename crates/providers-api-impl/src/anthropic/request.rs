//! Budowa ciała `POST /v1/messages` (streaming) z neutralnego IR.
//!
//! Zgodnie z dokumentacją Messages API (skill `claude-api`, wrzesień 2026):
//! stabilny prefiks cache `tools → system → messages`; `output_config.effort` jawnie; myślenia
//! modeli „zawsze włączonych" (Opus 5.5) nie wyłączamy; wymuszony `tool_choice` degradowany do
//! `auto` (+ `strict`); bloki myślenia odsyłane bez zmian (tylko własne, podpisane); parametry
//! próbkowania pomijane, gdy model ich nie przyjmuje.

use providers_contract::{
    CacheTtl, ChatRequest, ContentBlock, ImageSource, InterruptionRendering, Message,
    ModelCapabilities, ProviderError, ProviderErrorKind, ProviderId, Role, ThinkingDisplay,
    ThinkingSupport, ToolChoice, ToolResultPart, project_history,
};
use serde_json::{Map, Value, json};

use super::AnthropicOptions;
use lib_openai_compat::ProviderProfile;

/// Beta: notki postępu w blokach myślenia (`display: "updates"`).
pub const BETA_THINKING_UPDATES: &str = "thinking-display-updates-2026-08-18";
/// Beta: kontrola wiązania bloków myślenia z rozmową.
pub const BETA_THINKING_BINDING: &str = "thinking-binding-controls-2026-08-01";
/// Beta: serwerowy fallback po odmowie (`fallbacks: "default"`).
pub const BETA_SERVER_FALLBACK: &str = "server-side-fallback-2026-07-01";
/// Prefiks instrukcji systemowej ze środka rozmowy renderowanej jako tekst użytkownika
/// (deterministycznie — ten sam bajt w każdym kolejnym żądaniu).
pub const MID_SYSTEM_PREFIX: &str = "[Instrukcja systemowa Alfy] ";

fn cache_control(ttl: CacheTtl) -> Value {
    match ttl {
        CacheTtl::Short => json!({"type": "ephemeral"}),
        CacheTtl::Long => json!({"type": "ephemeral", "ttl": "1h"}),
    }
}

fn image(source: &ImageSource) -> Result<Value, ProviderError> {
    match source {
        ImageSource::Base64 { media_type, data } => Ok(json!({
            "type": "image",
            "source": {"type": "base64", "media_type": media_type, "data": data}
        })),
        ImageSource::Url { url } => {
            Ok(json!({"type": "image", "source": {"type": "url", "url": url}}))
        }
        ImageSource::Ref { id, .. } => Err(ProviderError::invalid_request(format!(
            "nierozwiązana referencja obrazu `{id}` — rozwiąż ją przed wywołaniem"
        ))),
    }
}

fn tool_result_parts(parts: &[ToolResultPart]) -> Result<Vec<Value>, ProviderError> {
    parts
        .iter()
        .map(|p| match p {
            ToolResultPart::Text { text } => Ok(json!({"type": "text", "text": text})),
            ToolResultPart::Image { source } => image(source),
        })
        .collect()
}

/// Renderuje jedną wiadomość; `None` = brak bloków do wysłania (np. tylko obce myślenie).
fn message(
    msg: &Message,
    own: &ProviderId,
    strip_thinking: bool,
) -> Result<Option<Value>, ProviderError> {
    let assistant = msg.role == Role::Assistant;
    let mut results = Vec::new();
    let mut blocks = Vec::new();
    for block in &msg.content {
        match block {
            ContentBlock::Text { text } if text.is_empty() => {}
            ContentBlock::Text { text } if msg.role == Role::System => {
                blocks.push(json!({"type": "text", "text": format!("{MID_SYSTEM_PREFIX}{text}")}));
            }
            ContentBlock::Text { text } => blocks.push(json!({"type": "text", "text": text})),
            ContentBlock::Image { source } => blocks.push(image(source)?),
            ContentBlock::Thinking(t) => {
                if let (true, false, true, Some(sig)) = (
                    assistant,
                    strip_thinking,
                    &t.provider_origin.provider == own,
                    &t.signature,
                ) {
                    blocks.push(json!({"type": "thinking", "thinking": t.text, "signature": sig}));
                }
            }
            ContentBlock::RedactedThinking(r) => {
                if assistant && !strip_thinking && &r.provider_origin.provider == own {
                    blocks.push(json!({"type": "redacted_thinking", "data": r.data}));
                }
            }
            ContentBlock::ToolUse(t) if assistant => blocks.push(json!({
                "type": "tool_use", "id": t.id, "name": t.name, "input": t.input
            })),
            ContentBlock::ToolResult(r) if !assistant => {
                let mut v = json!({
                    "type": "tool_result",
                    "tool_use_id": r.tool_use_id,
                    "content": tool_result_parts(&r.content)?,
                });
                if r.is_error {
                    v["is_error"] = Value::Bool(true);
                }
                results.push(v);
            }
            ContentBlock::ToolUse(_) | ContentBlock::ToolResult(_) => {
                return Err(ProviderError::invalid_request(
                    "tool_use tylko w turze asystentki, tool_result tylko w turze użytkownika",
                ));
            }
        }
    }
    // API wymaga, by wyniki narzędzi były pierwsze w wiadomości użytkownika.
    results.extend(blocks);
    if results.is_empty() {
        return Ok(None);
    }
    let role = if assistant { "assistant" } else { "user" };
    Ok(Some(json!({"role": role, "content": results})))
}

/// Ustawia `cache_control` na ostatnim bloku, który go przyjmuje (nie myślenie).
fn mark_last_block(message: &mut Value, ttl: CacheTtl) {
    if let Some(blocks) = message["content"].as_array_mut()
        && let Some(block) = blocks
            .iter_mut()
            .rev()
            .find(|b| !matches!(b["type"].as_str(), Some("thinking" | "redacted_thinking")))
    {
        block["cache_control"] = cache_control(ttl);
    }
}

fn tool_choice(choice: &ToolChoice, forced_ok: bool) -> Value {
    match choice {
        ToolChoice::Auto => json!({"type": "auto"}),
        ToolChoice::None => json!({"type": "none"}),
        ToolChoice::Required if forced_ok => json!({"type": "any"}),
        ToolChoice::Tool { name } if forced_ok => json!({"type": "tool", "name": name}),
        // Opus 5.5 / Sonnet 5.5 / Fable 5.1: `any`/`tool` → 400; `auto` + `strict` + prośba w prompcie.
        ToolChoice::Required | ToolChoice::Tool { .. } => json!({"type": "auto"}),
    }
}

fn thinking(
    req: &ChatRequest,
    caps: &ModelCapabilities,
    o: &AnthropicOptions,
    betas: &mut Vec<&'static str>,
) -> Option<Value> {
    let want = req.params.thinking;
    let mut t = match caps.thinking {
        ThinkingSupport::None => return None,
        ThinkingSupport::Optional if !want.enabled => return Some(json!({"type": "disabled"})),
        ThinkingSupport::Optional | ThinkingSupport::AlwaysOn => json!({"type": "adaptive"}),
    };
    match want.display {
        ThinkingDisplay::Omitted => {}
        ThinkingDisplay::Summarized => t["display"] = json!("summarized"),
        ThinkingDisplay::Updates if o.native => {
            t["display"] = json!("updates");
            betas.push(BETA_THINKING_UPDATES);
        }
        ThinkingDisplay::Updates => t["display"] = json!("summarized"),
    }
    if let (true, Some(mode)) = (o.native, o.block_binding) {
        t["block_binding"] = json!({"prefix_mismatch_behavior": mode.as_str()});
        betas.push(BETA_THINKING_BINDING);
    }
    Some(t)
}

/// Buduje ciało żądania i listę nagłówków beta.
pub(crate) fn build_body(
    req: &ChatRequest,
    caps: &ModelCapabilities,
    profile: &ProviderProfile,
    o: &AnthropicOptions,
    strip_thinking: bool,
) -> Result<(Value, Vec<&'static str>), ProviderError> {
    if !req.tools.is_empty() && !caps.tools {
        return Err(ProviderError::new(
            ProviderErrorKind::Unsupported,
            format!("model `{}` nie obsługuje narzędzi", req.model),
        ));
    }
    let cache = req.cache.enabled && caps.prompt_cache;
    let mut betas = Vec::new();
    let max_tokens = req
        .params
        .max_tokens
        .or(caps.max_output_tokens)
        .unwrap_or(profile.default_max_tokens);
    let mut body = Map::new();
    body.insert("model".into(), json!(req.model));
    body.insert("max_tokens".into(), json!(max_tokens));
    body.insert("stream".into(), json!(true));

    let mut tools: Vec<_> = req.tools.iter().collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name)); // deterministyczny prefiks cache
    let mut tools: Vec<Value> = tools
        .into_iter()
        .map(|t| {
            let mut v = json!({"name": t.name, "description": t.description, "input_schema": t.input_schema});
            if t.strict && caps.strict_tools {
                v["strict"] = json!(true);
            }
            if o.eager_input_streaming {
                v["eager_input_streaming"] = json!(true);
            }
            v
        })
        .collect();
    if cache
        && req.system.is_none()
        && let Some(last) = tools.last_mut()
    {
        last["cache_control"] = cache_control(req.cache.ttl);
    }
    if !tools.is_empty() {
        body.insert("tools".into(), Value::Array(tools));
        body.insert(
            "tool_choice".into(),
            tool_choice(&req.tool_choice, caps.forced_tool_choice),
        );
    }
    if let Some(system) = &req.system {
        let mut block = json!({"type": "text", "text": system});
        if cache {
            block["cache_control"] = cache_control(req.cache.ttl);
        }
        body.insert("system".into(), json!([block]));
    }

    let own = &profile.id;
    let mut messages = Vec::with_capacity(req.messages.len() + 2);
    for msg in project_history(&req.messages, InterruptionRendering::AppendNote) {
        if let Some(m) = message(&msg, own, strip_thinking)? {
            messages.push(m);
        }
    }
    if cache && let Some(last) = messages.last_mut() {
        mark_last_block(last, req.cache.ttl);
    }
    body.insert("messages".into(), Value::Array(messages));

    if let Some(t) = thinking(req, caps, o, &mut betas) {
        body.insert("thinking".into(), t);
    }
    if caps.effort {
        let effort = req.params.effort.unwrap_or(profile.default_effort);
        body.insert("output_config".into(), json!({"effort": effort.as_str()}));
    }
    if let (true, Some(t)) = (caps.sampling, req.params.temperature) {
        body.insert("temperature".into(), json!(t.clamp(0.0, 1.0)));
    }
    if !req.params.stop.is_empty() {
        body.insert("stop_sequences".into(), json!(req.params.stop));
    }
    if o.native && o.server_fallback_models.iter().any(|m| m == &req.model) {
        body.insert("fallbacks".into(), json!("default"));
        betas.push(BETA_SERVER_FALLBACK);
    }
    Ok((Value::Object(body), betas))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anthropic::{AnthropicOptions, PrefixMismatch, models};
    use providers_contract::{CacheTtl, ToolResult, ToolUse};

    fn body(req: &ChatRequest, o: &AnthropicOptions) -> (Value, Vec<&'static str>) {
        let caps = models::known("claude-opus-5-5").unwrap();
        build_body(req, &caps, &ProviderProfile::new("anthropic"), o, false).unwrap()
    }

    #[test]
    fn images_tool_results_ttl_updates_and_binding() {
        let png = ImageSource::Base64 {
            media_type: "image/png".into(),
            data: "AAA".into(),
        };
        let url = ImageSource::Url {
            url: "https://example.invalid/a.png".into(),
        };
        let call = Message::new(
            Role::Assistant,
            vec![ContentBlock::ToolUse(ToolUse {
                id: "t1".into(),
                name: "shot".into(),
                input: json!({}),
            })],
        );
        let result = Message::new(
            Role::User,
            vec![
                ContentBlock::text("i jeszcze to"),
                ContentBlock::ToolResult(ToolResult {
                    tool_use_id: "t1".into(),
                    content: vec![ToolResultPart::Image {
                        source: png.clone(),
                    }],
                    is_error: true,
                }),
            ],
        );
        let mut req = ChatRequest::new(
            "claude-opus-5-5",
            vec![
                Message::new(Role::User, vec![ContentBlock::Image { source: url }]),
                call,
                result,
            ],
        );
        req.cache.ttl = CacheTtl::Long;
        req.params.thinking.display = ThinkingDisplay::Updates;
        let mut o = AnthropicOptions::native();
        o.block_binding = Some(PrefixMismatch::DropBlock);
        let (b, betas) = body(&req, &o);
        assert_eq!(
            b["messages"][0]["content"][0]["source"],
            json!({"type": "url", "url": "https://example.invalid/a.png"})
        );
        let user = &b["messages"][2]["content"];
        assert_eq!(user[0]["type"], "tool_result", "wyniki narzędzi pierwsze");
        assert_eq!(user[0]["is_error"], true);
        assert_eq!(user[0]["content"][0]["source"]["type"], "base64");
        assert_eq!(
            user[1]["cache_control"],
            json!({"type": "ephemeral", "ttl": "1h"})
        );
        assert_eq!(
            b["thinking"],
            json!({"type": "adaptive", "display": "updates", "block_binding": {"prefix_mismatch_behavior": "drop_block"}})
        );
        assert!(betas.contains(&BETA_THINKING_UPDATES) && betas.contains(&BETA_THINKING_BINDING));
        assert_eq!(PrefixMismatch::Error.as_str(), "error");

        let (b, betas) = body(&req, &AnthropicOptions::compatible());
        assert_eq!(
            b["thinking"]["display"], "summarized",
            "bez bety: streszczenie zamiast notek"
        );
        assert!(betas.is_empty());
    }

    #[test]
    fn invalid_shapes_are_rejected_locally() {
        let unresolved = ImageSource::Ref {
            id: "art-1".into(),
            media_type: "image/png".into(),
        };
        let req = ChatRequest::new(
            "m",
            vec![Message::new(
                Role::User,
                vec![ContentBlock::Image { source: unresolved }],
            )],
        );
        let caps = models::unknown_claude();
        let o = AnthropicOptions::native();
        let profile = ProviderProfile::new("anthropic");
        assert!(build_body(&req, &caps, &profile, &o, false).is_err());
        let misplaced = Message::new(
            Role::User,
            vec![ContentBlock::ToolUse(ToolUse {
                id: "x".into(),
                name: "n".into(),
                input: json!({}),
            })],
        );
        let req = ChatRequest::new("m", vec![misplaced]);
        assert!(build_body(&req, &caps, &profile, &o, false).is_err());
        let only_foreign = Message::new(Role::Assistant, vec![ContentBlock::text("")]);
        let req = ChatRequest::new("m", vec![Message::user_text("a"), only_foreign]);
        let (b, _) = build_body(&req, &caps, &profile, &o, true).unwrap();
        assert_eq!(
            b["messages"].as_array().map(Vec::len),
            Some(1),
            "pusta tura pominięta"
        );
        assert!(b.get("output_config").is_none() && b.get("thinking").is_none());
    }
}
