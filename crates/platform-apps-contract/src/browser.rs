//! Port izolowanej przeglądarki (PLAN §7.2 „Przeglądarka: CDP, własny profil”, §1.3 pkt 3/6;
//! THREAT_MODEL S01, S15, S16).
//!
//! Niezmienniki (egzekwowane w porcie, testowane na atrapie):
//! - przeglądarka (Edge/Chrome) uruchamiana przez Alfę z **osobnym profilem** w katalogu Alfy;
//!   profil użytkownika (ciasteczka, hasła) nigdy — [`is_user_browser_profile`];
//! - CDP wyłącznie przez **potok** (`--remote-debugging-pipe`), nigdy port TCP ([`chromium_args`]);
//! - każde żądanie sieciowe strony przechodzi przez [`EgressFilter`] (hosty zatwierdzone przez
//!   Brokera `net.egress(host)`); reszta jest blokowana i raportowana (`blocked_hosts`);
//! - menedżer haseł i autouzupełnianie wyłączone w preferencjach profilu; pola haseł bez wartości
//!   i bez wpisywania ([`BrowserError::PasswordField`]);
//! - pobrane pliki trafiają do katalogu kwarantanny; treść stron jest niezaufana (taint `Web`).

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use platform_contract::PlatformError;
use serde::{Deserialize, Serialize};

/// Limit pojedynczej operacji przeglądarki (ms).
pub const BROWSER_CALL_TIMEOUT_MS: u64 = 30_000;
/// Najdłuższy URL.
pub const MAX_URL_LEN: usize = 4_096;
/// Najdłuższy wpisywany tekst (znaki).
pub const MAX_TYPE_CHARS: usize = 10_000;

/// Fragmenty ścieżek profili przeglądarek użytkownika (po normalizacji `\`, małe litery).
const USER_PROFILE_MARKERS: [&str; 14] = [
    "\\google\\chrome",
    "\\microsoft\\edge",
    "\\bravesoftware\\",
    "\\chromium\\",
    "\\vivaldi\\",
    "\\opera software\\",
    "\\mozilla\\firefox",
    "\\yandex\\yandexbrowser",
    "\\.mozilla\\",
    "\\.config\\google-chrome",
    "\\.config\\chromium",
    "\\.config\\microsoft-edge",
    "\\.config\\bravesoftware",
    "\\user data\\default",
];

/// Przeglądarka.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserKind {
    /// Microsoft Edge (domyślna w Windows 11).
    Edge,
    /// Google Chrome.
    Chrome,
}

impl BrowserKind {
    /// Plik wykonywalny.
    pub fn exe(self) -> &'static str {
        match self {
            Self::Edge => "msedge.exe",
            Self::Chrome => "chrome.exe",
        }
    }
}

/// Błąd portu przeglądarki.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BrowserError {
    /// Brak przeglądarki.
    #[error("przeglądarka niedostępna: {0}")]
    NotInstalled(String),
    /// Zasady (profil użytkownika, schemat URL, katalog poza Alfą, limit).
    #[error("zasady: {0}")]
    Policy(String),
    /// Host niezatwierdzony przez Brokera — żądanie zablokowane.
    #[error("host niezatwierdzony (net.egress): {0}")]
    Blocked(String),
    /// Limit czasu.
    #[error("limit czasu {ms} ms: {op}")]
    Timeout {
        /// Operacja.
        op: String,
        /// Limit.
        ms: u64,
    },
    /// Brak elementu / sesji.
    #[error("nie znaleziono: {0}")]
    NotFound(String),
    /// Pole hasła — agentka nie wpisuje ani nie czyta haseł.
    #[error("pole hasła — wpisywanie i odczyt zablokowane")]
    PasswordField,
    /// Przeglądarka zamknięta albo potok CDP przerwany.
    #[error("przeglądarka zamknięta: {0}")]
    Closed(String),
    /// Błąd protokołu CDP.
    #[error("CDP: {0}")]
    Protocol(String),
    /// Błąd platformy.
    #[error("{0}")]
    Platform(PlatformError),
}

/// Czy ścieżka wskazuje profil przeglądarki użytkownika (ciasteczka, hasła — THREAT_MODEL S15).
pub fn is_user_browser_profile(path: &Path) -> bool {
    let p = format!(
        "{}\\",
        path.to_string_lossy().replace('/', "\\").to_lowercase()
    );
    USER_PROFILE_MARKERS.iter().any(|m| p.contains(m))
}

fn under(path: &Path, root: &Path) -> bool {
    let clean = |p: &Path| -> Option<Vec<String>> {
        let mut out = Vec::new();
        for c in p.components() {
            match c {
                Component::ParentDir => return None,
                Component::CurDir => {}
                other => out.push(other.as_os_str().to_string_lossy().to_lowercase()),
            }
        }
        Some(out)
    };
    match (clean(path), clean(root)) {
        (Some(p), Some(r)) => p.len() > r.len() && p.starts_with(&r),
        _ => false,
    }
}

/// Konfiguracja sesji przeglądarki.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSpec {
    /// Przeglądarka.
    pub kind: BrowserKind,
    /// Ścieżka pliku wykonywalnego (`None` = wykrycie w typowych miejscach).
    pub executable: Option<PathBuf>,
    /// Katalog danych Alfy (`%LOCALAPPDATA%\Alfa`) — profil i kwarantanna muszą leżeć wewnątrz.
    pub alfa_root: PathBuf,
    /// Osobny profil Alfy.
    pub profile_dir: PathBuf,
    /// Katalog kwarantanny pobrań.
    pub quarantine_dir: PathBuf,
    /// Bez okna (domyślnie tak — agentka pracuje w tle).
    pub headless: bool,
}

impl BrowserSpec {
    /// Profil i kwarantanna wewnątrz katalogu Alfy, rozłączne, nigdy profil użytkownika.
    pub fn validate(&self) -> Result<(), BrowserError> {
        let policy = |m: &str| Err(BrowserError::Policy(m.to_owned()));
        if !self.alfa_root.is_absolute() || is_user_browser_profile(&self.alfa_root) {
            return policy("katalog Alfy musi być bezwzględny i poza profilami przeglądarek");
        }
        for (dir, what) in [
            (&self.profile_dir, "profil"),
            (&self.quarantine_dir, "kwarantanna"),
        ] {
            if is_user_browser_profile(dir) {
                return policy(&format!(
                    "{what}: profil przeglądarki użytkownika jest zablokowany"
                ));
            }
            if !under(dir, &self.alfa_root) {
                return policy(&format!("{what} musi leżeć w katalogu danych Alfy"));
            }
        }
        if under(&self.quarantine_dir, &self.profile_dir)
            || under(&self.profile_dir, &self.quarantine_dir)
            || self.quarantine_dir == self.profile_dir
        {
            return policy("profil i kwarantanna muszą być rozłączne");
        }
        Ok(())
    }
}

/// Argumenty uruchomienia Chromium: CDP przez potok, osobny profil, bez synchronizacji, rozszerzeń,
/// ruchu w tle i menedżera haseł. Nigdy `--remote-debugging-port`.
pub fn chromium_args(spec: &BrowserSpec) -> Vec<String> {
    let mut args = vec![
        "--remote-debugging-pipe".to_owned(),
        format!("--user-data-dir={}", spec.profile_dir.display()),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-sync".into(),
        "--disable-extensions".into(),
        "--disable-background-networking".into(),
        "--disable-component-update".into(),
        "--disable-domain-reliability".into(),
        "--disable-client-side-phishing-detection".into(),
        "--disable-default-apps".into(),
        "--no-pings".into(),
        "--password-store=basic".into(),
        "--disable-features=PasswordManagerOnboarding,AutofillServerCommunication,\
         MediaRouter,OptimizationHints,Translate"
            .into(),
    ];
    if spec.headless {
        args.push("--headless=new".into());
    }
    args.push("about:blank".into());
    args
}

/// Preferencje profilu (`Default/Preferences`) zapisywane przed pierwszym startem: menedżer haseł,
/// autouzupełnianie i zapisywanie kart wyłączone, pobrania bez pytania do kwarantanny.
pub fn profile_preferences(quarantine: &Path) -> serde_json::Value {
    serde_json::json!({
        "credentials_enable_service": false,
        "credentials_enable_autosignin": false,
        "profile": {"password_manager_enabled": false},
        "autofill": {"enabled": false, "profile_enabled": false, "credit_card_enabled": false},
        "download": {
            "default_directory": quarantine.to_string_lossy(),
            "prompt_for_download": false
        },
        "safebrowsing": {"enabled": true},
        "signin": {"allowed": false},
    })
}

/// Host z adresu sieciowego (`http`, `https`, `ws`, `wss`), małymi literami, bez portu.
/// `None` dla schematów bez sieci albo niepoprawnych adresów.
pub fn url_host(url: &str) -> Option<String> {
    let (scheme, rest) = url.trim().split_once("://")?;
    if !matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "ws" | "wss"
    ) {
        return None;
    }
    // Jak WHATWG URL dla schematów specjalnych: `\` to separator ścieżki (ten sam host co Chromium).
    let rest = rest.replace('\\', "/");
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split(']').next()?.to_owned()
    } else {
        host_port.split(':').next()?.to_owned()
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Czy żądanie nie wychodzi do sieci (`data:`, `blob:`, `about:blank`).
pub fn is_local_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("data:") || u.starts_with("blob:") || u == "about:blank"
}

/// Adres nawigacji od agentki: tylko `http`/`https`, z hostem, bez danych logowania w URL.
pub fn check_navigation_url(url: &str) -> Result<String, BrowserError> {
    let u = url.trim();
    let lower = u.to_ascii_lowercase();
    if u.len() > MAX_URL_LEN || u.chars().any(char::is_control) {
        return Err(BrowserError::Policy("niepoprawny adres".into()));
    }
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(BrowserError::Policy(
            "dozwolone tylko adresy http:// i https://".into(),
        ));
    }
    let normalized = u.replace('\\', "/");
    let authority = normalized
        .split_once("://")
        .map_or("", |(_, r)| r.split(['/', '?', '#']).next().unwrap_or(""));
    if authority.contains('@') {
        return Err(BrowserError::Policy(
            "adres z danymi logowania jest zablokowany".into(),
        ));
    }
    url_host(u).ok_or_else(|| BrowserError::Policy("adres bez hosta".into()))
}

/// Filtr egressu przeglądarki: decyzja dla każdego żądania sieciowego (host zatwierdzony przez
/// Brokera). Wołany synchronicznie z wątku protokołu — bez blokowania.
pub trait EgressFilter: Send + Sync {
    /// Czy host może dostać żądanie.
    fn allows(&self, host: &str) -> bool;
}

/// Decyzja filtra dla adresu żądania: lokalne bez sieci → tak; sieć → host z filtra; inne
/// schematy (`file:`, `chrome:`, `ftp:`…) → nie.
pub fn request_allowed(filter: &dyn EgressFilter, url: &str) -> bool {
    if is_local_url(url) {
        return true;
    }
    url_host(url).is_some_and(|h| filter.allows(&h))
}

/// Sesja przeglądarki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BrowserSessionId(pub u64);

/// Węzeł drzewa dostępności strony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageNode {
    /// Identyfikator węzła (do `click`/`type_text`).
    pub node: u32,
    /// Głębokość.
    pub depth: u16,
    /// Rola (`link`, `button`, `textbox`…).
    pub role: String,
    /// Nazwa dostępna (treść niezaufana).
    pub name: String,
    /// Wartość pola (zawsze `None` dla pól haseł).
    pub value: Option<String>,
    /// Pole hasła.
    pub password: bool,
    /// Czy można kliknąć/wpisać.
    pub focusable: bool,
}

/// Pobranie (zawsze w kwarantannie).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadInfo {
    /// Nazwa proponowana przez stronę (treść niezaufana).
    pub suggested_name: String,
    /// Ścieżka w kwarantannie.
    pub path: PathBuf,
    /// Rozmiar (bajty).
    pub bytes: u64,
    /// Zakończone.
    pub complete: bool,
}

/// Stan strony po akcji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageInfo {
    /// Bieżący adres.
    pub url: String,
    /// Tytuł (treść niezaufana).
    pub title: String,
    /// Hosty zablokowane od ostatniej akcji (niezatwierdzone).
    pub blocked_hosts: Vec<String>,
    /// Nowe pobrania od ostatniej akcji.
    pub downloads: Vec<DownloadInfo>,
}

/// Migawka strony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSnapshot {
    /// Stan strony.
    pub page: PageInfo,
    /// Węzły dostępności (preorder).
    pub nodes: Vec<PageNode>,
    /// Tekst strony (≤ limit).
    pub text: String,
    /// Obcięto limitem.
    pub truncated: bool,
}

/// Port przeglądarki. Każda sesja ma własny filtr egressu; port nie udostępnia ciasteczek,
/// haseł ani pamięci strony.
pub trait BrowserPort: Send + Sync {
    /// Uruchamia przeglądarkę ([`BrowserSpec::validate`] najpierw).
    fn open(
        &self,
        spec: &BrowserSpec,
        egress: Arc<dyn EgressFilter>,
    ) -> Result<BrowserSessionId, BrowserError>;
    /// Nawigacja ([`check_navigation_url`]; host musi przejść filtr).
    fn navigate(&self, session: BrowserSessionId, url: &str) -> Result<PageInfo, BrowserError>;
    /// Drzewo dostępności i tekst.
    fn snapshot(
        &self,
        session: BrowserSessionId,
        max_nodes: usize,
        max_text: usize,
    ) -> Result<PageSnapshot, BrowserError>;
    /// Kliknięcie węzła.
    fn click(&self, session: BrowserSessionId, node: u32) -> Result<PageInfo, BrowserError>;
    /// Wpisanie tekstu w pole (nigdy pole hasła); `submit` = Enter.
    fn type_text(
        &self,
        session: BrowserSessionId,
        node: u32,
        text: &str,
        submit: bool,
    ) -> Result<PageInfo, BrowserError>;
    /// Zrzut widoku (PNG, dłuższy bok ≤ `max_side`).
    fn screenshot(&self, session: BrowserSessionId, max_side: u32)
    -> Result<Vec<u8>, BrowserError>;
    /// Zamyka przeglądarkę (całe drzewo procesów).
    fn close(&self, session: BrowserSessionId) -> Result<(), BrowserError>;
}
