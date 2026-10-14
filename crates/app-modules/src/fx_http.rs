//! Klient HTTP kursu NBP dla `cost-meter` (rustls, bez OpenSSL; tylko GET, limit czasu 10 s).

use async_trait::async_trait;
use cost_meter_contract::{FxError, FxQuote, FxSource};
use cost_meter_impl::HttpGet;

/// `HttpGet` na `reqwest`.
pub struct ReqwestGet {
    client: reqwest::Client,
}

impl ReqwestGet {
    /// Nowy klient (błąd budowy TLS → `None`; licznik użyje kursu zapasowego).
    pub fn new() -> Option<Self> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .ok()
            .map(|client| Self { client })
    }
}

#[async_trait]
impl HttpGet for ReqwestGet {
    async fn get_text(&self, url: &str) -> Result<String, String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        response.text().await.map_err(|e| e.to_string())
    }
}

/// Źródło kursu bez sieci (testy, praca offline): zawsze błąd → kurs zapasowy z konfiguracji.
pub struct OfflineFx;

#[async_trait]
impl FxSource for OfflineFx {
    async fn fetch_usd_pln(&self) -> Result<FxQuote, FxError> {
        Err(FxError::Network("pobieranie kursu wyłączone".into()))
    }
}
