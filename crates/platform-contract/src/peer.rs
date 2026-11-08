//! Tożsamość procesu po drugiej stronie kanału lokalnego (named pipe): SID konta, poziom
//! integralności, obraz, sesja, podpis Authenticode — oraz czysta logika weryfikacji klienta
//! (PLAN §8.1, §8.4; THREAT_MODEL §4). Windows: `GetNamedPipeClientProcessId` → token procesu.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Identyfikator zabezpieczeń (SID) w postaci tekstowej `S-1-<autorytet>-<pod-autorytety>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Sid(String);

impl Sid {
    /// `LocalSystem`.
    pub const LOCAL_SYSTEM: &'static str = "S-1-5-18";

    /// Parsuje i normalizuje SID (1–15 pod-autorytetów, wyłącznie cyfry dziesiętne).
    pub fn parse(text: &str) -> Result<Self, PlatformError> {
        let bad = || PlatformError::InvalidPath(PathBuf::from(format!("niepoprawny SID: {text}")));
        let mut parts = text.trim().split('-');
        let head_ok =
            parts.next().is_some_and(|p| p.eq_ignore_ascii_case("S")) && parts.next() == Some("1");
        let rest: Vec<&str> = parts.collect();
        let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
        let authority_ok = rest
            .first()
            .filter(|a| digits(a))
            .and_then(|a| a.parse::<u64>().ok())
            .is_some_and(|a| a < (1 << 48));
        let subs_ok = rest
            .iter()
            .skip(1)
            .all(|p| digits(p) && p.parse::<u32>().is_ok());
        if !head_ok || !(2..=16).contains(&rest.len()) || !authority_ok || !subs_ok {
            return Err(bad());
        }
        Ok(Self(format!("S-1-{}", rest.join("-"))))
    }

    /// Postać tekstowa.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Sid {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|e| e.to_string())
    }
}

impl From<Sid> for String {
    fn from(value: Sid) -> Self {
        value.0
    }
}

impl fmt::Display for Sid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Obowiązkowy poziom integralności (Mandatory Integrity Control, UIPI).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityLevel {
    /// Niezaufany (RID 0).
    Untrusted,
    /// Niski (0x1000) — narzędzia w izolacji.
    Low,
    /// Średni (0x2000) — zwykłe procesy użytkownika, Alfa, agentki.
    #[default]
    Medium,
    /// Średni plus (0x2100).
    MediumPlus,
    /// Wysoki (0x3000) — Broker-UI (UIPI blokuje SendInput z niższych poziomów).
    High,
    /// Systemowy (0x4000).
    System,
    /// Chroniony proces (0x5000).
    Protected,
}

impl IntegrityLevel {
    /// Poziom z RID ostatniego pod-autorytetu SID-u etykiety (`S-1-16-<RID>`).
    pub fn from_rid(rid: u32) -> Self {
        match rid {
            r if r >= 0x5000 => Self::Protected,
            r if r >= 0x4000 => Self::System,
            r if r >= 0x3000 => Self::High,
            r if r >= 0x2100 => Self::MediumPlus,
            r if r >= 0x2000 => Self::Medium,
            r if r >= 0x1000 => Self::Low,
            _ => Self::Untrusted,
        }
    }

    /// RID poziomu.
    pub fn rid(self) -> u32 {
        match self {
            Self::Untrusted => 0,
            Self::Low => 0x1000,
            Self::Medium => 0x2000,
            Self::MediumPlus => 0x2100,
            Self::High => 0x3000,
            Self::System => 0x4000,
            Self::Protected => 0x5000,
        }
    }

    /// SID etykiety (`S-1-16-<RID>`) — do ustawiania poziomu tokenu.
    pub fn label_sid(self) -> String {
        format!("S-1-16-{}", self.rid())
    }

    /// Nazwa po polsku.
    pub fn label_pl(self) -> &'static str {
        match self {
            Self::Untrusted => "niezaufany",
            Self::Low => "niski",
            Self::Medium => "średni",
            Self::MediumPlus => "średni plus",
            Self::High => "wysoki",
            Self::System => "systemowy",
            Self::Protected => "chroniony",
        }
    }
}

/// Wynik weryfikacji podpisu Authenticode obrazu procesu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "detail", rename_all = "snake_case")]
pub enum SignatureStatus {
    /// Nie sprawdzano (buildy deweloperskie — brak certyfikatu, bramka ludzka #10).
    NotVerified,
    /// Plik bez podpisu.
    Unsigned,
    /// Podpis niepoprawny / niezaufany.
    Invalid(String),
    /// Podpis poprawny.
    Trusted {
        /// Wystawca certyfikatu (podmiot).
        signer: String,
    },
}

/// Tożsamość procesu-klienta ustalona przez system (nie deklarowana przez klienta).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerIdentity {
    /// PID.
    pub pid: u32,
    /// Pełna ścieżka obrazu (`QueryFullProcessImageNameW`).
    pub image: PathBuf,
    /// SID konta z tokenu procesu.
    pub user: Sid,
    /// Poziom integralności tokenu.
    pub integrity: IntegrityLevel,
    /// Sesja Windows (0 = usługi).
    pub session: u32,
    /// Podpis obrazu.
    pub signature: SignatureStatus,
}

/// Wymagania wobec klienta kanału (np. rola Broker-UI: wysoki poziom, konkretny obraz).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerRequirement {
    /// Dozwolone konta (puste = nikt — fail-closed).
    pub users: Vec<Sid>,
    /// Minimalny poziom integralności.
    pub min_integrity: IntegrityLevel,
    /// Dozwolone obrazy (puste = dowolny).
    #[serde(default)]
    pub images: Vec<PathBuf>,
    /// Wymagany wystawca podpisu (`None` = bez wymogu, buildy deweloperskie).
    #[serde(default)]
    pub signer: Option<String>,
    /// Wymagana sesja Windows.
    #[serde(default)]
    pub session: Option<u32>,
}

/// Powód odrzucenia klienta.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PeerRejection {
    /// Konto spoza listy.
    #[error("konto {0} nie ma dostępu")]
    User(Sid),
    /// Za niski poziom integralności.
    #[error("poziom integralności {got:?} < wymaganego {need:?}")]
    Integrity {
        /// Wymagany.
        need: IntegrityLevel,
        /// Faktyczny.
        got: IntegrityLevel,
    },
    /// Obraz spoza listy.
    #[error("obraz {0} nie jest dozwolony")]
    Image(PathBuf),
    /// Podpis niezgodny z wymaganym.
    #[error("podpis niezgodny: {0:?}")]
    Signature(SignatureStatus),
    /// Inna sesja Windows.
    #[error("sesja {got} ≠ wymaganej {need}")]
    Session {
        /// Wymagana.
        need: u32,
        /// Faktyczna.
        got: u32,
    },
}

/// Porównanie ścieżek obrazów jak w Windows: bez prefiksu `\\?\`, `/` = `\`, bez wielkości liter.
pub fn same_image(a: &Path, b: &Path) -> bool {
    fn norm(p: &Path) -> String {
        let text = p.to_string_lossy().replace('/', "\\");
        let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
        text.to_lowercase()
    }
    norm(a) == norm(b)
}

impl PeerRequirement {
    /// Sprawdza tożsamość klienta (wszystkie warunki muszą być spełnione).
    pub fn check(&self, id: &PeerIdentity) -> Result<(), PeerRejection> {
        if !self.users.contains(&id.user) {
            return Err(PeerRejection::User(id.user.clone()));
        }
        if id.integrity < self.min_integrity {
            return Err(PeerRejection::Integrity {
                need: self.min_integrity,
                got: id.integrity,
            });
        }
        if !self.images.is_empty() && !self.images.iter().any(|p| same_image(p, &id.image)) {
            return Err(PeerRejection::Image(id.image.clone()));
        }
        if let Some(need) = self.session
            && need != id.session
        {
            return Err(PeerRejection::Session {
                need,
                got: id.session,
            });
        }
        if let Some(signer) = &self.signer {
            let ok =
                matches!(&id.signature, SignatureStatus::Trusted { signer: got } if got == signer);
            if !ok {
                return Err(PeerRejection::Signature(id.signature.clone()));
            }
        }
        Ok(())
    }
}

/// Ustalanie tożsamości procesu po PID (z tokenu procesu, nie z deklaracji klienta).
pub trait ProcessIdentityPort: Send + Sync {
    /// Tożsamość procesu (podpis: [`SignatureStatus::NotVerified`] — sprawdza [`CodeSignaturePort`]).
    fn identify(&self, pid: u32) -> Result<PeerIdentity, PlatformError>;

    /// SID konta bieżącego procesu.
    fn current_user(&self) -> Result<Sid, PlatformError>;
}

/// Weryfikacja podpisu Authenticode obrazu.
pub trait CodeSignaturePort: Send + Sync {
    /// Stan podpisu pliku.
    fn verify(&self, image: &Path) -> SignatureStatus;
}

/// Port podpisów buildów deweloperskich: zawsze „niezweryfikowane” (bez certyfikatu nie ma czego
/// przypiąć; wymaganie `signer` w [`PeerRequirement`] odrzuci takiego klienta).
#[derive(Debug, Clone, Copy, Default)]
pub struct UnverifiedSignatures;

impl CodeSignaturePort for UnverifiedSignatures {
    fn verify(&self, _image: &Path) -> SignatureStatus {
        SignatureStatus::NotVerified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> Sid {
        Sid::parse(s).unwrap()
    }

    fn identity() -> PeerIdentity {
        PeerIdentity {
            pid: 7,
            image: PathBuf::from(r"\\?\C:\Program Files\Alfa\alfa-broker-ui.exe"),
            user: sid("S-1-5-21-1-2-3-1001"),
            integrity: IntegrityLevel::High,
            session: 1,
            signature: SignatureStatus::NotVerified,
        }
    }

    #[test]
    fn sid_parsing_is_strict_and_normalizing() {
        assert_eq!(sid("s-1-5-18").as_str(), "S-1-5-18");
        assert_eq!(sid(" S-1-16-12288 ").as_str(), "S-1-16-12288");
        for bad in [
            "",
            "S-1",
            "S-1-5",
            "S-2-5-18",
            "X-1-5-18",
            "S-1-5-1x",
            "S-1--5",
            "S-1-5-99999999999",
            "S-1-5-18-",
            "S-1-5-1-2-3-4-5-6-7-8-9-10-11-12-13-14-15-16",
        ] {
            assert!(Sid::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(String::from(sid("S-1-5-18")), "S-1-5-18");
        assert!(Sid::try_from("zły".to_owned()).is_err());
    }

    #[test]
    fn integrity_from_rid_and_order() {
        assert_eq!(IntegrityLevel::from_rid(0x3000), IntegrityLevel::High);
        assert_eq!(IntegrityLevel::from_rid(0x2fff), IntegrityLevel::MediumPlus);
        assert_eq!(IntegrityLevel::from_rid(0x1fff), IntegrityLevel::Low);
        assert_eq!(IntegrityLevel::from_rid(0), IntegrityLevel::Untrusted);
        assert_eq!(IntegrityLevel::from_rid(0x7000), IntegrityLevel::Protected);
        let all = [0, 0x1000, 0x2000, 0x2100, 0x3000, 0x4000, 0x5000].map(IntegrityLevel::from_rid);
        for l in all {
            assert_eq!(IntegrityLevel::from_rid(l.rid()), l);
            assert!(!l.label_pl().is_empty());
        }
        assert!(IntegrityLevel::Medium < IntegrityLevel::High);
        assert_eq!(IntegrityLevel::High.label_sid(), "S-1-16-12288");
    }

    #[test]
    fn requirement_checks_every_field() {
        let id = identity();
        let mut req = PeerRequirement {
            users: vec![id.user.clone()],
            min_integrity: IntegrityLevel::High,
            images: vec![PathBuf::from("c:/program files/alfa/ALFA-BROKER-UI.EXE")],
            signer: None,
            session: Some(1),
        };
        assert_eq!(req.check(&id), Ok(()));
        let mut low = id.clone();
        low.integrity = IntegrityLevel::Medium;
        assert!(matches!(
            req.check(&low),
            Err(PeerRejection::Integrity { .. })
        ));
        let mut other = id.clone();
        other.user = sid("S-1-5-21-9-9-9-1002");
        assert!(matches!(req.check(&other), Err(PeerRejection::User(_))));
        let mut img = id.clone();
        img.image = PathBuf::from(r"C:\Temp\alfa-broker-ui.exe");
        assert!(matches!(req.check(&img), Err(PeerRejection::Image(_))));
        let mut sess = id.clone();
        sess.session = 2;
        assert!(matches!(
            req.check(&sess),
            Err(PeerRejection::Session { .. })
        ));
        req.signer = Some("Alfa".into());
        assert!(matches!(req.check(&id), Err(PeerRejection::Signature(_))));
        let mut signed = id.clone();
        signed.signature = SignatureStatus::Trusted {
            signer: "Alfa".into(),
        };
        assert_eq!(req.check(&signed), Ok(()));
        req.users.clear();
        assert!(req.check(&signed).is_err(), "pusta lista kont = nikt");
        assert_eq!(
            UnverifiedSignatures.verify(Path::new("x")),
            SignatureStatus::NotVerified
        );
        assert!(
            PeerRejection::Image(PathBuf::from("x"))
                .to_string()
                .contains("obraz")
        );
    }
}
