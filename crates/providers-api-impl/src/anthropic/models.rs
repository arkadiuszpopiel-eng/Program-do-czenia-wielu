//! Możliwości modeli Anthropic: tabela znanych modeli (z dokumentacji API, skill `claude-api`,
//! stan 2026-09) i mapowanie odpowiedzi Models API (`GET /v1/models`). Ceny **nie** są tu
//! trzymane — pochodzą z konfiguracji.

use providers_contract::{ModelCapabilities, ModelInfo, ModelKind, ThinkingSupport};
use serde_json::Value;

fn claude(
    thinking: ThinkingSupport,
    forced_tool_choice: bool,
    sampling: bool,
) -> ModelCapabilities {
    ModelCapabilities {
        kinds: vec![ModelKind::Chat, ModelKind::Vision],
        context_window: Some(1_000_000),
        max_output_tokens: Some(128_000),
        tools: true,
        strict_tools: true,
        forced_tool_choice,
        vision: true,
        thinking,
        effort: thinking != ThinkingSupport::None,
        sampling,
        streaming: true,
        prompt_cache: true,
        native_truncate: false,
    }
}

/// Możliwości znanych modeli. `claude-opus-5-5`: myślenie zawsze włączone (`disabled` → 400),
/// `tool_choice any/tool` → 400, `temperature` → 400; domyślny `effort` = `medium`.
pub(crate) fn known(model: &str) -> Option<ModelCapabilities> {
    let caps = match model {
        "claude-opus-5-5" | "claude-sonnet-5-5" | "claude-fable-5-1" | "claude-mythos-5-1" => {
            claude(ThinkingSupport::AlwaysOn, false, false)
        }
        "claude-fable-5" => claude(ThinkingSupport::AlwaysOn, true, false),
        "claude-opus-5" | "claude-opus-4-8" | "claude-opus-4-7" | "claude-sonnet-5" => {
            claude(ThinkingSupport::Optional, true, false)
        }
        "claude-opus-4-6" | "claude-sonnet-4-6" => claude(ThinkingSupport::Optional, true, true),
        "claude-haiku-4-5" => ModelCapabilities {
            context_window: Some(200_000),
            max_output_tokens: None,
            effort: false,
            ..claude(ThinkingSupport::None, true, true)
        },
        _ => return None,
    };
    Some(caps)
}

/// Ostrożne możliwości nieznanego modelu Claude (narzędzia i wizja tak; nic opcjonalnego nie wysyłamy).
pub(crate) fn unknown_claude() -> ModelCapabilities {
    ModelCapabilities {
        kinds: vec![ModelKind::Chat, ModelKind::Vision],
        tools: true,
        vision: true,
        ..ModelCapabilities::default()
    }
}

fn supported(v: &Value) -> bool {
    v.get("supported").and_then(Value::as_bool).unwrap_or(false)
}

fn as_u32(v: &Value) -> Option<u32> {
    v.as_u64().and_then(|x| u32::try_from(x).ok())
}

/// Mapuje wpis Models API. Tabela znanych modeli ma pierwszeństwo tam, gdzie API milczy
/// (np. czy myślenie da się wyłączyć, czy przyjmowana jest temperatura).
pub(crate) fn from_models_api(entry: &Value) -> Option<ModelInfo> {
    let id = entry.get("id")?.as_str()?.to_owned();
    let base = known(&id).unwrap_or_else(unknown_claude);
    let caps = entry
        .get("capabilities")
        .filter(|c| c.is_object())
        .map_or_else(
            || base.clone(),
            |c| {
                let adaptive = supported(&c["thinking"]["types"]["adaptive"]);
                let thinking = match (adaptive, base.thinking) {
                    (false, _) => ThinkingSupport::None,
                    (true, ThinkingSupport::AlwaysOn) => ThinkingSupport::AlwaysOn,
                    (true, _) => ThinkingSupport::Optional,
                };
                let vision = supported(&c["image_input"]);
                let mut kinds = vec![ModelKind::Chat];
                if vision {
                    kinds.push(ModelKind::Vision);
                }
                ModelCapabilities {
                    kinds,
                    vision,
                    thinking,
                    effort: supported(&c["effort"]),
                    strict_tools: supported(&c["structured_outputs"]),
                    ..base.clone()
                }
            },
        );
    let caps = ModelCapabilities {
        context_window: as_u32(&entry["max_input_tokens"]).or(caps.context_window),
        max_output_tokens: as_u32(&entry["max_tokens"]).or(caps.max_output_tokens),
        ..caps
    };
    Some(ModelInfo {
        id,
        display_name: entry["display_name"].as_str().map(str::to_owned),
        created: entry["created_at"].as_str().map(str::to_owned),
        capabilities: Some(caps),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_5_5_is_always_on_without_forcing_or_sampling() {
        let c = known("claude-opus-5-5").unwrap();
        assert_eq!(c.thinking, ThinkingSupport::AlwaysOn);
        assert!(!c.forced_tool_choice && !c.sampling && c.effort && c.prompt_cache);
        assert!(known("claude-haiku-4-5").is_some_and(|h| h.thinking == ThinkingSupport::None));
        assert!(known("nieznany").is_none());
    }

    #[test]
    fn models_api_entry_maps_capabilities() {
        let entry = serde_json::json!({
            "id": "claude-opus-5-5", "display_name": "Claude Opus 5.5",
            "created_at": "2026-09-01T00:00:00Z",
            "max_input_tokens": 1_000_000, "max_tokens": 128_000,
            "capabilities": {
                "image_input": {"supported": true},
                "structured_outputs": {"supported": true},
                "thinking": {"supported": true, "types": {"enabled": {"supported": false}, "adaptive": {"supported": true}}},
                "effort": {"supported": true}
            }
        });
        let info = from_models_api(&entry).unwrap();
        let caps = info.capabilities.unwrap();
        assert_eq!(
            caps.thinking,
            ThinkingSupport::AlwaysOn,
            "tabela znanych modeli uzupełnia API"
        );
        assert!(caps.vision && caps.effort && caps.strict_tools);
        assert_eq!(caps.context_window, Some(1_000_000));
        let bare = from_models_api(&serde_json::json!({"id": "claude-x-9"})).unwrap();
        assert_eq!(bare.capabilities.unwrap(), unknown_claude());
        assert!(from_models_api(&serde_json::json!({})).is_none());
    }
}
