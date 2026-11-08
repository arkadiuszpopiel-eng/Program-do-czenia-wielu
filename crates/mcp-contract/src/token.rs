//! Tablica tokenów sesyjnych z TTL (czysta logika wspólna `-impl` i `-fake`; zegar podaje
//! wywołujący). TTL dotyczy **nawiązania połączenia**; unieważnienie (koniec zadania) zamyka
//! także połączenia już nawiązane.

use crate::bridge::{RegistrationId, SessionToken};

/// Powód odrzucenia tokenu (do dziennika; proxy dostaje tylko zamknięcie połączenia).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRejection {
    /// Token nieznany.
    Unknown,
    /// Token wygasł.
    Expired,
    /// Rejestracja unieważniona.
    Revoked,
}

impl std::fmt::Display for TokenRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TokenRejection::Unknown => "nieznany token",
            TokenRejection::Expired => "token wygasł",
            TokenRejection::Revoked => "rejestracja unieważniona",
        })
    }
}

#[derive(Debug)]
struct Entry<T> {
    id: RegistrationId,
    token: SessionToken,
    expires_at_ms: u64,
    revoked: bool,
    value: T,
}

/// Tablica rejestracji.
#[derive(Debug)]
pub struct TokenTable<T> {
    entries: Vec<Entry<T>>,
}

impl<T> Default for TokenTable<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T: Clone> TokenTable<T> {
    /// Pusta tablica.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dodaje rejestrację.
    pub fn insert(
        &mut self,
        id: RegistrationId,
        token: SessionToken,
        expires_at_ms: u64,
        value: T,
    ) {
        self.entries.push(Entry {
            id,
            token,
            expires_at_ms,
            revoked: false,
            value,
        });
    }

    /// Sprawdza token przy nawiązaniu połączenia. Porównuje ze **wszystkimi** wpisami
    /// (bez wczesnego wyjścia), żeby czas odpowiedzi nie zdradzał pozycji wpisu.
    pub fn validate(
        &self,
        presented: &str,
        now_ms: u64,
    ) -> Result<(RegistrationId, T), TokenRejection> {
        let mut found: Option<&Entry<T>> = None;
        for entry in &self.entries {
            if entry.token.matches(presented) && found.is_none() {
                found = Some(entry);
            }
        }
        let entry = found.ok_or(TokenRejection::Unknown)?;
        if entry.revoked {
            return Err(TokenRejection::Revoked);
        }
        if now_ms >= entry.expires_at_ms {
            return Err(TokenRejection::Expired);
        }
        Ok((entry.id.clone(), entry.value.clone()))
    }

    /// Czy rejestracja nadal obowiązuje dla nawiązanego połączenia (nie unieważniona).
    pub fn is_live(&self, id: &RegistrationId) -> bool {
        self.entries.iter().any(|e| &e.id == id && !e.revoked)
    }

    /// Unieważnia rejestrację; `false`, gdy jej nie ma.
    pub fn revoke(&mut self, id: &RegistrationId) -> bool {
        let mut hit = false;
        for entry in self.entries.iter_mut().filter(|e| &e.id == id) {
            entry.revoked = true;
            hit = true;
        }
        hit
    }

    /// Usuwa wpisy wygasłe i unieważnione (po tym ich tokeny są „nieznane”).
    pub fn purge(&mut self, now_ms: u64) {
        self.entries
            .retain(|e| !e.revoked && now_ms < e.expires_at_ms);
    }

    /// Liczba wpisów.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Czy pusta.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> RegistrationId {
        RegistrationId(s.into())
    }

    #[test]
    fn ttl_revocation_and_purge() {
        let mut t = TokenTable::new();
        t.insert(id("a"), SessionToken::new("tok-a"), 100, 1u8);
        t.insert(id("b"), SessionToken::new("tok-b"), 200, 2u8);
        assert_eq!(t.validate("tok-a", 99), Ok((id("a"), 1)));
        assert_eq!(t.validate("tok-a", 100), Err(TokenRejection::Expired));
        assert_eq!(t.validate("nope", 0), Err(TokenRejection::Unknown));
        assert!(t.revoke(&id("b")));
        assert!(!t.revoke(&id("zzz")));
        assert_eq!(t.validate("tok-b", 0), Err(TokenRejection::Revoked));
        assert!(!t.is_live(&id("b")));
        assert!(t.is_live(&id("a")));
        t.purge(150);
        assert!(t.is_empty());
        assert_eq!(t.validate("tok-a", 0), Err(TokenRejection::Unknown));
        assert_eq!(TokenRejection::Expired.to_string(), "token wygasł");
        assert_eq!(t.len(), 0);
    }
}
