//! Testy własności (ACC-F3-safety-broker-03): potomek nigdy szerszy niż rodzic (ścieżki
//! z normalizacją Windows, hosty), zmiana dowolnego bajtu tokenu zawsze wykrywalna.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::PathEnv;
use compliance_contract::deny::normalize;
use proptest::prelude::*;
use safety_broker_contract::{
    BootId, CapToken, Capability, Holder, HostPattern, PathScope, SecretId, TokenId,
};

const COMPS: &[&str] = &[
    "users", "Users", "ala", "ALA", "docs", "Docs", "..", ".", "a.txt", "a.txt.", "b~1", "windows",
    "x:stream", "proj", "",
];

fn raw_path() -> impl Strategy<Value = String> {
    (
        prop_oneof![
            Just("C:"),
            Just("c:"),
            Just("D:"),
            Just(r"\\?\C:"),
            Just(r"\\srv\share")
        ],
        proptest::collection::vec(proptest::sample::select(COMPS), 0..6),
        prop_oneof![Just("\\"), Just("/")],
    )
        .prop_map(|(root, comps, sep)| {
            let mut s = root.to_owned();
            for c in comps {
                s.push_str(sep);
                s.push_str(c);
            }
            s
        })
}

fn scope() -> impl Strategy<Value = PathScope> {
    (raw_path(), any::<bool>()).prop_filter_map("bezwzględna", |(p, sub)| {
        PathScope::new(&p, sub, &PathEnv::new()).ok()
    })
}

const LABELS: &[&str] = &[
    "example", "com", "a", "b", "evil", "net", "api", "claude", "ai",
];

fn host() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::sample::select(LABELS), 1..4).prop_map(|l| l.join("."))
}

fn pattern() -> impl Strategy<Value = HostPattern> {
    (host(), any::<bool>()).prop_filter_map("poprawny wzorzec", |(h, sub)| {
        HostPattern::parse(&if sub { format!("*.{h}") } else { h }).ok()
    })
}

fn token() -> impl Strategy<Value = CapToken> {
    (
        any::<u64>(),
        proptest::option::of(any::<u64>()),
        scope(),
        any::<[u8; 16]>(),
        any::<u32>(),
        any::<(u64, u64)>(),
        "[a-z0-9]{1,12}",
        proptest::option::of("[a-z]{1,8}"),
        any::<[u8; 32]>(),
    )
        .prop_map(
            |(id, parent, s, boot, epoch, (a, b), session, agent, mac)| CapToken {
                id: TokenId(id),
                parent: parent.map(TokenId),
                cap: Capability::FsWrite(s),
                holder: Holder {
                    session: core_bus_contract::SessionId::new(session),
                    agent: agent.map(core_bus_contract::AgentId::new),
                    role: Some("rola".into()),
                },
                boot: BootId(boot),
                key_epoch: epoch,
                issued_at_ms: a.min(b),
                expires_at_ms: a.max(b),
                mac,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    #[test]
    fn path_child_never_wider_than_parent(child in scope(), parent in scope(), probe in raw_path()) {
        if child.is_subset_of(&parent) {
            let p = normalize(&probe, &PathEnv::new());
            prop_assert!(!child.contains(&p) || parent.contains(&p), "{child} ⊆ {parent}, {probe}");
            prop_assert!(parent.subtree() || !child.subtree());
        }
    }

    #[test]
    fn subset_is_reflexive_and_transitive(a in scope(), b in scope(), c in scope()) {
        prop_assert!(a.is_subset_of(&a));
        if a.is_subset_of(&b) && b.is_subset_of(&c) {
            prop_assert!(a.is_subset_of(&c));
        }
    }

    #[test]
    fn host_child_never_wider_than_parent(child in pattern(), parent in pattern(), probe in host()) {
        if child.is_subset_of(&parent) {
            prop_assert!(!child.matches(&probe) || parent.matches(&probe), "{child} ⊆ {parent}, {probe}");
        }
    }

    #[test]
    fn capability_subset_keeps_family(a in scope(), b in scope(), id in "[a-z]{1,6}") {
        let caps = [
            Capability::FsRead(a.clone()),
            Capability::FsWrite(b.clone()),
            Capability::ShellExec(a),
            Capability::SecretsRead(SecretId::parse(&id).unwrap()),
        ];
        for x in &caps {
            for y in &caps {
                if x.is_subset_of(y) {
                    prop_assert_eq!(x.family(), y.family());
                }
            }
        }
    }

    #[test]
    fn any_byte_change_is_detected(t in token(), idx in any::<prop::sample::Index>(), mask in 1u8..=255) {
        let wire = t.to_wire();
        let i = idx.index(wire.len());
        let mut bad = wire.clone();
        bad[i] ^= mask;
        match CapToken::from_wire(&bad) {
            Err(_) => {}
            Ok(parsed) => prop_assert!(
                parsed.signing_bytes() != t.signing_bytes() || parsed.mac != t.mac,
                "zmiana bajtu {} niewykrywalna", i
            ),
        }
    }

    #[test]
    fn wire_round_trip(t in token()) {
        prop_assert_eq!(CapToken::from_wire(&t.to_wire()).unwrap(), t);
    }
}
