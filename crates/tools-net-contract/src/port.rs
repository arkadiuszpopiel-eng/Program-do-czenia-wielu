//! Porty sieci: klient HTTP **bez przekierowań** (narzędzie decyduje o każdym kolejnym hoście
//! przez Brokera) ze strumieniem treści (limit rozmiaru egzekwuje narzędzie, odczyt można
//! przerwać) i wyszukiwarka (dostawca do podpięcia później).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Metoda HTTP (bez metod wysyłających treść — v2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HttpMethod {
    /// `GET`.
    Get,
    /// `HEAD` (tylko nagłówki).
    Head,
}

/// Żądanie (adres już sprawdzony `lib_netguard::check_url`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// Adres `https://…`.
    pub url: String,
    /// Metoda.
    pub method: HttpMethod,
}

/// Błąd sieci.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetError {
    /// Adres zablokowany (niepubliczny po DNS — rebinding, schemat, host lokalny).
    #[error("adres zablokowany: {0}")]
    Blocked(String),
    /// Przekroczony limit czasu.
    #[error("przekroczony limit czasu")]
    Timeout,
    /// Połączenie (DNS, TCP, TLS).
    #[error("połączenie nie powiodło się: {0}")]
    Connect(String),
    /// Odczyt odpowiedzi.
    #[error("odczyt odpowiedzi: {0}")]
    Read(String),
    /// Nieobsługiwane (brak klienta albo dostawcy).
    #[error("nieobsługiwane: {0}")]
    Unsupported(String),
}

/// Strumień treści odpowiedzi.
#[async_trait]
pub trait BodyReader: Send {
    /// Następny fragment (`None` = koniec treści).
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, NetError>;
}

/// Odpowiedź (nagłówki + strumień treści).
pub struct HttpResponse {
    /// Status HTTP.
    pub status: u16,
    /// `Content-Type`.
    pub content_type: Option<String>,
    /// `Content-Length` (deklarowany przez serwer — niezaufany).
    pub content_length: Option<u64>,
    /// `Location` (przekierowania).
    pub location: Option<String>,
    /// `Content-Disposition`.
    pub content_disposition: Option<String>,
    /// Treść.
    pub body: Box<dyn BodyReader>,
}

impl std::fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("content_length", &self.content_length)
            .field("location", &self.location)
            .finish_non_exhaustive()
    }
}

impl HttpResponse {
    /// Czy to przekierowanie z adresem docelowym.
    pub fn redirect_location(&self) -> Option<&str> {
        matches!(self.status, 301 | 302 | 303 | 307 | 308)
            .then_some(self.location.as_deref())
            .flatten()
    }
}

/// Klient HTTPS (produkcyjnie `tools-net-impl::ReqwestHttp`: bez proxy, bez przekierowań,
/// resolver odrzucający adresy niepubliczne).
#[async_trait]
pub trait HttpPort: Send + Sync {
    /// Wysyła żądanie; nigdy nie podąża za przekierowaniem.
    async fn send(&self, request: &HttpRequest) -> Result<HttpResponse, NetError>;
}

/// Wynik wyszukiwania (niezaufany).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchHit {
    /// Tytuł.
    pub title: String,
    /// Adres.
    pub url: String,
    /// Fragment.
    pub snippet: String,
}

/// Wyszukiwarka (dostawca do podpięcia później; klucz w sejfie, trasa w `compliance`).
#[async_trait]
pub trait SearchPort: Send + Sync {
    /// Host dostawcy (zgoda `net.egress`); `None` = brak dostawcy.
    fn endpoint_host(&self) -> Option<String>;

    /// Wyszukuje.
    async fn search(&self, query: &str, max: u32) -> Result<Vec<SearchHit>, NetError>;
}

/// Brak dostawcy wyszukiwania.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSearch;

#[async_trait]
impl SearchPort for NoSearch {
    fn endpoint_host(&self) -> Option<String> {
        None
    }

    async fn search(&self, _query: &str, _max: u32) -> Result<Vec<SearchHit>, NetError> {
        Err(NetError::Unsupported(
            "wyszukiwarka nie jest skonfigurowana".into(),
        ))
    }
}
