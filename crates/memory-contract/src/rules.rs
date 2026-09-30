//! Reguły wspólne dla `-impl` i `-fake` (żeby atrapa nie rozjechała się z implementacją).

use chrono::{DateTime, Utc};
use core_bus_contract::SessionId;

use crate::types::{Layer, MemoryEntry, MemoryError, MemoryScope, NewMemory, RememberMode};

fn unsupported(what: impl Into<String>) -> MemoryError {
    MemoryError::Unsupported { what: what.into() }
}

/// Walidacja nowego wpisu; zwraca sesję zakresu (v0).
///
/// Kolejność: zakres (tylko sesja) → warstwa (epizodyczna/semantyczna) → treść → pewność →
/// automatyczne zapamiętanie z treści niezaufanej (odmowa).
pub fn validate_new(new: &NewMemory, mode: RememberMode) -> Result<SessionId, MemoryError> {
    let MemoryScope::Session(session) = &new.scope else {
        return Err(unsupported("zakres inny niż sesja"));
    };
    if matches!(new.layer, Layer::Working | Layer::Procedural) {
        return Err(unsupported("warstwa robocza/proceduralna"));
    }
    if new.text.trim().is_empty() {
        return Err(MemoryError::Invalid {
            reason: "pusta treść".into(),
        });
    }
    if !(0.0..=1.0).contains(&new.confidence) {
        return Err(MemoryError::Invalid {
            reason: format!("pewność {} poza 0–1", new.confidence),
        });
    }
    if mode == RememberMode::AutoPendingApproval && !new.provenance.is_trusted() {
        return Err(MemoryError::UntrustedAutoRemember);
    }
    Ok(session.clone())
}

/// Sesje z listy zakresów `recall` (v0: każdy inny zakres → `Unsupported`), bez duplikatów.
pub fn recall_sessions(scopes: &[MemoryScope]) -> Result<Vec<SessionId>, MemoryError> {
    let mut out = Vec::new();
    for scope in scopes {
        match scope {
            MemoryScope::Session(s) if !out.contains(s) => out.push(s.clone()),
            MemoryScope::Session(_) => {}
            _ => return Err(unsupported("recall poza zakresem sesji")),
        }
    }
    Ok(out)
}

/// Czy wpis wygasł (`created_at + ttl ≤ now`).
pub fn is_expired(entry: &MemoryEntry, now: DateTime<Utc>) -> bool {
    entry.ttl_secs.is_some_and(|ttl| {
        i64::try_from(ttl)
            .ok()
            .and_then(chrono::Duration::try_seconds)
            .and_then(|ttl| entry.created_at.checked_add_signed(ttl))
            .is_some_and(|deadline| deadline <= now)
    })
}

/// Awans: niezaufane nigdy (do żadnego zakresu, w tym `Global`); pozostałe — v0 `Unsupported`.
pub fn check_promotion(entry: &MemoryEntry, to: &MemoryScope) -> Result<(), MemoryError> {
    if !entry.provenance.is_trusted() || !entry.trusted {
        return Err(MemoryError::UntrustedCannotPromote);
    }
    if *to == entry.scope {
        return Err(MemoryError::Invalid {
            reason: "zakres docelowy = bieżący".into(),
        });
    }
    Err(unsupported("awans zakresu (F7)"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MemoryId, Provenance};

    fn entry(ttl: Option<u64>, provenance: Provenance) -> MemoryEntry {
        MemoryEntry {
            id: MemoryId("m".into()),
            scope: MemoryScope::Session(SessionId::new("s")),
            layer: Layer::Semantic,
            text: "x".into(),
            entities: vec![],
            trusted: provenance.is_trusted(),
            provenance,
            confidence: 1.0,
            ttl_secs: ttl,
            created_at: DateTime::<Utc>::default(),
            approved: true,
        }
    }

    #[test]
    fn ttl_expiry() {
        let now = DateTime::<Utc>::default() + chrono::Duration::seconds(10);
        assert!(is_expired(&entry(Some(0), Provenance::User), now));
        assert!(is_expired(&entry(Some(10), Provenance::User), now));
        assert!(!is_expired(&entry(Some(11), Provenance::User), now));
        assert!(!is_expired(&entry(None, Provenance::User), now));
        assert!(!is_expired(&entry(Some(u64::MAX), Provenance::User), now));
    }

    #[test]
    fn untrusted_never_promotes() {
        let bad = entry(
            None,
            Provenance::UntrustedContent {
                source: "https://x".into(),
            },
        );
        for to in [MemoryScope::Global, MemoryScope::Project("p".into())] {
            assert_eq!(
                check_promotion(&bad, &to),
                Err(MemoryError::UntrustedCannotPromote)
            );
        }
        let good = entry(None, Provenance::User);
        assert!(matches!(
            check_promotion(&good, &MemoryScope::Global),
            Err(MemoryError::Unsupported { .. })
        ));
    }

    #[test]
    fn validation_order_and_nan() {
        let mut new = NewMemory::user_fact(SessionId::new("s"), "fakt");
        new.confidence = f32::NAN;
        assert!(matches!(
            validate_new(&new, RememberMode::Explicit),
            Err(MemoryError::Invalid { .. })
        ));
        new.scope = MemoryScope::Global;
        assert!(matches!(
            validate_new(&new, RememberMode::Explicit),
            Err(MemoryError::Unsupported { .. })
        ));
        assert!(recall_sessions(&[MemoryScope::Global]).is_err());
        let s = MemoryScope::Session(SessionId::new("a"));
        assert_eq!(recall_sessions(&[s.clone(), s]).unwrap().len(), 1);
    }
}
