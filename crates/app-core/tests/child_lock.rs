//! `permissions_child_lock` (plan 50 §4.4, Z.6): profil Dziecko z Sędzim w procesie — PIN
//! rodzica, kłódka obniża tryb globalny do „Ręcznie” i odrzuca podniesienia, zły PIN liczy próby.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::dto::{AutonomyLevel, ChildLockRequest, ChildLockState};
use common::harness;

fn change(json: serde_json::Value) -> Option<ChildLockRequest> {
    Some(serde_json::from_value(json).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn parent_pin_locks_and_opens_the_modes() {
    let h = harness().await;
    let core = &h.core;
    assert_eq!(
        core.permissions_child_lock(None).await.unwrap(),
        ChildLockState {
            attempts_left: 5,
            ..ChildLockState::default()
        }
    );
    let bad = core
        .permissions_child_lock(change(
            serde_json::json!({ "op": "set_pin", "old": null, "new": "12" }),
        ))
        .await;
    assert!(bad.is_err(), "PIN ma 4–8 cyfr");
    core.permissions_child_lock(change(
        serde_json::json!({ "op": "set_pin", "old": null, "new": "2468" }),
    ))
    .await
    .unwrap();
    let on = core
        .permissions_child_lock(change(
            serde_json::json!({ "op": "enable", "pin": "2468", "on": true }),
        ))
        .await
        .unwrap();
    assert!(on.enabled && on.pin_set && !on.open);
    assert_eq!(
        core.permissions_get(None).await.unwrap().global,
        AutonomyLevel::L1,
        "kłódka: najwyżej Ręcznie"
    );
    let refused = core
        .permissions_request_level(AutonomyLevel::L4, None)
        .await
        .unwrap_err();
    assert!(refused.message.contains("kłódka"), "{}", refused.message);

    let wrong = core
        .permissions_child_lock(change(
            serde_json::json!({ "op": "open", "pin": "1111", "minutes": 60 }),
        ))
        .await
        .unwrap_err();
    assert!(wrong.message.contains("PIN"), "{}", wrong.message);
    assert_eq!(
        core.permissions_child_lock(None)
            .await
            .unwrap()
            .attempts_left,
        4
    );

    let open = core
        .permissions_child_lock(change(
            serde_json::json!({ "op": "open", "pin": "2468", "minutes": 60 }),
        ))
        .await
        .unwrap();
    assert!(open.open && open.open_until_ms.is_some());
    assert_eq!(
        core.permissions_get(None).await.unwrap().global,
        AutonomyLevel::L3
    );
    let closed = core
        .permissions_child_lock(change(serde_json::json!({ "op": "close" })))
        .await
        .unwrap();
    assert!(!closed.open);
    let off = core
        .permissions_child_lock(change(
            serde_json::json!({ "op": "enable", "pin": "2468", "on": false }),
        ))
        .await
        .unwrap();
    assert!(!off.enabled);
    assert_eq!(
        core.permissions_get(None).await.unwrap().global,
        AutonomyLevel::L3
    );
}
