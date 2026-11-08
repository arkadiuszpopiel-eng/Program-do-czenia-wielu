//! Wirtualna sieć dla `HttpPort` i wyszukiwarka skryptowana.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use tools_net_contract::{
    BodyReader, HttpMethod, HttpPort, HttpRequest, HttpResponse, NetError, SearchHit, SearchPort,
};

/// Fragment treści nieskończonej.
const ENDLESS_CHUNK: usize = 64 * 1024;

/// Odpowiedź trasy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeRoute {
    /// Treść.
    Body {
        /// Status.
        status: u16,
        /// `Content-Type`.
        content_type: Option<String>,
        /// Treść.
        body: Vec<u8>,
        /// Deklarowany `Content-Length` (może kłamać).
        declared_len: Option<u64>,
        /// `Content-Disposition`.
        disposition: Option<String>,
    },
    /// Przekierowanie.
    Redirect {
        /// Status (301/302/303/307/308).
        status: u16,
        /// `Location`.
        location: String,
    },
    /// Treść bez końca (bomba rozmiarowa).
    Endless,
    /// Nagłówki są, treść nie przychodzi nigdy.
    HangBody,
    /// Odpowiedź nie przychodzi nigdy.
    HangHeaders,
    /// Błąd sieci.
    Fail(NetError),
}

#[derive(Debug, Default)]
struct State {
    routes: BTreeMap<String, FakeRoute>,
    private: BTreeSet<String>,
    rebind_after: BTreeMap<String, usize>,
    seen: BTreeMap<String, usize>,
    log: Vec<(HttpMethod, String)>,
    blocked: Vec<String>,
}

/// Wirtualna sieć.
#[derive(Debug, Default)]
pub struct FakeHttp {
    state: Mutex<State>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl FakeHttp {
    /// Trasa dla adresu (dokładnie jak po `check_url`, np. `https://a.pl/`).
    pub fn route(&self, url: &str, route: FakeRoute) {
        lock(&self.state).routes.insert(url.to_owned(), route);
    }

    /// Trasa z treścią 200.
    pub fn page(&self, url: &str, content_type: &str, body: &[u8]) {
        self.route(
            url,
            FakeRoute::Body {
                status: 200,
                content_type: Some(content_type.to_owned()),
                body: body.to_vec(),
                declared_len: Some(body.len() as u64),
                disposition: None,
            },
        );
    }

    /// Host „rozwiązywany” na adres niepubliczny (odmowa resolvera, zero połączeń).
    pub fn private_host(&self, host: &str) {
        lock(&self.state).private.insert(host.to_owned());
    }

    /// DNS rebinding: po `n` udanych żądaniach host zaczyna wskazywać adres niepubliczny.
    pub fn rebind_after(&self, host: &str, n: usize) {
        lock(&self.state).rebind_after.insert(host.to_owned(), n);
    }

    /// Żądania, które doszły do „serwerów” (metoda, adres).
    pub fn requests(&self) -> Vec<(HttpMethod, String)> {
        lock(&self.state).log.clone()
    }

    /// Hosty, z którymi nawiązano połączenie.
    pub fn hosts_contacted(&self) -> BTreeSet<String> {
        lock(&self.state)
            .log
            .iter()
            .filter_map(|(_, u)| lib_netguard::check_url(u).ok().map(|t| t.host().to_owned()))
            .collect()
    }

    /// Hosty odrzucone przez „resolver”.
    pub fn blocked(&self) -> Vec<String> {
        lock(&self.state).blocked.clone()
    }
}

struct Bytes(Option<Vec<u8>>);

#[async_trait]
impl BodyReader for Bytes {
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, NetError> {
        Ok(self.0.take().filter(|b| !b.is_empty()))
    }
}

struct Endless;

#[async_trait]
impl BodyReader for Endless {
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, NetError> {
        tokio_yield().await;
        Ok(Some(vec![b'a'; ENDLESS_CHUNK]))
    }
}

struct Hang;

#[async_trait]
impl BodyReader for Hang {
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, NetError> {
        std::future::pending::<()>().await;
        Ok(None)
    }
}

/// Oddaje sterowanie wykonawcy (treść nieskończona nie głodzi anulowania).
async fn tokio_yield() {
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if yielded {
            std::task::Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
    .await;
}

fn response(status: u16, body: Box<dyn BodyReader>) -> HttpResponse {
    HttpResponse {
        status,
        content_type: None,
        content_length: None,
        location: None,
        content_disposition: None,
        body,
    }
}

#[async_trait]
impl HttpPort for FakeHttp {
    async fn send(&self, request: &HttpRequest) -> Result<HttpResponse, NetError> {
        let target =
            lib_netguard::check_url(&request.url).map_err(|e| NetError::Blocked(e.to_string()))?;
        let route = {
            let mut s = lock(&self.state);
            let host = target.host().to_owned();
            let count = s.seen.get(&host).copied().unwrap_or(0);
            let rebound = s.rebind_after.get(&host).is_some_and(|n| count >= *n);
            if s.private.contains(&host) || rebound {
                s.blocked.push(host.clone());
                return Err(NetError::Blocked(format!(
                    "host {host} wskazuje adres niepubliczny 127.0.0.1"
                )));
            }
            s.seen.insert(host, count + 1);
            s.log.push((request.method, target.as_str().to_owned()));
            s.routes.get(target.as_str()).cloned()
        };
        let head = request.method == HttpMethod::Head;
        match route {
            None => Ok(response(404, Box::new(Bytes(None)))),
            Some(FakeRoute::Body {
                status,
                content_type,
                body,
                declared_len,
                disposition,
            }) => Ok(HttpResponse {
                status,
                content_type,
                content_length: declared_len,
                location: None,
                content_disposition: disposition,
                body: Box::new(Bytes((!head).then_some(body))),
            }),
            Some(FakeRoute::Redirect { status, location }) => Ok(HttpResponse {
                location: Some(location),
                ..response(status, Box::new(Bytes(None)))
            }),
            Some(FakeRoute::Endless) => Ok(response(200, Box::new(Endless))),
            Some(FakeRoute::HangBody) => Ok(response(200, Box::new(Hang))),
            Some(FakeRoute::HangHeaders) => {
                std::future::pending::<()>().await;
                Err(NetError::Timeout)
            }
            Some(FakeRoute::Fail(e)) => Err(e),
        }
    }
}

/// Wyszukiwarka skryptowana.
#[derive(Debug)]
pub struct FakeSearch {
    host: String,
    hits: Vec<SearchHit>,
    queries: Mutex<Vec<String>>,
}

impl FakeSearch {
    /// Dostawca na hoście `host` zwracający `hits`.
    pub fn new(host: &str, hits: Vec<SearchHit>) -> Self {
        Self {
            host: host.to_owned(),
            hits,
            queries: Mutex::new(Vec::new()),
        }
    }

    /// Zapytania, które doszły do dostawcy.
    pub fn queries(&self) -> Vec<String> {
        lock(&self.queries).clone()
    }
}

#[async_trait]
impl SearchPort for FakeSearch {
    fn endpoint_host(&self) -> Option<String> {
        Some(self.host.clone())
    }

    async fn search(&self, query: &str, max: u32) -> Result<Vec<SearchHit>, NetError> {
        lock(&self.queries).push(query.to_owned());
        Ok(self.hits.iter().take(max as usize).cloned().collect())
    }
}
