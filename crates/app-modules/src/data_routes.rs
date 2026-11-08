//! „Gdzie trafiają dane” (C4.1): trasy z rejestru zgodności (`compliance`) w kształcie UI —
//! jurysdykcja, tagi prywatności, ryzyko trenowania, status i decyzje dla sesji zwykłej
//! i prywatnej liczone tą samą funkcją co w Routerze. Tylko odczyt.

use app_api::dto::{DataRoute, DataRoutesView};
use compliance_contract::{Compliance, RouteMode, RouteStatus, SessionTag, TrainingRisk};

fn mode(m: RouteMode) -> &'static str {
    match m {
        RouteMode::CliHeadless => "cli-p",
        RouteMode::AgentSdk => "sdk",
        RouteMode::Api => "api",
    }
}

fn status(s: RouteStatus) -> &'static str {
    match s {
        RouteStatus::Green => "green",
        RouteStatus::Grey => "grey",
        RouteStatus::Forbidden => "forbidden",
    }
}

fn training(t: TrainingRisk) -> &'static str {
    match t {
        TrainingRisk::NoTrain => "no_train",
        TrainingRisk::MayTrain => "may_train",
        TrainingRisk::Unknown => "unknown",
    }
}

/// Widok wszystkich tras (włączone pierwsze, potem po dostawcy i identyfikatorze).
pub fn view(c: &dyn Compliance) -> DataRoutesView {
    let mut routes: Vec<DataRoute> = c
        .views()
        .into_iter()
        .map(|v| {
            let standard = c.route_allowed(&v.id, SessionTag::Standard);
            let private = c.route_allowed(&v.id, SessionTag::Private);
            DataRoute {
                id: v.id.to_string(),
                provider: v.provider.clone(),
                mode: mode(v.mode).into(),
                jurisdiction: v.tags.jurisdiction.codes().map(str::to_owned).collect(),
                privacy: v
                    .tags
                    .privacy
                    .iter()
                    .map(|t| t.as_str().to_owned())
                    .collect(),
                training: training(v.tags.training_risk()).into(),
                status: status(v.effective.status).into(),
                stale: v.effective.stale,
                unverified: v.effective.unverified,
                enabled: v.enabled,
                standard_allowed: standard.allowed,
                private_allowed: private.allowed,
                private_reason: private.reason.to_string(),
            }
        })
        .collect();
    routes.sort_by(|a, b| (!a.enabled, &a.provider, &a.id).cmp(&(!b.enabled, &b.provider, &b.id)));
    DataRoutesView { routes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compliance_contract::{
        PrivacyTag, ProviderApiStatus, ProviderPolicyInput, Registry, RouteTags,
    };

    const REGISTRY: &str = include_str!("../../../docs/compliance/compliance-registry.json");

    #[test]
    fn real_registry_and_private_session_rules() {
        let tags = |t: PrivacyTag, j: &str| RouteTags {
            privacy: [t].into(),
            jurisdiction: j.parse().unwrap_or_default(),
        };
        let catalog = vec![
            ProviderPolicyInput {
                provider: "deepseek".into(),
                tags: tags(PrivacyTag::CnMayTrain, "CN"),
                api_status: ProviderApiStatus::Green,
            },
            ProviderPolicyInput {
                provider: "przyklad-ue".into(),
                tags: tags(PrivacyTag::Eu, "EU"),
                api_status: ProviderApiStatus::Green,
            },
        ];
        let today = chrono::Local::now().date_naive();
        let svc = compliance_fake::FakeCompliance::new(
            Registry::from_json(REGISTRY).unwrap(),
            catalog,
            today,
        );
        let v = view(&svc);
        assert!(v.routes.len() > 2, "rejestr + katalog");
        for r in &v.routes {
            assert!(["api", "cli-p", "sdk"].contains(&r.mode.as_str()), "{r:?}");
            assert!(["green", "grey", "forbidden"].contains(&r.status.as_str()));
            assert!(!r.private_reason.is_empty());
            assert!(!r.private_allowed || r.standard_allowed, "{r:?}");
        }
        let cn = v.routes.iter().find(|r| r.provider == "deepseek").unwrap();
        assert_eq!(cn.jurisdiction, vec!["CN".to_owned()]);
        assert_eq!(cn.training, "may_train");
        assert!(cn.standard_allowed && !cn.private_allowed, "{cn:?}");
        let eu = v
            .routes
            .iter()
            .find(|r| r.provider == "przyklad-ue")
            .unwrap();
        assert!(eu.private_allowed, "{eu:?}");
        let first_disabled = v.routes.iter().position(|r| !r.enabled);
        let last_enabled = v.routes.iter().rposition(|r| r.enabled);
        if let (Some(d), Some(e)) = (first_disabled, last_enabled) {
            assert!(e < d, "włączone przed wyłączonymi");
        }
    }
}
