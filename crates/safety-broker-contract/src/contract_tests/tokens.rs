//! Testy kontraktowe tokenów: wydanie, weryfikacja, manipulacja, TTL, atenuacja,
//! unieważnianie, kill-switch, nieprzenośność między uruchomieniami.

use std::sync::Arc;

use platform_contract::ProcessHandle;
use risk_classifier_contract::CommandOrigin;
use watchdog_contract::{KillReason, ManualClock, ProcessRole};

use super::{FullBroker, allowed, delta, exact, request, tree};
use crate::{AttenuateRequest, BrokerError, Capability, Holder, TokenId};

fn read_docs<B: FullBroker>(b: &B) -> impl std::future::Future<Output = crate::CapToken> + '_ {
    let req = request(
        &delta(),
        Capability::FsRead(tree(r"C:\Users\ala\Docs")),
        CommandOrigin::UserText,
    );
    async move { allowed(b.decide(req).await) }
}

/// Odczyt w profilu na L3 → token; weryfikacja tylko dla podzbioru, właściwego okaziciela.
pub async fn read_in_profile_verifies_only_subset<B: FullBroker>(b: &B) {
    let t = read_docs(b).await;
    let file = Capability::FsRead(exact(r"C:\Users\ala\Docs\raport.txt"));
    assert_eq!(b.verify(&t, &file, &delta()), Ok(()));
    let wider = Capability::FsRead(tree(r"C:\Users\ala"));
    assert!(b.verify(&t, &wider, &delta()).is_err());
    let other_family = Capability::FsWrite(exact(r"C:\Users\ala\Docs\raport.txt"));
    assert!(b.verify(&t, &other_family, &delta()).is_err());
    let thief = Holder::agent("s1", "beta");
    assert!(b.verify(&t, &file, &thief).is_err());
    let escape = Capability::FsRead(exact(r"C:\Users\ala\Docs\..\.ssh\id_rsa"));
    assert!(b.verify(&t, &escape, &delta()).is_err());
}

/// Zmodyfikowany token (każde pole, MAC) jest odrzucany.
pub async fn tampered_token_rejected<B: FullBroker>(b: &B) {
    let t = read_docs(b).await;
    let file = Capability::FsRead(exact(r"C:\Users\ala\Docs\a.txt"));
    let mut variants = Vec::new();
    let mut x = t.clone();
    x.expires_at_ms += 3_600_000;
    variants.push(x);
    let mut x = t.clone();
    x.cap = Capability::FsRead(tree(r"C:\Users\ala"));
    variants.push(x);
    let mut x = t.clone();
    x.holder = Holder::agent("s1", "beta");
    variants.push(x);
    let mut x = t.clone();
    x.id = TokenId(t.id.0 + 1);
    variants.push(x);
    let mut x = t.clone();
    x.mac[0] ^= 1;
    variants.push(x);
    let mut x = t.clone();
    x.key_epoch += 1;
    variants.push(x);
    let mut x = t.clone();
    x.boot.0[0] ^= 0x80;
    variants.push(x);
    for v in variants {
        let presenter = v.holder.clone();
        assert!(b.verify(&v, &file, &presenter).is_err(), "{v:?}");
    }
    assert_eq!(b.verify(&t, &file, &delta()), Ok(()));
}

/// Token po TTL jest odrzucany.
pub async fn expired_token_rejected<B: FullBroker>(b: &B, clock: &ManualClock) {
    let mut req = request(
        &delta(),
        Capability::FsRead(tree(r"C:\Users\ala\Docs")),
        CommandOrigin::UserText,
    );
    req.ttl_ms = Some(1_000);
    let t = allowed(b.decide(req).await);
    let file = Capability::FsRead(exact(r"C:\Users\ala\Docs\a.txt"));
    assert_eq!(b.verify(&t, &file, &delta()), Ok(()));
    clock.advance(1_000);
    assert_eq!(
        b.verify(&t, &file, &delta()),
        Err(BrokerError::TokenExpired)
    );
}

/// Atenuacja: potomek ⊆ rodzic, TTL ≤ rodzic, ten sam podmiot; unieważnienie rodzica
/// unieważnia potomka.
pub async fn attenuation_and_revocation<B: FullBroker>(b: &B) {
    let parent = allowed(
        b.decide(request(
            &delta(),
            Capability::FsWrite(tree(r"C:\Users\ala\Projects")),
            CommandOrigin::UserText,
        ))
        .await,
    );
    let narrow = AttenuateRequest {
        capability: Capability::FsWrite(exact(r"C:\Users\ala\Projects\a.txt")),
        role: Some("pomocnica".into()),
        ttl_ms: u64::MAX / 4,
    };
    let child = b
        .attenuate(&parent, &delta(), narrow.clone())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(child.expires_at_ms <= parent.expires_at_ms);
    assert_eq!(child.parent, Some(parent.id));
    let child_holder = child.holder.clone();
    let cap = Capability::FsWrite(exact(r"C:\Users\ala\Projects\a.txt"));
    assert_eq!(b.verify(&child, &cap, &child_holder), Ok(()));
    let wider = AttenuateRequest {
        capability: Capability::FsWrite(tree(r"C:\Users\ala")),
        ..narrow.clone()
    };
    assert_eq!(
        b.attenuate(&parent, &delta(), wider).await,
        Err(BrokerError::NotAttenuated)
    );
    let family = AttenuateRequest {
        capability: Capability::FsRead(exact(r"C:\Users\ala\Projects\a.txt")),
        ..narrow.clone()
    };
    assert_eq!(
        b.attenuate(&parent, &delta(), family).await,
        Err(BrokerError::NotAttenuated)
    );
    let stranger = Holder::agent("s1", "gama");
    assert!(b.attenuate(&parent, &stranger, narrow).await.is_err());
    let revoked = b.revoke(parent.id).await.unwrap_or_else(|e| panic!("{e}"));
    assert!(revoked >= 2, "unieważniono {revoked}");
    assert!(b.verify(&child, &cap, &child_holder).is_err());
}

/// Kill-switch unieważnia wszystkie tokeny i zabija zarejestrowane drzewa procesów.
pub async fn kill_switch_revokes_everything<B: FullBroker>(b: &B) {
    let t1 = read_docs(b).await;
    let t2 = read_docs(b).await;
    b.register_job(
        ProcessHandle(11),
        ProcessRole::Tool("shell".into()),
        "shell",
    );
    b.register_job(
        ProcessHandle(12),
        ProcessRole::Sidecar("voice-stt".into()),
        "stt",
    );
    let report = b.kill_all(KillReason::Hotkey).await;
    assert!(report.tokens_revoked >= 2, "{report:?}");
    assert_eq!(report.jobs_killed, 2);
    assert!(report.audio_silenced);
    assert!(b.jobs().is_empty());
    let file = Capability::FsRead(exact(r"C:\Users\ala\Docs\a.txt"));
    assert!(b.verify(&t1, &file, &delta()).is_err());
    assert!(b.verify(&t2, &file, &delta()).is_err());
    let fresh = read_docs(b).await;
    assert_eq!(b.verify(&fresh, &file, &delta()), Ok(()));
}

/// Token z innego uruchomienia Brokera jest odrzucany.
pub async fn not_portable_between_runs<B: FullBroker>(a: &B, other: &B) {
    let t = read_docs(a).await;
    let file = Capability::FsRead(exact(r"C:\Users\ala\Docs\a.txt"));
    assert!(other.verify(&t, &file, &delta()).is_err());
}

/// Uruchamia testy tokenów.
pub async fn run<B, F>(fresh: &F)
where
    B: FullBroker,
    F: Fn() -> (B, Arc<ManualClock>),
{
    read_in_profile_verifies_only_subset(&fresh().0).await;
    tampered_token_rejected(&fresh().0).await;
    let (b, clock) = fresh();
    expired_token_rejected(&b, &clock).await;
    attenuation_and_revocation(&fresh().0).await;
    kill_switch_revokes_everything(&fresh().0).await;
    not_portable_between_runs(&fresh().0, &fresh().0).await;
}
