//! Współdzielone testy kontraktowe logów (feature `contract-tests`).
//! Ten sam zestaw uruchamiają `core-log-impl` i `core-log-fake`; rozjazd = błąd.

use std::fmt::Display;

use chrono::{DateTime, Duration, Utc};
use core_bus_contract::{Event, EventKind, Level, SessionId};
use serde_json::json;

use crate::{AuditWriter, LogQuery, LogSink, LogStream, REDACTED, RecordRef};

fn ok<T, E: Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// Stały punkt odniesienia czasu testów: 2026-01-01T00:00:00Z.
pub fn t0() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(1_767_225_600, 0).unwrap_or_default()
}

/// Zdarzenie z czasem `t0 + secs`, opcjonalną sesją i ładunkiem.
pub fn event_at(
    kind: EventKind,
    secs: i64,
    session: Option<&str>,
    payload: serde_json::Value,
) -> Event {
    let mut ev = Event::new(kind, Level::Info, payload);
    ev.ts = t0() + Duration::seconds(secs);
    ev.session = session.map(SessionId::from);
    ev
}

fn payloads(records: &[crate::LogRecord]) -> Vec<serde_json::Value> {
    records.iter().map(|r| r.event.payload.clone()).collect()
}

/// `append` nadaje `seq` od 0, niezależnie w każdym strumieniu; `query` oddaje dane bez zmian.
pub async fn append_assigns_sequential_refs<S: LogSink>(sink: S) {
    for i in 0..3u64 {
        let ev = event_at(EventKind::Voice, i as i64, None, json!({"i": i}));
        let r = ok(sink.append(LogStream::Voice, &ev).await);
        assert_eq!(
            r,
            RecordRef {
                stream: LogStream::Voice,
                seq: i
            }
        );
    }
    let ev = event_at(EventKind::Tool, 0, None, json!("gui"));
    let r = ok(sink.append(LogStream::ToolsGui, &ev).await);
    assert_eq!(r.seq, 0, "każdy strumień ma własną numerację");
    let voice = LogQuery {
        stream: Some(LogStream::Voice),
        ..LogQuery::default()
    };
    let got = ok(sink.query(voice).await);
    assert_eq!(got.len(), 3);
    assert_eq!(
        got[2].reference,
        RecordRef {
            stream: LogStream::Voice,
            seq: 2
        }
    );
    assert_eq!(got[1].event.kind, EventKind::Voice);
    assert_eq!(got[1].event.ts, t0() + Duration::seconds(1));
    assert_eq!(got[0].event.payload, json!({"i": 0}));
    let empty = LogQuery {
        stream: Some(LogStream::Diagnostics),
        ..LogQuery::default()
    };
    assert!(ok(sink.query(empty).await).is_empty());
}

/// Filtry: sesja, rodzaj, `from_seq`, zakres czasu, `limit`; scalanie strumieni po czasie.
pub async fn query_filters<S: LogSink>(sink: S) {
    let model = |secs, session, n: u64| event_at(EventKind::ModelCall, secs, session, json!(n));
    let diag = event_at(EventKind::Diagnostics, 1, Some("s1"), json!(9));
    for ev in [
        model(0, Some("s1"), 0),
        model(1, Some("s2"), 1),
        model(2, Some("s1"), 2),
    ] {
        ok(sink.append(LogStream::ModelCalls, &ev).await);
    }
    ok(sink.append(LogStream::Diagnostics, &diag).await);
    let base = LogQuery {
        stream: Some(LogStream::ModelCalls),
        ..LogQuery::default()
    };
    let q = |f: fn(LogQuery) -> LogQuery| f(base.clone());
    let s1 = q(|b| LogQuery {
        session: Some(SessionId::from("s1")),
        ..b
    });
    assert_eq!(payloads(&ok(sink.query(s1).await)), [json!(0), json!(2)]);
    let from = q(|b| LogQuery {
        from_seq: Some(1),
        ..b
    });
    assert_eq!(payloads(&ok(sink.query(from).await)), [json!(1), json!(2)]);
    let limit = q(|b| LogQuery {
        limit: Some(2),
        ..b
    });
    assert_eq!(payloads(&ok(sink.query(limit).await)), [json!(0), json!(1)]);
    let window = LogQuery {
        since: Some(t0() + Duration::seconds(1)),
        until: Some(t0() + Duration::seconds(2)),
        ..base.clone()
    };
    assert_eq!(payloads(&ok(sink.query(window).await)), [json!(1)]);
    let all = ok(sink.query(LogQuery::default()).await);
    assert_eq!(payloads(&all), [json!(0), json!(1), json!(9), json!(2)]);
    let by_kind = LogQuery {
        kind: Some(EventKind::Diagnostics),
        ..LogQuery::default()
    };
    let diag_only = ok(sink.query(by_kind).await);
    assert_eq!(diag_only.len(), 1);
    assert_eq!(diag_only[0].reference.stream, LogStream::Diagnostics);
}

/// Sekrety w ładunku są redagowane przed zapisem (także w zagnieżdżonych wartościach).
pub async fn secrets_are_redacted<S: LogSink>(sink: S) {
    let payload = json!({
        "headers": {"authorization": "Bearer abcdefghijklmnopqrst"},
        "notes": ["klucz sk-ant-api03-abcdefghijklmnop", "zwykły tekst"],
    });
    let ev = event_at(EventKind::ModelCall, 0, None, payload);
    ok(sink.append(LogStream::ModelCalls, &ev).await);
    let got = ok(sink.query(LogQuery::default()).await);
    let text = got[0].event.payload.to_string();
    assert!(!text.contains("abcdefghijklmnop"), "sekret w logu: {text}");
    assert!(text.contains(REDACTED));
    assert!(text.contains("zwykły tekst"));
}

/// Łańcuch audytu: pusta głowa, `seq` od 0, każdy rekord ma nowy hash = głowa.
pub async fn audit_chain_links<A: AuditWriter>(audit: A) {
    assert_eq!(ok(audit.head_hash().await), None);
    let first = ok(audit
        .append_audit(&event_at(EventKind::Audit, 0, None, json!(1)))
        .await);
    assert_eq!(first.seq, 0);
    assert!(!first.hash.is_empty());
    assert_eq!(ok(audit.head_hash().await), Some(first.hash.clone()));
    let second = ok(audit
        .append_audit(&event_at(EventKind::Audit, 0, None, json!(1)))
        .await);
    assert_eq!(second.seq, 1);
    assert_ne!(
        second.hash, first.hash,
        "ten sam ładunek, inne miejsce w łańcuchu"
    );
    assert_eq!(ok(audit.head_hash().await), Some(second.hash));
}

/// Uruchamia zestaw `LogSink`; `factory` daje świeży, pusty sink dla każdego przypadku.
pub async fn run_all<S: LogSink, F: Fn() -> S>(factory: F) {
    append_assigns_sequential_refs(factory()).await;
    query_filters(factory()).await;
    secrets_are_redacted(factory()).await;
}

/// Uruchamia zestaw `AuditWriter`; `factory` daje świeży, pusty łańcuch.
pub async fn run_audit<A: AuditWriter, F: Fn() -> A>(factory: F) {
    audit_chain_links(factory()).await;
}
