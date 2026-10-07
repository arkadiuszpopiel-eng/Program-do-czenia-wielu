//! Kanał lokalny z kontrolą dostępu: named pipe z DACL na SID-y, `PIPE_REJECT_REMOTE_CLIENTS`,
//! `FILE_FLAG_FIRST_PIPE_INSTANCE` (ochrona przed przejęciem nazwy) i etykietą integralności
//! (PLAN §8.1, §8.7; THREAT_MODEL §4: „named pipe z ACL na SID”). Bez TCP.
//!
//! Połączenie jest blokujące i półdupleksowe w użyciu: protokół Brokera to żądanie → odpowiedź,
//! więc jednego połączenia nie czyta się i nie pisze jednocześnie z dwóch wątków (synchroniczne
//! uchwyty Windows serializują operacje na jednym obiekcie pliku).

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;
use crate::peer::{IntegrityLevel, Sid};

/// Prefiks ścieżki potoku lokalnego.
pub const PIPE_PREFIX: &str = r"\\.\pipe\";

/// Prawa klienta: `FILE_GENERIC_READ | FILE_WRITE_DATA` — bez `FILE_APPEND_DATA`
/// (= `FILE_CREATE_PIPE_INSTANCE`), więc klient nie utworzy własnej instancji serwera tej nazwy.
pub const CLIENT_ACCESS_MASK: u32 = 0x0012_008B;

/// Maksymalna długość nazwy potoku (bez prefiksu).
const MAX_NAME: usize = 128;

/// Opis zabezpieczeń potoku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipeSecurity {
    name: String,
    owner: Sid,
    clients: Vec<Sid>,
    label: IntegrityLevel,
    first_instance: bool,
}

/// Sprawdza nazwę potoku: 1–128 znaków `[A-Za-z0-9._-]`.
pub fn validate_pipe_name(name: &str) -> Result<(), PlatformError> {
    let ok = !name.is_empty()
        && name.len() <= MAX_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(PlatformError::InvalidPath(
            format!("{PIPE_PREFIX}{name}").into(),
        ))
    }
}

impl PipeSecurity {
    /// Potok `name`: pełny dostęp (tworzenie instancji) tylko `owner` (konto serwera), klienci —
    /// odczyt i zapis danych. Etykieta integralności: średnia (procesy niskiej integralności
    /// nie mogą pisać), pierwsza instancja wymagana.
    pub fn new(name: &str, owner: Sid, clients: Vec<Sid>) -> Result<Self, PlatformError> {
        validate_pipe_name(name)?;
        if clients.is_empty() {
            return Err(PlatformError::PermissionDenied(
                "potok bez dozwolonych klientów".into(),
            ));
        }
        Ok(Self {
            name: name.to_owned(),
            owner,
            clients,
            label: IntegrityLevel::Medium,
            first_instance: true,
        })
    }

    /// Minimalny poziom integralności piszącego klienta (etykieta `NW`); od `Low` do `High`.
    #[must_use]
    pub fn with_min_integrity(mut self, level: IntegrityLevel) -> Self {
        self.label = level.clamp(IntegrityLevel::Low, IntegrityLevel::High);
        self
    }

    /// Wyłącza wymóg pierwszej instancji (tylko kolejne instancje tej samej nazwy w serwerze).
    #[must_use]
    pub fn with_first_instance(mut self, first: bool) -> Self {
        self.first_instance = first;
        self
    }

    /// Nazwa (bez prefiksu).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Pełna ścieżka `\\.\pipe\<nazwa>`.
    pub fn path(&self) -> String {
        format!("{PIPE_PREFIX}{}", self.name)
    }

    /// Konto serwera.
    pub fn owner(&self) -> &Sid {
        &self.owner
    }

    /// Dozwoleni klienci.
    pub fn clients(&self) -> &[Sid] {
        &self.clients
    }

    /// Czy wymagana jest pierwsza instancja (`FILE_FLAG_FIRST_PIPE_INSTANCE`).
    pub fn first_instance(&self) -> bool {
        self.first_instance
    }

    /// Etykieta integralności.
    pub fn min_integrity(&self) -> IntegrityLevel {
        self.label
    }

    /// Deskryptor SDDL: chroniony DACL (bez dziedziczenia), tylko wymienione SID-y, bez
    /// `Everyone`/`Anonymous`; etykieta `ML;;NW` (no-write-up) na poziomie [`Self::min_integrity`].
    pub fn sddl(&self) -> String {
        let mut out = format!("D:P(A;;FA;;;{})", self.owner);
        for client in self.clients.iter().filter(|c| **c != self.owner) {
            out.push_str(&format!("(A;;0x{CLIENT_ACCESS_MASK:x};;;{client})"));
        }
        let label = match self.label {
            IntegrityLevel::Low | IntegrityLevel::Untrusted => "LW",
            IntegrityLevel::Medium => "ME",
            IntegrityLevel::MediumPlus => "MP",
            _ => "HI",
        };
        out.push_str(&format!("S:(ML;;NW;;;{label})"));
        out
    }
}

/// SDDL katalogu prywatnego (kotwica i pliki Audytu): pełny dostęp tylko `owner` i `SYSTEM`,
/// dziedziczony przez pliki i podkatalogi, bez dziedziczenia z nadrzędnych (`P`).
pub fn private_dir_sddl(owner: &Sid) -> String {
    if owner.as_str() == Sid::LOCAL_SYSTEM {
        return "D:P(A;OICI;FA;;;SY)".to_owned();
    }
    format!("D:P(A;OICI;FA;;;{owner})(A;OICI;FA;;;SY)")
}

/// Połączenie przez potok (blokujące).
pub trait PipeConnection: Read + Write + Send {
    /// PID drugiej strony: dla serwera — klient (`GetNamedPipeClientProcessId`), dla klienta —
    /// serwer (`GetNamedPipeServerProcessId`, ochrona przed podstawionym serwerem).
    fn peer_pid(&self) -> u32;
}

/// Nasłuch potoku: każda instancja tworzona z tym samym deskryptorem.
pub trait PipeListener: Send {
    /// Czeka na klienta (blokująco) i zwraca połączenie.
    fn accept(&mut self) -> Result<Box<dyn PipeConnection>, PlatformError>;
}

/// Port potoków z kontrolą dostępu.
pub trait SecurePipePort: Send + Sync {
    /// Tworzy potok (pierwsza instancja zajęta przez inny proces → błąd: możliwe przejęcie nazwy).
    fn listen(&self, security: &PipeSecurity) -> Result<Box<dyn PipeListener>, PlatformError>;

    /// Łączy się z potokiem `name` (tylko lokalnie; klient prosi o minimalne prawa i poziom
    /// personifikacji `Identification`), czekając na wolną instancję do `timeout_ms`.
    fn connect(
        &self,
        name: &str,
        timeout_ms: u32,
    ) -> Result<Box<dyn PipeConnection>, PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> Sid {
        Sid::parse(s).unwrap()
    }

    #[test]
    fn sddl_lists_only_owner_and_clients() {
        let owner = sid("S-1-5-80-1-2-3-4-5");
        let user = sid("S-1-5-21-10-20-30-1001");
        let sec = PipeSecurity::new(
            "alfa-broker",
            owner.clone(),
            vec![user.clone(), owner.clone()],
        )
        .unwrap();
        assert_eq!(sec.path(), r"\\.\pipe\alfa-broker");
        assert_eq!(
            sec.sddl(),
            "D:P(A;;FA;;;S-1-5-80-1-2-3-4-5)(A;;0x12008b;;;S-1-5-21-10-20-30-1001)S:(ML;;NW;;;ME)"
        );
        assert!(!sec.sddl().contains(";WD)") && !sec.sddl().contains(";AN)"));
        assert!(sec.first_instance());
        let high = sec
            .clone()
            .with_min_integrity(IntegrityLevel::System)
            .with_first_instance(false);
        assert!(high.sddl().ends_with("S:(ML;;NW;;;HI)"));
        assert_eq!(high.min_integrity(), IntegrityLevel::High);
        assert!(!high.first_instance());
        let low = sec.with_min_integrity(IntegrityLevel::Untrusted);
        assert!(low.sddl().ends_with("(ML;;NW;;;LW)"));
        assert_eq!(low.owner(), &owner);
        assert_eq!(low.clients().len(), 2);
        assert_eq!(low.name(), "alfa-broker");
    }

    #[test]
    fn client_mask_cannot_create_instances() {
        const FILE_APPEND_DATA_CREATE_PIPE_INSTANCE: u32 = 0x4;
        const FILE_WRITE_DATA: u32 = 0x2;
        const FILE_READ_DATA: u32 = 0x1;
        assert_eq!(
            CLIENT_ACCESS_MASK & FILE_APPEND_DATA_CREATE_PIPE_INSTANCE,
            0
        );
        assert_ne!(CLIENT_ACCESS_MASK & FILE_WRITE_DATA, 0);
        assert_ne!(CLIENT_ACCESS_MASK & FILE_READ_DATA, 0);
    }

    #[test]
    fn names_and_empty_clients_rejected() {
        let o = sid("S-1-5-18");
        for bad in ["", "a\\b", "x y", "..\\x", &"a".repeat(129)] {
            assert!(
                PipeSecurity::new(bad, o.clone(), vec![o.clone()]).is_err(),
                "{bad}"
            );
        }
        assert!(PipeSecurity::new("ok.name_1-2", o.clone(), vec![]).is_err());
        assert_eq!(private_dir_sddl(&o), "D:P(A;OICI;FA;;;SY)");
        let u = sid("S-1-5-21-1-2-3-500");
        assert_eq!(
            private_dir_sddl(&u),
            "D:P(A;OICI;FA;;;S-1-5-21-1-2-3-500)(A;OICI;FA;;;SY)"
        );
    }
}
