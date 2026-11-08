//! Dziennik zdarzeń (tylko Windows): `EvtQuery` (kanał `Application`/`System`, XPath z wartości
//! sprawdzonych w kontrakcie, od najnowszych), `EvtRender` do XML-a (dekodowanie przenośne
//! w `events_xml`) i komunikat `EvtFormatMessage` z metadanych dostawcy (pamięć podręczna na
//! czas zapytania; brak metadanych → dane zdarzenia).

#![allow(unsafe_code)]

use std::collections::HashMap;

use platform_apps_contract::{EventRecord, SysError};
use windows::Win32::Foundation::{ERROR_EVT_CHANNEL_NOT_FOUND, ERROR_NO_MORE_ITEMS};
use windows::Win32::System::EventLog::{
    EVT_HANDLE, EvtClose, EvtFormatMessage, EvtFormatMessageEvent, EvtNext,
    EvtOpenPublisherMetadata, EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender,
    EvtRenderEventXml,
};
use windows::core::{HRESULT, PCWSTR};

use crate::events_xml::record_from_xml;
use crate::procs_win::sys_error;

/// Najdłuższy XML jednego zdarzenia (bajty).
const MAX_XML: u32 = 1 << 20;
/// Limit czekania `EvtNext` (ms).
const NEXT_TIMEOUT_MS: u32 = 2_000;

struct Evt(EVT_HANDLE);

impl Drop for Evt {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: uchwyt zapytania/zdarzenia/metadanych otwarty przez nas, zamykany raz.
            let _ = unsafe { EvtClose(self.0) };
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn render_xml(ev: &Evt) -> Option<String> {
    let (mut used, mut props) = (0u32, 0u32);
    // SAFETY: zapytanie o rozmiar (bez bufora).
    let _ = unsafe {
        EvtRender(
            None,
            ev.0,
            EvtRenderEventXml.0,
            0,
            None,
            &raw mut used,
            &raw mut props,
        )
    };
    if used == 0 || used > MAX_XML {
        return None;
    }
    let mut buf = vec![0u16; (used as usize).div_ceil(2)];
    let size = u32::try_from(buf.len() * 2).ok()?;
    // SAFETY: bufor o rozmiarze `size` bajtów.
    unsafe {
        EvtRender(
            None,
            ev.0,
            EvtRenderEventXml.0,
            size,
            Some(buf.as_mut_ptr().cast()),
            &raw mut used,
            &raw mut props,
        )
    }
    .ok()?;
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

fn publisher(name: &str, cache: &mut HashMap<String, Option<Evt>>) -> Option<EVT_HANDLE> {
    let entry = cache.entry(name.to_owned()).or_insert_with(|| {
        let w = wide(name);
        // SAFETY: nazwa dostawcy zakończona zerem; lokalne metadane.
        unsafe { EvtOpenPublisherMetadata(None, PCWSTR(w.as_ptr()), PCWSTR::null(), 0, 0) }
            .ok()
            .map(Evt)
    });
    entry.as_ref().map(|e| e.0)
}

fn message(publisher: EVT_HANDLE, ev: &Evt) -> Option<String> {
    let mut used = 0u32;
    // SAFETY: zapytanie o rozmiar (bez bufora).
    let _ = unsafe {
        EvtFormatMessage(
            Some(publisher),
            Some(ev.0),
            0,
            None,
            EvtFormatMessageEvent.0,
            None,
            &raw mut used,
        )
    };
    if used == 0 || used > 1 << 16 {
        return None;
    }
    let mut buf = vec![0u16; used as usize];
    // SAFETY: bufor o długości `used` znaków.
    unsafe {
        EvtFormatMessage(
            Some(publisher),
            Some(ev.0),
            0,
            None,
            EvtFormatMessageEvent.0,
            Some(&mut buf),
            &raw mut used,
        )
    }
    .ok()?;
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// Zdarzenia z kanału (najnowsze pierwsze), najwyżej `max`.
pub(crate) fn query(channel: &str, xpath: &str, max: u32) -> Result<Vec<EventRecord>, SysError> {
    let (c, x) = (wide(channel), wide(xpath));
    let flags = EvtQueryChannelPath.0 | EvtQueryReverseDirection.0;
    // SAFETY: kanał i zapytanie zakończone zerem żyją do końca wywołania.
    let q = match unsafe { EvtQuery(None, PCWSTR(c.as_ptr()), PCWSTR(x.as_ptr()), flags) } {
        Ok(h) => Evt(h),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_EVT_CHANNEL_NOT_FOUND.0) => {
            return Err(SysError::NotFound(format!("dziennik {channel}")));
        }
        Err(e) => return Err(sys_error(&format!("dziennik {channel}"), &e)),
    };
    let mut cache: HashMap<String, Option<Evt>> = HashMap::new();
    let mut out = Vec::new();
    while out.len() < max as usize {
        let mut handles = [0isize; 16];
        let mut returned = 0u32;
        // SAFETY: tablica wyjściowa uchwytów i licznik.
        match unsafe { EvtNext(q.0, &mut handles, NEXT_TIMEOUT_MS, 0, &raw mut returned) } {
            Ok(()) => {}
            Err(e) if e.code() == HRESULT::from_win32(ERROR_NO_MORE_ITEMS.0) => break,
            Err(e) => return Err(sys_error("odczyt dziennika", &e)),
        }
        if returned == 0 {
            break;
        }
        let events: Vec<Evt> = handles[..(returned as usize).min(handles.len())]
            .iter()
            .map(|h| Evt(EVT_HANDLE(*h)))
            .collect();
        for ev in &events {
            if out.len() >= max as usize {
                break;
            }
            let Some(xml) = render_xml(ev) else {
                continue;
            };
            let provider = record_from_xml(&xml, None).provider;
            let msg = publisher(&provider, &mut cache).and_then(|p| message(p, ev));
            out.push(record_from_xml(&xml, msg));
        }
    }
    Ok(out)
}
