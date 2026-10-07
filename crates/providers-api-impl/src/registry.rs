//! Rejestr możliwości modeli jednego dostawcy: konfiguracja > Models API > tabela znanych > ostrożne domyślne.

use std::collections::BTreeMap;
use std::sync::RwLock;

use providers_contract::{
    ChatRequest, Cost, CostEstimate, InterruptionRendering, ModelCapabilities, ModelInfo,
    ProviderCapabilities, Usage,
};

use lib_openai_compat::ProviderProfile;

/// Funkcja „znanych modeli" adaptera.
pub(crate) type KnownModels = fn(&str) -> Option<ModelCapabilities>;

/// Rejestr możliwości.
#[derive(Debug)]
pub(crate) struct ModelRegistry {
    configured: BTreeMap<String, ModelCapabilities>,
    discovered: RwLock<BTreeMap<String, ModelCapabilities>>,
    known: KnownModels,
    fallback: ModelCapabilities,
}

impl ModelRegistry {
    pub fn new(
        configured: BTreeMap<String, ModelCapabilities>,
        known: KnownModels,
        fallback: ModelCapabilities,
    ) -> Self {
        Self {
            configured,
            discovered: RwLock::default(),
            known,
            fallback,
        }
    }

    fn discovered(&self) -> BTreeMap<String, ModelCapabilities> {
        self.discovered
            .read()
            .map(|g| g.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    /// Możliwości modelu.
    pub fn caps(&self, model: &str) -> ModelCapabilities {
        if let Some(c) = self.configured.get(model) {
            return c.clone();
        }
        if let Some(c) = self.discovered().get(model) {
            return c.clone();
        }
        (self.known)(model).unwrap_or_else(|| self.fallback.clone())
    }

    /// Wszystkie znane modele (skonfigurowane + wykryte).
    pub fn all(&self) -> BTreeMap<String, ModelCapabilities> {
        let mut all = self.discovered();
        all.extend(self.configured.iter().map(|(k, v)| (k.clone(), v.clone())));
        all
    }

    /// Zapamiętuje możliwości wykryte przez Models API.
    pub fn learn(&self, models: &[ModelInfo]) {
        let mut guard = self.discovered.write().unwrap_or_else(|p| p.into_inner());
        for m in models {
            if let Some(c) = &m.capabilities {
                guard.insert(m.id.clone(), c.clone());
            }
        }
    }

    /// Opis możliwości dostawcy.
    pub fn provider_capabilities(&self, profile: &ProviderProfile) -> ProviderCapabilities {
        ProviderCapabilities {
            provider: profile.id.clone(),
            default_model: profile.default_model.clone(),
            models: self.all(),
            interruption: InterruptionRendering::AppendNote,
            privacy: profile.privacy.clone(),
        }
    }

    /// Oszacowanie kosztu z cennika profilu.
    pub fn estimate_cost(
        &self,
        profile: &ProviderProfile,
        req: &ChatRequest,
    ) -> Option<CostEstimate> {
        let pricing = profile.pricing.get(&req.model)?;
        let max_out = req
            .params
            .max_tokens
            .or(self.caps(&req.model).max_output_tokens)
            .unwrap_or(profile.default_max_tokens);
        Some(pricing.estimate(req, u64::from(max_out)))
    }
}

/// Koszt z cennika profilu.
pub(crate) fn cost(profile: &ProviderProfile, model: &str, usage: &Usage) -> Option<Cost> {
    profile.pricing.get(model).map(|p| p.cost(usage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use providers_contract::{Message, Pricing, ThinkingSupport};

    fn known(m: &str) -> Option<ModelCapabilities> {
        (m == "k").then(|| ModelCapabilities {
            thinking: ThinkingSupport::AlwaysOn,
            ..ModelCapabilities::default()
        })
    }

    #[test]
    fn precedence_and_learning() {
        let configured = BTreeMap::from([(
            "c".to_owned(),
            ModelCapabilities {
                tools: true,
                ..ModelCapabilities::default()
            },
        )]);
        let reg = ModelRegistry::new(configured, known, ModelCapabilities::default());
        assert!(reg.caps("c").tools);
        assert_eq!(reg.caps("k").thinking, ThinkingSupport::AlwaysOn);
        assert_eq!(reg.caps("x"), ModelCapabilities::default());
        reg.learn(&[ModelInfo {
            id: "k".into(),
            display_name: None,
            created: None,
            capabilities: Some(ModelCapabilities {
                vision: true,
                ..ModelCapabilities::default()
            }),
        }]);
        assert!(reg.caps("k").vision, "Models API wygrywa z tabelą znanych");
        assert_eq!(reg.all().len(), 2);
        let mut profile = ProviderProfile::new("p");
        profile.pricing.insert(
            "c".into(),
            Pricing {
                input_per_mtok_usd: 1.0,
                output_per_mtok_usd: 1.0,
                cache_read_per_mtok_usd: None,
                cache_write_per_mtok_usd: None,
            },
        );
        let req = ChatRequest::new("c", vec![Message::user_text("abcd")]);
        let est = reg.estimate_cost(&profile, &req).unwrap();
        assert_eq!(est.max_output_tokens, 8_192);
        assert!(cost(&profile, "c", &Usage::default()).is_some());
        assert!(
            reg.estimate_cost(&profile, &ChatRequest::new("x", vec![]))
                .is_none()
        );
        assert_eq!(reg.provider_capabilities(&profile).provider.as_str(), "p");
    }
}
