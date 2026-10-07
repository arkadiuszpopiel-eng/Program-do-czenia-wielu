//! Atrapa `BrowserPort`: wirtualna sieć stron (zasoby, linki, formularze, pobrania), każde
//! żądanie przez filtr egressu sesji (dziennik: adres + decyzja), argumenty uruchomienia z
//! kontraktu (`chromium_args`), pobrania w kwarantannie, pola haseł bez wartości i bez wpisywania.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use platform_apps_contract::{
    BrowserError, BrowserPort, BrowserSessionId, BrowserSpec, DownloadInfo, EgressFilter, PageInfo,
    PageNode, PageSnapshot, check_navigation_url, chromium_args, request_allowed, url_host,
};

/// Element strony.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FakeNode {
    /// Rola.
    pub role: String,
    /// Nazwa.
    pub name: String,
    /// Wartość pola.
    pub value: Option<String>,
    /// Pole hasła.
    pub password: bool,
    /// Link / akcja formularza (kliknięcie, `submit`).
    pub href: Option<String>,
    /// Pobranie po kliknięciu: (adres, nazwa, rozmiar).
    pub download: Option<(String, String, u64)>,
}

impl FakeNode {
    /// Element o roli i nazwie.
    pub fn new(role: &str, name: &str) -> Self {
        Self {
            role: role.into(),
            name: name.into(),
            ..Self::default()
        }
    }
}

/// Strona wirtualnej sieci.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FakePage {
    /// Tytuł.
    pub title: String,
    /// Tekst.
    pub text: String,
    /// Elementy.
    pub nodes: Vec<FakeNode>,
    /// Zasoby ładowane przy otwarciu (skrypty, obrazy, śledzenie).
    pub resources: Vec<String>,
}

struct Session {
    filter: Arc<dyn EgressFilter>,
    spec: BrowserSpec,
    url: String,
    blocked: Vec<String>,
    downloads: Vec<DownloadInfo>,
}

#[derive(Default)]
struct State {
    pages: BTreeMap<String, FakePage>,
    sessions: BTreeMap<u64, Session>,
    next: u64,
    launches: Vec<Vec<String>>,
    network: Vec<(String, bool)>,
    typed: Vec<(u32, String)>,
    quarantine: Vec<(PathBuf, u64)>,
}

/// Atrapa przeglądarki.
#[derive(Default)]
pub struct FakeBrowser {
    state: Mutex<State>,
}

impl std::fmt::Debug for FakeBrowser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeBrowser").finish_non_exhaustive()
    }
}

impl FakeBrowser {
    /// Pusta sieć.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Dodaje stronę pod adresem.
    pub fn add_page(&self, url: &str, page: FakePage) {
        self.lock().pages.insert(url.to_owned(), page);
    }

    /// Argumenty każdego uruchomienia.
    pub fn launches(&self) -> Vec<Vec<String>> {
        self.lock().launches.clone()
    }

    /// Dziennik sieci: (adres, czy przepuszczony przez filtr).
    pub fn network(&self) -> Vec<(String, bool)> {
        self.lock().network.clone()
    }

    /// Wpisane teksty (węzeł, tekst).
    pub fn typed(&self) -> Vec<(u32, String)> {
        self.lock().typed.clone()
    }

    /// Pliki zapisane w kwarantannie (ścieżka, rozmiar).
    pub fn quarantine(&self) -> Vec<(PathBuf, u64)> {
        self.lock().quarantine.clone()
    }

    /// Otwarte sesje.
    pub fn open_sessions(&self) -> usize {
        self.lock().sessions.len()
    }
}

fn session(s: &mut State, id: BrowserSessionId) -> Result<&mut Session, BrowserError> {
    s.sessions
        .get_mut(&id.0)
        .ok_or_else(|| BrowserError::NotFound(format!("sesja {}", id.0)))
}

/// Żądanie przez filtr; zablokowane hosty trafiają do sesji.
fn request(s: &mut State, id: BrowserSessionId, url: &str) -> Result<bool, BrowserError> {
    let sess = session(s, id)?;
    let ok = request_allowed(sess.filter.as_ref(), url);
    if !ok {
        let host = url_host(url).unwrap_or_else(|| url.chars().take(64).collect());
        if !sess.blocked.contains(&host) {
            sess.blocked.push(host);
        }
    }
    s.network.push((url.to_owned(), ok));
    Ok(ok)
}

fn info(s: &mut State, id: BrowserSessionId) -> Result<PageInfo, BrowserError> {
    let title = {
        let url = session(s, id)?.url.clone();
        s.pages
            .get(&url)
            .map(|p| p.title.clone())
            .unwrap_or_default()
    };
    let sess = session(s, id)?;
    Ok(PageInfo {
        url: sess.url.clone(),
        title,
        blocked_hosts: std::mem::take(&mut sess.blocked),
        downloads: std::mem::take(&mut sess.downloads),
    })
}

fn go(s: &mut State, id: BrowserSessionId, url: &str) -> Result<(), BrowserError> {
    if !request(s, id, url)? {
        return Ok(());
    }
    session(s, id)?.url = url.to_owned();
    let resources = s
        .pages
        .get(url)
        .map(|p| p.resources.clone())
        .unwrap_or_default();
    for r in resources {
        request(s, id, &r)?;
    }
    Ok(())
}

fn node(s: &State, id: BrowserSessionId, n: u32) -> Result<FakeNode, BrowserError> {
    let url = s
        .sessions
        .get(&id.0)
        .map(|x| x.url.clone())
        .ok_or_else(|| BrowserError::NotFound(format!("sesja {}", id.0)))?;
    let idx = usize::try_from(n).ok().and_then(|i| i.checked_sub(1));
    s.pages
        .get(&url)
        .and_then(|p| idx.and_then(|i| p.nodes.get(i)))
        .cloned()
        .ok_or_else(|| BrowserError::NotFound(format!("węzeł {n}")))
}

impl BrowserPort for FakeBrowser {
    fn open(
        &self,
        spec: &BrowserSpec,
        egress: Arc<dyn EgressFilter>,
    ) -> Result<BrowserSessionId, BrowserError> {
        spec.validate()?;
        let mut s = self.lock();
        s.launches.push(chromium_args(spec));
        s.next += 1;
        let id = s.next;
        s.sessions.insert(
            id,
            Session {
                filter: egress,
                spec: spec.clone(),
                url: "about:blank".into(),
                blocked: Vec::new(),
                downloads: Vec::new(),
            },
        );
        Ok(BrowserSessionId(id))
    }

    fn navigate(&self, id: BrowserSessionId, url: &str) -> Result<PageInfo, BrowserError> {
        let host = check_navigation_url(url)?;
        let mut s = self.lock();
        if !session(&mut s, id)?.filter.allows(&host) {
            s.network.push((url.to_owned(), false));
            return Err(BrowserError::Blocked(host));
        }
        go(&mut s, id, url)?;
        info(&mut s, id)
    }

    fn snapshot(
        &self,
        id: BrowserSessionId,
        max_nodes: usize,
        max_text: usize,
    ) -> Result<PageSnapshot, BrowserError> {
        let mut s = self.lock();
        let url = session(&mut s, id)?.url.clone();
        let page = s.pages.get(&url).cloned().unwrap_or_default();
        let nodes: Vec<PageNode> = page
            .nodes
            .iter()
            .zip(1u32..)
            .take(max_nodes)
            .map(|(n, i)| PageNode {
                node: i,
                depth: 1,
                role: n.role.clone(),
                name: n.name.clone(),
                value: if n.password { None } else { n.value.clone() },
                password: n.password,
                focusable: n.href.is_some() || n.download.is_some() || n.role == "textbox",
            })
            .collect();
        let truncated = page.nodes.len() > max_nodes || page.text.chars().count() > max_text;
        let text = page.text.chars().take(max_text).collect();
        Ok(PageSnapshot {
            page: info(&mut s, id)?,
            nodes,
            text,
            truncated,
        })
    }

    fn click(&self, id: BrowserSessionId, n: u32) -> Result<PageInfo, BrowserError> {
        let mut s = self.lock();
        let target = node(&s, id, n)?;
        if let Some((url, name, bytes)) = &target.download {
            if request(&mut s, id, url)? {
                let dir = session(&mut s, id)?.spec.quarantine_dir.clone();
                let path = dir.join(format!("pobranie-{}-{n}.bin", id.0));
                s.quarantine.push((path.clone(), *bytes));
                session(&mut s, id)?.downloads.push(DownloadInfo {
                    suggested_name: name.clone(),
                    path,
                    bytes: *bytes,
                    complete: true,
                });
            }
        } else if let Some(href) = &target.href {
            go(&mut s, id, href)?;
        }
        info(&mut s, id)
    }

    fn type_text(
        &self,
        id: BrowserSessionId,
        n: u32,
        text: &str,
        submit: bool,
    ) -> Result<PageInfo, BrowserError> {
        let mut s = self.lock();
        let target = node(&s, id, n)?;
        if target.password {
            return Err(BrowserError::PasswordField);
        }
        s.typed.push((n, text.to_owned()));
        if submit && let Some(action) = &target.href {
            go(&mut s, id, action)?;
        }
        info(&mut s, id)
    }

    fn screenshot(&self, id: BrowserSessionId, _max_side: u32) -> Result<Vec<u8>, BrowserError> {
        let mut s = self.lock();
        session(&mut s, id)?;
        Ok(b"\x89PNG\r\n\x1a\nATRAPA".to_vec())
    }

    fn close(&self, id: BrowserSessionId) -> Result<(), BrowserError> {
        self.lock()
            .sessions
            .remove(&id.0)
            .map(|_| ())
            .ok_or_else(|| BrowserError::NotFound(format!("sesja {}", id.0)))
    }
}
