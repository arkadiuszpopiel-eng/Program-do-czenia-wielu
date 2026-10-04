//! Rekordy biblioteki wtyczek → DTO strony „Wtyczki” (karta: zdolności z zakresami, limity,
//! narzędzia, hash przejrzanej wersji, propozycja R2) i błędy biblioteki → `AppError`.

use app_api::AppError;
use app_api::dto::{
    PluginCapabilityView, PluginInfo, PluginLimitsView, PluginOrigin, PluginR2View,
    PluginStateView, PluginToolView, iso,
};
use plugin_runtime_contract::{
    PluginError, PluginManifest, PluginRecord, PluginSource, PluginState, R2Change,
};

/// Czas (ms od epoki) → ISO 8601.
pub fn iso_ms(ms: u64) -> String {
    let ms = i64::try_from(ms).unwrap_or(i64::MAX);
    chrono::DateTime::from_timestamp_millis(ms).map_or_else(String::new, iso)
}

fn state(s: PluginState) -> PluginStateView {
    match s {
        PluginState::Proposed => PluginStateView::Proposed,
        PluginState::Installed => PluginStateView::Installed,
        PluginState::Disabled => PluginStateView::Disabled,
        PluginState::Rejected => PluginStateView::Rejected,
        PluginState::Superseded => PluginStateView::Superseded,
    }
}

fn origin(s: &PluginSource) -> PluginOrigin {
    match s {
        PluginSource::User => PluginOrigin::User,
        PluginSource::Improver { .. } => PluginOrigin::Improver,
        PluginSource::Import { external: false } => PluginOrigin::Import,
        PluginSource::Import { external: true } => PluginOrigin::External,
    }
}

fn capabilities(m: &PluginManifest) -> Vec<PluginCapabilityView> {
    m.capabilities
        .iter()
        .map(|c| {
            let text = c.to_string();
            let scope = text
                .strip_prefix(c.family())
                .and_then(|s| s.strip_prefix('('))
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(&text)
                .to_owned();
            PluginCapabilityView {
                family: c.family().to_owned(),
                scope,
            }
        })
        .collect()
}

fn limits(m: &PluginManifest) -> PluginLimitsView {
    let l = m.limits;
    PluginLimitsView {
        memory_mib: l.memory_mib,
        fuel_per_call: l.fuel_per_call,
        wall_ms: l.wall_ms,
        max_input_bytes: l.max_input_bytes,
        max_output_bytes: l.max_output_bytes,
        max_host_calls: l.max_host_calls,
    }
}

/// Zmiana R2 → DTO.
pub fn r2(c: &R2Change) -> PluginR2View {
    PluginR2View {
        key: c.key.clone(),
        value: c.review_hash.clone(),
        from_version: c.from_version.as_ref().map(ToString::to_string),
        added_capabilities: c.added_capabilities.clone(),
    }
}

/// Rekord → karta wtyczki.
pub fn info(r: &PluginRecord, r2: Option<PluginR2View>) -> PluginInfo {
    let m = &r.manifest;
    let side_effects = m.has_side_effects();
    PluginInfo {
        id: m.id.to_string(),
        version: m.version.to_string(),
        author: m.author.clone(),
        description: m.description.clone(),
        state: state(r.state),
        origin: origin(&r.source),
        wasm_sha256: m.wasm_sha256.clone(),
        review_hash: r.review_hash.clone(),
        capabilities: capabilities(m),
        limits: limits(m),
        tools: m
            .tools
            .iter()
            .map(|t| PluginToolView {
                name: PluginManifest::tool_name(t),
                title: t.title.clone(),
                description: t.description.clone(),
                mutating: t.mutating || side_effects,
            })
            .collect(),
        side_effects,
        proposed_at: iso_ms(r.proposed_at_ms),
        decided_at: r.decided_at_ms.map(iso_ms),
        r2,
    }
}

/// Błąd biblioteki → błąd komendy (komunikat po polsku z kontraktu).
pub fn error(e: PluginError) -> AppError {
    let text = format!("Wtyczki: {e}");
    match e {
        PluginError::NotFound(_) => AppError::not_found(text),
        PluginError::HashMismatch
        | PluginError::ApprovalChannel
        | PluginError::ForbiddenCapability(_) => AppError::forbidden(text),
        PluginError::Store(_) => AppError::storage(text),
        _ => AppError::invalid(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_runtime_contract::{PluginApproval, samples};
    use safety_broker_contract::{Capability, HostPattern};

    fn record(state: PluginState) -> PluginRecord {
        let mut m = samples::manifest("licznik", "1.2.0", b"wasm");
        m.capabilities = vec![Capability::NetEgress(
            HostPattern::parse("api.example.com").unwrap(),
        )];
        PluginRecord {
            review_hash: "ab".repeat(32),
            manifest: m,
            source: PluginSource::Import { external: true },
            state,
            approval: Some(PluginApproval::ui("x")),
            proposed_at_ms: 1_790_000_000_000,
            decided_at_ms: None,
        }
    }

    #[test]
    fn card_shows_capabilities_limits_tools_and_hash() {
        let v = info(&record(PluginState::Installed), None);
        assert_eq!(v.state, PluginStateView::Installed);
        assert_eq!(v.origin, PluginOrigin::External);
        assert_eq!(v.capabilities[0].family, "net.egress");
        assert_eq!(v.capabilities[0].scope, "api.example.com");
        assert_eq!(v.tools[0].name, "plugin_word_count");
        assert!(
            v.side_effects && v.tools[0].mutating,
            "sieć = skutki uboczne"
        );
        assert_eq!(v.limits.memory_mib, 16);
        assert_eq!(v.review_hash.len(), 64);
        assert!(v.proposed_at.starts_with("2026-"));
        assert!(v.decided_at.is_none());
    }

    #[test]
    fn errors_map_to_codes() {
        use app_api::ErrorCode;
        assert_eq!(
            error(PluginError::NotFound("x".into())).code,
            ErrorCode::NotFound
        );
        assert_eq!(error(PluginError::HashMismatch).code, ErrorCode::Forbidden);
        assert_eq!(
            error(PluginError::Store("io".into())).code,
            ErrorCode::Storage
        );
        assert_eq!(
            error(PluginError::Invalid("x".into())).code,
            ErrorCode::InvalidInput
        );
        assert!(iso_ms(0).starts_with("1970-01-01"));
    }
}
