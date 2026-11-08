//! Klient HTTPS z ochroną egressu (cecha `client`): resolver dla `reqwest`, który odrzuca host,
//! gdy którykolwiek adres z DNS jest niepubliczny, i oddaje do połączenia wyłącznie adresy z tego
//! samego rozwiązania (`hyper` nie pyta DNS drugi raz — brak okna na rebinding między sprawdzeniem
//! a połączeniem); klient tylko HTTPS, bez proxy systemowego, bez przekierowań (każdy nowy host
//! przechodzi przez Brokera w narzędziu), bez ciasteczek i bez nagłówka `Referer`.
//!
//! Uwaga: adres IP w URL omija resolver (`hyper` łączy się wprost) — dlatego [`crate::check_url`]
//! odrzuca niepubliczne adresy IP przed żądaniem.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::addr::check_resolved;

/// Przyszłość rozwiązania nazwy.
pub type LookupFuture = Pin<Box<dyn Future<Output = std::io::Result<Vec<IpAddr>>> + Send>>;

/// Źródło adresów dla nazwy (produkcyjnie DNS systemu; w testach — skrypt, np. rebinding).
pub trait Lookup: Send + Sync + 'static {
    /// Adresy hosta.
    fn lookup(&self, host: String) -> LookupFuture;
}

/// DNS systemu (`getaddrinfo` przez `tokio`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemLookup;

impl Lookup for SystemLookup {
    fn lookup(&self, host: String) -> LookupFuture {
        Box::pin(async move {
            Ok(tokio::net::lookup_host((host.as_str(), 443))
                .await?
                .map(|a| a.ip())
                .collect())
        })
    }
}

/// Resolver `reqwest` przepuszczający wyłącznie hosty z samymi adresami publicznymi.
pub struct PublicResolver<L: Lookup> {
    lookup: Arc<L>,
}

impl<L: Lookup> PublicResolver<L> {
    /// Resolver nad źródłem adresów.
    pub fn new(lookup: L) -> Self {
        Self {
            lookup: Arc::new(lookup),
        }
    }

    /// Rozwiązuje i sprawdza (do testów i diagnostyki).
    pub async fn resolve_checked(&self, host: &str) -> Result<Vec<IpAddr>, String> {
        let ips = self
            .lookup
            .lookup(host.to_owned())
            .await
            .map_err(|e| format!("DNS {host}: {e}"))?;
        check_resolved(host, &ips).map_err(|e| e.to_string())?;
        Ok(ips)
    }
}

impl<L: Lookup> reqwest::dns::Resolve for PublicResolver<L> {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        let lookup = self.lookup.clone();
        Box::pin(async move {
            let ips = lookup.lookup(host.clone()).await?;
            if let Err(e) = check_resolved(&host, &ips) {
                let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(e);
                return Err(boxed);
            }
            // Port nadpisuje `hyper` portem z adresu URL.
            let addrs: Vec<SocketAddr> = ips.into_iter().map(|ip| SocketAddr::new(ip, 0)).collect();
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// Ustawienia klienta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// Limit nawiązania połączenia (TCP + TLS).
    pub connect_timeout: Duration,
    /// Limit bezczynności odczytu.
    pub read_timeout: Duration,
    /// Limit całego żądania (`None` — pilnuje wywołujący, np. narzędzie z terminem).
    pub total_timeout: Option<Duration>,
    /// `User-Agent`.
    pub user_agent: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            total_timeout: None,
            user_agent: format!("Alfa/{} (agentka)", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Klient: tylko HTTPS, bez proxy, bez przekierowań, bez `Referer`, resolver z odrzucaniem adresów
/// niepublicznych.
pub fn guarded_client<L: Lookup>(
    config: &ClientConfig,
    lookup: L,
) -> Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .referer(false)
        .dns_resolver(Arc::new(PublicResolver::new(lookup)))
        .connect_timeout(config.connect_timeout)
        .read_timeout(config.read_timeout)
        .user_agent(config.user_agent.clone());
    let builder = match config.total_timeout {
        Some(t) => builder.timeout(t),
        None => builder,
    };
    builder.build().map_err(|e| e.to_string())
}

/// Czy błąd `reqwest` pochodzi z odmowy resolvera (adres niepubliczny) — łańcuch źródeł.
pub fn is_guard_rejection(err: &(dyn std::error::Error + 'static)) -> bool {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = cur {
        if e.downcast_ref::<crate::GuardError>().is_some() {
            return true;
        }
        cur = e.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Skrypt DNS: każde zapytanie zwraca kolejną odpowiedź (rebinding: publiczny → pętla).
    struct Scripted(std::sync::Mutex<Vec<Vec<IpAddr>>>);

    impl Lookup for Scripted {
        fn lookup(&self, _host: String) -> LookupFuture {
            let next = {
                let mut q = self.0.lock().unwrap_or_else(|p| p.into_inner());
                if q.is_empty() {
                    Vec::new()
                } else {
                    q.remove(0)
                }
            };
            Box::pin(async move { Ok(next) })
        }
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[tokio::test]
    async fn resolver_rejects_rebinding_and_mixed_records() {
        let r = PublicResolver::new(Scripted(std::sync::Mutex::new(vec![
            vec![ip("93.184.216.34")],
            vec![ip("127.0.0.1")],
            vec![ip("93.184.216.34"), ip("192.168.0.10")],
            vec![],
        ])));
        assert!(r.resolve_checked("zmienny.example").await.is_ok());
        let e = r.resolve_checked("zmienny.example").await.unwrap_err();
        assert!(e.contains("127.0.0.1"), "{e}");
        assert!(r.resolve_checked("zmienny.example").await.is_err());
        assert!(r.resolve_checked("zmienny.example").await.is_err());
    }

    #[tokio::test]
    async fn client_never_connects_to_rebound_private_address() {
        let lookup = Scripted(std::sync::Mutex::new(vec![vec![ip("127.0.0.1")]]));
        let client = guarded_client(&ClientConfig::default(), lookup).unwrap();
        let err = client
            .get("https://rebind.example/")
            .send()
            .await
            .unwrap_err();
        assert!(is_guard_rejection(&err), "{err:?}");
        let err = client.get("http://example.com/").send().await.unwrap_err();
        assert!(!is_guard_rejection(&err));
    }
}
