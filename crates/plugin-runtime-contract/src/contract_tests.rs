//! Współdzielone testy kontraktowe (feature `contract-tests`) dla `-impl` i `-fake`.
//! `wasm(n)` dostarcza poprawny moduł licznika słów (impl: komponent z WAT; fake: dowolne
//! bajty z nagłówkiem komponentu) — `n` zmienia bajty (inna treść, ten sam kontrakt).

use semver::Version;

use crate::samples::manifest;
use crate::{
    ApprovalOrigin, LoadError, PluginApproval, PluginError, PluginId, PluginSource, PluginState,
    Plugins, TOOL_PREFIX,
};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn v(s: &str) -> Version {
    ok(Version::parse(s))
}

fn approval(origin: ApprovalOrigin, hash: &str) -> PluginApproval {
    PluginApproval {
        origin,
        reviewed_hash: hash.to_owned(),
        signature: None,
    }
}

/// Cykl życia: propozycja → zatwierdzenie (UI + hash) → aktualizacja → R2 → wyłączenie →
/// włączenie → odrzucenie → konflikt narzędzi → usunięcie.
pub async fn lifecycle(p: &dyn Plugins, wasm: &dyn Fn(u8) -> Vec<u8>) {
    let id = PluginId::new("licznik");
    assert!(p.tools().is_empty() && p.tool_catalog().is_empty());

    // Hash modułu niezgodny z manifestem → odmowa przed czymkolwiek.
    let mut bad = manifest("licznik", "1.0.0", &wasm(1));
    bad.wasm_sha256 = crate::sha256_hex(b"inne bajty");
    let err = p.propose(bad, wasm(1), PluginSource::User).await;
    assert!(
        matches!(err, Err(PluginError::Load(LoadError::HashMismatch { .. }))),
        "{err:?}"
    );

    // Propozycja: niewidoczna dla agentek.
    let m1 = manifest("licznik", "1.0.0", &wasm(1));
    let r1 = ok(p.propose(m1.clone(), wasm(1), PluginSource::User).await);
    assert_eq!(r1.state, PluginState::Proposed);
    assert!(p.tools().is_empty() && p.installed(&id).is_none());

    // Kanał głosowy/tekstowy i zły hash — odmowa.
    for origin in [ApprovalOrigin::Voice, ApprovalOrigin::Text] {
        let e = p
            .approve(&id, &v("1.0.0"), approval(origin, &r1.review_hash))
            .await;
        assert_eq!(e, Err(PluginError::ApprovalChannel));
    }
    let e = p
        .approve(
            &id,
            &v("1.0.0"),
            PluginApproval::ui(crate::sha256_hex(b"x")),
        )
        .await;
    assert_eq!(e, Err(PluginError::HashMismatch));

    // Zatwierdzenie → narzędzie w rejestrze.
    let inst = ok(p
        .approve(&id, &v("1.0.0"), PluginApproval::ui(&r1.review_hash))
        .await);
    assert_eq!(inst.state, PluginState::Installed);
    let catalog = p.tool_catalog();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].name, format!("{TOOL_PREFIX}word_count"));
    assert_eq!(p.tools().len(), 1);

    // Ta sama wersja: ta sama treść = ten sam rekord, inna treść = błąd.
    let same = ok(p.propose(m1.clone(), wasm(1), PluginSource::User).await);
    assert_eq!(same.review_hash, r1.review_hash);
    let mut changed = m1.clone();
    changed.description = "Zmieniony opis po przeglądzie właściciela.".into();
    let e = p.propose(changed, wasm(1), PluginSource::User).await;
    assert_eq!(e, Err(PluginError::VersionExists("1.0.0".into())));

    // Aktualizacja = nowa wersja + ponowne zatwierdzenie.
    let r2 = ok(p
        .propose(
            manifest("licznik", "1.1.0", &wasm(2)),
            wasm(2),
            PluginSource::User,
        )
        .await);
    assert_eq!(
        p.installed(&id).map(|r| r.manifest.version),
        Some(v("1.0.0"))
    );
    ok(
        p.approve(&id, &v("1.1.0"), PluginApproval::ui(&r2.review_hash))
            .await,
    );
    assert_eq!(
        p.installed(&id).map(|r| r.manifest.version),
        Some(v("1.1.0"))
    );
    let old = p
        .list()
        .into_iter()
        .find(|r| r.manifest.version == v("1.0.0"));
    assert_eq!(old.map(|r| r.state), Some(PluginState::Superseded));
    let e = p
        .propose(
            manifest("licznik", "1.0.5", &wasm(3)),
            wasm(3),
            PluginSource::User,
        )
        .await;
    assert_eq!(e, Err(PluginError::NotNewer("1.0.5".into())));

    // R2 (Ulepszacz): zmiana z hashem jako wartością; wdrożenie tylko tego hasha.
    let src = PluginSource::Improver {
        proposal: "P-7".into(),
    };
    let r3 = ok(p
        .propose(manifest("licznik", "1.2.0", &wasm(3)), wasm(3), src)
        .await);
    let change = ok(p.r2_proposal(&id, &v("1.2.0")));
    assert_eq!(change.review_hash, r3.review_hash);
    assert_eq!(change.from_version, Some(v("1.1.0")));
    let e = p
        .deploy_r2(
            &change.key,
            &change.value(),
            PluginApproval::ui(&r2.review_hash),
        )
        .await;
    assert_eq!(e, Err(PluginError::HashMismatch));
    let dep = ok(p
        .deploy_r2(
            &change.key,
            &change.value(),
            PluginApproval::ui(&r3.review_hash),
        )
        .await);
    assert_eq!(dep.manifest.version, v("1.2.0"));

    // Wyłączenie i ponowne włączenie (z ponownym zatwierdzeniem).
    ok(p.disable(&id).await);
    assert!(p.tools().is_empty() && p.installed(&id).is_none());
    let e = p.enable(&id, PluginApproval::ui(&r1.review_hash)).await;
    assert_eq!(e, Err(PluginError::HashMismatch));
    ok(p.enable(&id, PluginApproval::ui(&r3.review_hash)).await);
    assert_eq!(p.tools().len(), 1);

    // Odrzucenie.
    let r4 = ok(p
        .propose(
            manifest("licznik", "2.0.0", &wasm(4)),
            wasm(4),
            PluginSource::User,
        )
        .await);
    ok(p.reject(&id, &v("2.0.0")).await);
    let e = p
        .approve(&id, &v("2.0.0"), PluginApproval::ui(&r4.review_hash))
        .await;
    assert_eq!(e, Err(PluginError::WrongState(PluginState::Rejected)));

    // Konflikt nazw narzędzi z inną wtyczką.
    let e = p
        .propose(
            manifest("inny-licznik", "1.0.0", &wasm(5)),
            wasm(5),
            PluginSource::User,
        )
        .await;
    assert_eq!(
        e,
        Err(PluginError::ToolConflict(format!(
            "{TOOL_PREFIX}word_count"
        )))
    );

    // Usunięcie.
    let gone = ok(p.remove(&id).await);
    assert_eq!(gone.len(), 4);
    assert!(p.list().is_empty() && p.tools().is_empty());
    assert_eq!(
        p.remove(&id).await,
        Err(PluginError::NotFound("licznik".into()))
    );
}
