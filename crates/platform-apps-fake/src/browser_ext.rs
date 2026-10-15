//! Nowe operacje atrapy przeglądarki (C6.4): przewijanie, klawisze, lista wyboru, historia karty,
//! wyszukiwanie tekstu i konsola. Każda operacja trafia do dziennika ([`ActionLog`]).

use platform_apps_contract::{
    BrowserError, BrowserKey, BrowserSessionId, ConsoleEntry, HistoryNav, PageInfo,
};

use super::{State, go, info, node, session};

/// Dziennik operacji (`scroll:+600`, `key:Enter`, `select:3=PL`, `history:Back`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionLog(pub Vec<String>);

pub(super) fn scroll(
    s: &mut State,
    id: BrowserSessionId,
    n: Option<u32>,
    dy: i32,
) -> Result<PageInfo, BrowserError> {
    if let Some(n) = n {
        node(s, id, n)?;
        s.actions.push(format!("scroll:node{n}"));
    } else {
        session(s, id)?;
        s.actions.push(format!("scroll:{dy:+}"));
    }
    info(s, id)
}

pub(super) fn press_key(
    s: &mut State,
    id: BrowserSessionId,
    key: BrowserKey,
) -> Result<PageInfo, BrowserError> {
    session(s, id)?;
    s.actions.push(format!("key:{key:?}"));
    // Enter po wpisaniu w pole z akcją formularza — wysłanie jak `submit`.
    if key == BrowserKey::Enter
        && let Some((n, _)) = s.typed.last().cloned()
        && let Ok(target) = node(s, id, n)
        && let Some(action) = target.href
    {
        go(s, id, &action)?;
    }
    info(s, id)
}

pub(super) fn select_option(
    s: &mut State,
    id: BrowserSessionId,
    n: u32,
    value: &str,
) -> Result<PageInfo, BrowserError> {
    let target = node(s, id, n)?;
    if target.options.is_empty() {
        return Err(BrowserError::NotFound(format!(
            "węzeł {n} nie jest listą wyboru — użyj kliknięcia"
        )));
    }
    if !target.options.iter().any(|o| o.eq_ignore_ascii_case(value)) {
        return Err(BrowserError::NotFound(format!(
            "lista {n} nie ma opcji „{value}” (dostępne: {})",
            target.options.join(", ")
        )));
    }
    s.actions.push(format!("select:{n}={value}"));
    info(s, id)
}

pub(super) fn history(
    s: &mut State,
    id: BrowserSessionId,
    nav: HistoryNav,
) -> Result<PageInfo, BrowserError> {
    s.actions.push(format!("history:{nav:?}"));
    let sess = session(s, id)?;
    let moved = match nav {
        HistoryNav::Reload => true,
        HistoryNav::Back => match sess.back.pop() {
            Some(prev) => {
                let cur = std::mem::replace(&mut sess.url, prev);
                sess.forward.push(cur);
                true
            }
            None => false,
        },
        HistoryNav::Forward => match sess.forward.pop() {
            Some(next) => {
                let cur = std::mem::replace(&mut sess.url, next);
                sess.back.push(cur);
                true
            }
            None => false,
        },
    };
    if !moved {
        return Err(BrowserError::NotFound(format!(
            "brak strony w historii ({nav:?})"
        )));
    }
    info(s, id)
}

pub(super) fn has_text(
    s: &mut State,
    id: BrowserSessionId,
    text: &str,
) -> Result<bool, BrowserError> {
    let url = session(s, id)?.url.clone();
    let needle = text.to_lowercase();
    Ok(s.pages.get(&url).is_some_and(|p| {
        p.text.to_lowercase().contains(&needle)
            || p.nodes
                .iter()
                .any(|n| n.name.to_lowercase().contains(&needle))
    }))
}

pub(super) fn console(
    s: &mut State,
    id: BrowserSessionId,
    max: usize,
) -> Result<Vec<ConsoleEntry>, BrowserError> {
    let sess = session(s, id)?;
    let take = sess.console.len().min(max);
    Ok(sess.console.drain(..take).collect())
}
