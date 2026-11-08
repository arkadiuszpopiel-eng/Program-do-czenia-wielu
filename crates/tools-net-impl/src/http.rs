//! `HttpPort` produkcyjny: klient `reqwest` z `lib-netguard` (tylko HTTPS, bez proxy, bez
//! przekierowań, bez `Referer`, resolver odrzucający host z jakimkolwiek adresem niepublicznym)
//! i port zastępczy, gdy klienta nie da się zbudować.

use async_trait::async_trait;
use lib_netguard::client::{
    ClientConfig, Lookup, SystemLookup, guarded_client, is_guard_rejection,
};
use tools_net_contract::{BodyReader, HttpMethod, HttpPort, HttpRequest, HttpResponse, NetError};

/// Klient HTTPS agentek.
#[derive(Debug, Clone)]
pub struct ReqwestHttp {
    client: reqwest::Client,
}

impl ReqwestHttp {
    /// Klient z DNS systemu.
    pub fn system() -> Result<Self, String> {
        Self::with_lookup(SystemLookup)
    }

    /// Klient z własnym źródłem adresów (testy: rebinding).
    pub fn with_lookup<L: Lookup>(lookup: L) -> Result<Self, String> {
        guarded_client(&ClientConfig::default(), lookup).map(|client| Self { client })
    }
}

fn map_err(e: &reqwest::Error) -> NetError {
    let msg = e.to_string();
    if is_guard_rejection(e) {
        let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(e);
        let mut detail = msg.clone();
        while let Some(x) = cur {
            if x.downcast_ref::<lib_netguard::GuardError>().is_some() {
                detail = x.to_string();
            }
            cur = x.source();
        }
        NetError::Blocked(detail)
    } else if e.is_timeout() {
        NetError::Timeout
    } else if e.is_connect() || e.is_builder() || e.is_request() {
        NetError::Connect(msg)
    } else {
        NetError::Read(msg)
    }
}

struct Body(reqwest::Response);

#[async_trait]
impl BodyReader for Body {
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, NetError> {
        self.0
            .chunk()
            .await
            .map(|c| c.map(|b| b.to_vec()))
            .map_err(|e| map_err(&e))
    }
}

/// Limit długości nagłówka przekazywanego dalej (znaki).
const HEADER_MAX: usize = 2_048;

fn header(resp: &reqwest::Response, name: reqwest::header::HeaderName) -> Option<String> {
    resp.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(HEADER_MAX).collect())
}

/// Cel przekierowania w całości albo odmowa: adres obcięty do limitu nagłówka byłby innym
/// adresem niż ten, który podał serwer.
fn location(value: Option<&str>) -> Result<Option<String>, NetError> {
    match value {
        Some(v) if v.chars().count() > HEADER_MAX => Err(NetError::Blocked(format!(
            "cel przekierowania dłuższy niż {HEADER_MAX} znaków"
        ))),
        other => Ok(other.map(str::to_owned)),
    }
}

#[async_trait]
impl HttpPort for ReqwestHttp {
    async fn send(&self, request: &HttpRequest) -> Result<HttpResponse, NetError> {
        let target =
            lib_netguard::check_url(&request.url).map_err(|e| NetError::Blocked(e.to_string()))?;
        let method = match request.method {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Head => reqwest::Method::HEAD,
        };
        let resp = self
            .client
            .request(method, target.url().clone())
            .header(reqwest::header::ACCEPT, "*/*")
            .send()
            .await
            .map_err(|e| map_err(&e))?;
        use reqwest::header::{CONTENT_DISPOSITION, CONTENT_TYPE, LOCATION};
        Ok(HttpResponse {
            status: resp.status().as_u16(),
            content_type: header(&resp, CONTENT_TYPE),
            content_length: resp.content_length(),
            location: location(resp.headers().get(LOCATION).and_then(|v| v.to_str().ok()))?,
            content_disposition: header(&resp, CONTENT_DISPOSITION),
            body: Box::new(Body(resp)),
        })
    }
}

/// Port zastępczy (klient niedostępny) — każde żądanie: „nieobsługiwane”, bez ruchu.
#[derive(Debug, Clone)]
pub struct NoHttp(pub String);

#[async_trait]
impl HttpPort for NoHttp {
    async fn send(&self, _request: &HttpRequest) -> Result<HttpResponse, NetError> {
        Err(NetError::Unsupported(format!(
            "klient sieci niedostępny: {}",
            self.0
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::sync::Mutex;

    use lib_netguard::client::LookupFuture;

    use super::*;

    /// N-02 (docs/reviews/2026-10-08-tools-shell-clipboard-net-review.md): za długi cel
    /// przekierowania to odmowa — obcięty adres byłby innym adresem.
    #[test]
    fn overlong_redirect_target_is_refused_not_truncated() {
        assert_eq!(location(None).ok(), Some(None));
        let short = "https://example.org/a?b=c";
        assert_eq!(location(Some(short)).ok(), Some(Some(short.to_owned())));
        let limit = format!("https://example.org/{}", "a".repeat(HEADER_MAX - 20));
        assert_eq!(limit.chars().count(), HEADER_MAX);
        assert_eq!(location(Some(&limit)).ok(), Some(Some(limit.clone())));
        let long = format!("{limit}x");
        assert!(matches!(location(Some(&long)), Err(NetError::Blocked(_))));
    }

    struct Answers(Mutex<Vec<Vec<IpAddr>>>);

    impl Lookup for Answers {
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

    fn req(url: &str) -> HttpRequest {
        HttpRequest {
            url: url.into(),
            method: HttpMethod::Get,
        }
    }

    #[tokio::test]
    async fn real_client_blocks_private_resolution_and_bad_urls_without_traffic() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        let http = ReqwestHttp::with_lookup(Answers(Mutex::new(vec![
            vec![ip("10.0.0.7")],
            vec![ip("93.184.216.34"), ip("169.254.169.254")],
        ])))
        .unwrap();
        for url in ["https://rebind.example/", "https://mieszany.example/"] {
            let e = http.send(&req(url)).await.unwrap_err();
            assert!(
                matches!(&e, NetError::Blocked(m) if m.contains("niepubliczny")),
                "{e:?}"
            );
        }
        for url in [
            "http://example.com/",
            "https://127.0.0.1/",
            "https://localhost/",
        ] {
            assert!(matches!(
                http.send(&req(url)).await,
                Err(NetError::Blocked(_))
            ));
        }
        let none = NoHttp("test".into());
        assert!(matches!(
            none.send(&req("https://example.com/")).await,
            Err(NetError::Unsupported(_))
        ));
    }
}
