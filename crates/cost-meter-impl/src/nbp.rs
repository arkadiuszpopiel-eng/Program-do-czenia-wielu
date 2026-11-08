//! Źródło kursu NBP (tabela A) przez port HTTP dostarczany przez kompozycję (egress przez Brokera:
//! `net.egress(api.nbp.pl)`), z limitem czasu.

use std::time::Duration;

use async_trait::async_trait;
use cost_meter_contract::{FxError, FxQuote, FxSource, NBP_USD_URL, parse_nbp_json};

/// Minimalny port HTTP GET (implementuje go kompozycja wybranym klientem HTTP).
#[async_trait]
pub trait HttpGet: Send + Sync {
    /// Treść odpowiedzi 2xx jako tekst; inaczej opis błędu.
    async fn get_text(&self, url: &str) -> Result<String, String>;
}

/// Kurs USD/PLN z NBP.
pub struct NbpFxSource<H: HttpGet> {
    http: H,
    timeout: Duration,
}

impl<H: HttpGet> NbpFxSource<H> {
    /// Źródło z klientem HTTP i limitem czasu 10 s.
    pub fn new(http: H) -> Self {
        Self {
            http,
            timeout: Duration::from_secs(10),
        }
    }

    /// Zmienia limit czasu.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl<H: HttpGet> FxSource for NbpFxSource<H> {
    async fn fetch_usd_pln(&self) -> Result<FxQuote, FxError> {
        let body = tokio::time::timeout(self.timeout, self.http.get_text(NBP_USD_URL))
            .await
            .map_err(|_| FxError::Network("przekroczony czas odpowiedzi NBP".into()))?
            .map_err(FxError::Network)?;
        parse_nbp_json(&body)
    }
}
