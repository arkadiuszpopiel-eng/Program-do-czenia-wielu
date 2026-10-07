//! Property-based: strażnik nigdy nie przepuszcza kluczy spoza listy ani obszarów zakazanych.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use improver_contract::{
    ChangeTarget, FORBIDDEN_PREFIXES, FORBIDDEN_SEGMENTS, IMPROVABLE, Violation, assess,
    pattern_matches,
};
use proptest::prelude::*;
use serde_json::json;

fn seg() -> impl Strategy<Value = String> {
    "[a-z0-9_]{1,12}"
}

proptest! {
    #[test]
    fn forbidden_prefix_never_passes(p in 0..FORBIDDEN_PREFIXES.len(), rest in proptest::collection::vec(seg(), 1..4), v in any::<i32>()) {
        let key = format!("{}.{}", FORBIDDEN_PREFIXES[p], rest.join("."));
        let target = ChangeTarget::Config { key: key.clone(), value: json!(v) };
        prop_assert!(assess(&target, None).is_err(), "{}", key);
    }

    #[test]
    fn forbidden_segment_inside_allowed_pattern_never_passes(r in 0..IMPROVABLE.len(), s in 0..FORBIDDEN_SEGMENTS.len()) {
        let key = IMPROVABLE[r].pattern.replace('*', FORBIDDEN_SEGMENTS[s]);
        let target = ChangeTarget::Config { key: key.clone(), value: json!("x") };
        if key.contains(FORBIDDEN_SEGMENTS[s]) {
            prop_assert!(assess(&target, None).is_err(), "{}", key);
        }
    }

    #[test]
    fn keys_outside_allowlist_are_not_improvable(segs in proptest::collection::vec(seg(), 1..5)) {
        let key = segs.join(".");
        let allowed = IMPROVABLE.iter().any(|r| pattern_matches(r.pattern, &key));
        let r = assess(&ChangeTarget::Config { key: key.clone(), value: json!(true) }, None);
        if !allowed {
            prop_assert!(r.is_err(), "{}", key);
        }
    }

    #[test]
    fn files_and_code_always_rejected(path in "[a-zA-Z0-9_/.-]{1,40}") {
        let file = ChangeTarget::File { path: path.clone(), content: String::new() };
        let is_forbidden = matches!(assess(&file, None), Err(Violation::Forbidden { .. }));
        prop_assert!(is_forbidden);
        let code = ChangeTarget::Code { path, diff: String::new() };
        let is_ring = matches!(assess(&code, None), Err(Violation::RingNotAllowed { .. }));
        prop_assert!(is_ring);
    }
}
