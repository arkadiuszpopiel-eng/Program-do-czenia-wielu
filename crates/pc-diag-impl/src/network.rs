//! Test sieci: rozwiązywanie nazw (DNS) i czas nawiązania połączenia TCP z kilkoma serwerami
//! (port 443) — bez wysyłania danych. Wynik: średnie czasy, liczba nieudanych prób.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use pc_diag_contract::{CheckKind, CheckResult, Metric, Verdict};

/// Serwery testu (duzi operatorzy, różne sieci).
pub const HOSTS: [&str; 3] = ["www.microsoft.com", "one.one.one.one", "www.google.com"];
/// Limit połączenia.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Wynik jednej próby (czas DNS, czas połączenia).
pub type Probe = Result<(f64, f64), String>;

/// Prawdziwa próba sieci.
pub fn probe_host(host: &str) -> Probe {
    let t = Instant::now();
    let addrs: Vec<SocketAddr> = (host, 443)
        .to_socket_addrs()
        .map_err(|e| format!("{host}: nazwa nie rozwiązana ({e})"))?
        .collect();
    let dns = t.elapsed().as_secs_f64() * 1000.0;
    let addr = addrs
        .first()
        .ok_or_else(|| format!("{host}: brak adresu"))?;
    let t = Instant::now();
    TcpStream::connect_timeout(addr, CONNECT_TIMEOUT)
        .map_err(|e| format!("{host}: brak połączenia ({e})"))?;
    Ok((dns, t.elapsed().as_secs_f64() * 1000.0))
}

/// Ocena wyników prób (osobno od sieci — testowalna).
pub fn rate(results: &[(String, Probe)], duration_ms: u64) -> CheckResult {
    let ok: Vec<(f64, f64)> = results
        .iter()
        .filter_map(|(_, r)| r.as_ref().ok().copied())
        .collect();
    let failed: Vec<&String> = results
        .iter()
        .filter_map(|(_, r)| r.as_ref().err())
        .collect();
    let avg = |f: fn(&(f64, f64)) -> f64| ok.iter().map(f).sum::<f64>() / ok.len().max(1) as f64;
    let mut r = CheckResult {
        kind: CheckKind::Network,
        verdict: Verdict::Pass,
        metrics: vec![Metric::new(
            "failed",
            "Nieudane próby",
            failed.len() as f64,
            "szt.",
            false,
        )],
        notes: failed.iter().map(|f| (*f).clone()).collect(),
        duration_ms,
    };
    if ok.is_empty() {
        r.verdict = Verdict::Fail;
        r.notes
            .push("Brak połączenia z internetem (albo blokuje je zapora).".into());
        return r;
    }
    let (dns, tcp) = (avg(|p| p.0), avg(|p| p.1));
    r.metrics.push(Metric::new(
        "dns_ms",
        "Rozwiązywanie nazw",
        dns,
        "ms",
        false,
    ));
    r.metrics.push(Metric::new(
        "connect_ms",
        "Nawiązanie połączenia",
        tcp,
        "ms",
        false,
    ));
    if !failed.is_empty() {
        r.verdict = Verdict::Warn;
    }
    if tcp > 150.0 || dns > 300.0 {
        r.verdict = Verdict::Warn;
        r.notes.push(format!(
            "Wolna odpowiedź sieci (DNS {dns:.0} ms, połączenie {tcp:.0} ms)."
        ));
    }
    if r.notes.is_empty() {
        r.notes.push("Połączenie z internetem działa.".into());
    }
    r
}

/// Test sieci na serwerach [`HOSTS`].
pub fn network() -> CheckResult {
    let start = Instant::now();
    let results: Vec<(String, Probe)> = std::thread::scope(|s| {
        let hs: Vec<_> = HOSTS
            .iter()
            .map(|h| s.spawn(move || ((*h).to_owned(), probe_host(h))))
            .collect();
        hs.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    rate(&results, start.elapsed().as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rating_offline_partial_and_slow() {
        let off = rate(&[("a".into(), Err("a: brak".into()))], 5);
        assert_eq!(off.verdict, Verdict::Fail);
        let partial = rate(
            &[
                ("a".into(), Ok((10.0, 20.0))),
                ("b".into(), Err("b: brak".into())),
            ],
            5,
        );
        assert_eq!(partial.verdict, Verdict::Warn);
        assert_eq!(partial.metric("connect_ms"), Some(20.0));
        let slow = rate(&[("a".into(), Ok((10.0, 400.0)))], 5);
        assert_eq!(slow.verdict, Verdict::Warn);
        let good = rate(&[("a".into(), Ok((5.0, 15.0)))], 5);
        assert_eq!(good.verdict, Verdict::Pass);
    }
}
