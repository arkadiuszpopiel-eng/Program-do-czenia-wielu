//! ACC-F1-router-01 (property-based): dla losowych polityk i stanów dostawców decyzja spełnia
//! wszystkie ograniczenia (prywatność, jurysdykcja, zgodność, możliwości, klucz, obwód) w 100%
//! i jest dokładnie filtrem polityki wg niezależnej referencji (kolejność zachowana).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use compliance_contract::{
    Compliance, PrivacyTag, ProviderApiStatus, ProviderPolicyInput, RouteId, SessionTag,
};
use proptest::prelude::*;
use providers_contract::{ProviderErrorKind, ProviderPrivacy, RequestPrivacy, check_privacy};
use providers_fake::FAKE_MODEL;
use router_contract::{
    Candidate, CapabilityNeeds, Constraints, Outcome, RouteKind, RoutePolicy, Router, TaskClass,
};
use router_impl::RouterCore;
use support::{Switchable, caps, catalog_entry, compliance, fake, unconfigured};

#[derive(Debug, Clone)]
struct Spec {
    local: bool,
    tag: usize,
    status: usize,
    configured: bool,
    vision: bool,
    tools: bool,
    broken: bool,
}

const TAGS: [(Option<PrivacyTag>, &str, &str); 4] = [
    (Some(PrivacyTag::Eu), "eu", "EU"),
    (Some(PrivacyTag::CnMayTrain), "cn-may-train", "CN"),
    (Some(PrivacyTag::Sg), "sg", "SG"),
    (None, "unknown", "unknown"),
];

const STATUSES: [ProviderApiStatus; 3] = [
    ProviderApiStatus::Green,
    ProviderApiStatus::Unverified,
    ProviderApiStatus::Forbidden,
];

fn spec() -> impl Strategy<Value = Spec> {
    (
        any::<bool>(),
        0usize..4,
        0usize..3,
        prop::bool::weighted(0.8),
        any::<bool>(),
        any::<bool>(),
        prop::bool::weighted(0.2),
    )
        .prop_map(
            |(local, tag, status, configured, vision, tools, broken)| Spec {
                local,
                tag,
                status,
                configured,
                vision,
                tools,
                broken,
            },
        )
}

fn id(i: usize) -> String {
    format!("p{i}")
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 500, ..ProptestConfig::default() })]

    #[test]
    fn decision_satisfies_all_constraints(
        specs in prop::collection::vec(spec(), 1..7),
        order in prop::collection::vec(any::<prop::sample::Index>(), 1..10),
        private in any::<bool>(),
        eu_only in any::<bool>(),
        need_vision in any::<bool>(),
        need_tools in any::<bool>(),
    ) {
        let entries: Vec<ProviderPolicyInput> = specs
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.local)
            .map(|(i, s)| {
                let (tag, _, jur) = TAGS[s.tag];
                let mut e = catalog_entry(&id(i), &tag.into_iter().collect::<Vec<_>>(), jur);
                e.api_status = STATUSES[s.status];
                e
            })
            .collect();
        let comp = Arc::new(compliance(entries));
        let core = RouterCore::new(Some(comp.clone()), None);
        for (i, s) in specs.iter().enumerate() {
            let (_, tag, jur) = TAGS[s.tag];
            let f = fake(&id(i))
                .with_privacy(ProviderPrivacy::new(tag, jur))
                .with_model(FAKE_MODEL, caps(s.vision, s.tools));
            let health = (!s.configured).then(unconfigured).flatten();
            let kind = if s.local { RouteKind::Local } else { RouteKind::Api };
            core.register(Switchable::new(f, health), kind);
            if s.broken {
                for _ in 0..3 {
                    core.report(
                        &Candidate::new(id(i), FAKE_MODEL),
                        Outcome::Failed { kind: ProviderErrorKind::Network },
                    );
                }
            }
        }
        let list: Vec<Candidate> = {
            let mut l: Vec<Candidate> = Vec::new();
            for ix in &order {
                let c = Candidate::new(id(ix.index(specs.len())), FAKE_MODEL);
                if !l.contains(&c) {
                    l.push(c);
                }
            }
            l
        };
        let mut policy = RoutePolicy::defaults(None, &[]);
        policy.classes.insert(TaskClass::Conversation, list.clone());
        core.set_policy(Some(policy));
        let constraints = Constraints {
            session: if private { SessionTag::Private } else { SessionTag::Standard },
            jurisdiction_allow: if eu_only { vec!["EU".into()] } else { vec![] },
            needs: CapabilityNeeds { vision: need_vision, tools: need_tools, ..CapabilityNeeds::default() },
            ..Constraints::default()
        };
        // Niezależna referencja: które kandydatury są dozwolone.
        let ok = |c: &Candidate| -> bool {
            let i: usize = c.provider.as_str()[1..].parse().unwrap();
            let s = &specs[i];
            if !s.configured || s.broken { return false; }
            if (need_vision && !s.vision) || (need_tools && !s.tools) { return false; }
            if s.local { return true; }
            let route = RouteId::api(c.provider.as_str()).unwrap();
            if !comp.route_allowed(&route, constraints.session).allowed { return false; }
            let (_, tag, jur) = TAGS[s.tag];
            if eu_only && jur != "EU" { return false; }
            let req = RequestPrivacy {
                tag: if private { providers_contract::PrivacyTag::Private } else { providers_contract::PrivacyTag::Normal },
                jurisdiction_allow: constraints.jurisdiction_allow.clone(),
            };
            check_privacy(&req, &ProviderPrivacy::new(tag, jur)).is_ok()
        };
        let expected: Vec<Candidate> = list.iter().filter(|c| ok(c)).cloned().collect();
        match core.route(TaskClass::Conversation, &constraints, None) {
            Ok(d) => {
                let got: Vec<Candidate> = d.targets().cloned().collect();
                prop_assert_eq!(&got, &expected);
                prop_assert_eq!(d.rejected.len() + got.len(), list.len());
                for c in &got {
                    let i: usize = c.provider.as_str()[1..].parse().unwrap();
                    let s = &specs[i];
                    // Sesja prywatna nigdy nie trafia do CN / „może trenować" / nieznanych.
                    if private && !s.local {
                        let tag = TAGS[s.tag].1;
                        prop_assert!(tag != "cn-may-train" && tag != "unknown", "prywatna → {tag}");
                    }
                }
            }
            Err(router_contract::RouteError::NoRoute { rejected, .. }) => {
                prop_assert!(expected.is_empty(), "odrzucono dozwolonych: {:?}", expected);
                prop_assert_eq!(rejected.len(), list.len());
            }
        }
    }
}
