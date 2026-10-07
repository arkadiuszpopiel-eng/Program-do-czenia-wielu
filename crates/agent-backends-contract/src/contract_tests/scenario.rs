//! Scenariusze atrap wybierane znacznikiem w poleceniu (`[alfa-fake:<nazwa>:<argumenty>]`).
//! Rozumie je `FakeAgentBackend` oraz fałszywe CLI (`alfa-fake-agent-cli`), więc te same testy
//! kontraktowe działają na atrapie i na prawdziwych mostach z fałszywym procesem.

/// Prefiks znacznika.
pub const TAG_PREFIX: &str = "[alfa-fake:";

/// Scenariusz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scenario {
    /// Pełny przebieg: plan, narzędzie, zmiana pliku, wynik.
    Ok,
    /// `n` próśb o uprawnienie, każda czeka na decyzję; wynik wymienia decyzje (`allow`/`deny`).
    Permission(u32),
    /// `n` wyjść co `interval_ms`; tekst `t=<unix_ms>` (pomiar opóźnienia postępu).
    Slow {
        /// Liczba wyjść.
        n: u32,
        /// Odstęp.
        interval_ms: u64,
    },
    /// Po starcie wisi bez końca (z procesem-wnukiem) — test anulowania drzewa procesów.
    Hang,
    /// Kilka zdarzeń, potem wyjście z kodem 3 bez wyniku.
    Crash,
    /// Wynik z `is_error = true` i kodem wyjścia 1.
    ErrorResult,
    /// Śmieci i nieznane typy wiadomości przeplatane z poprawnymi.
    Garbage,
    /// Linia o podanej długości w bajtach (powyżej limitu → pominięta z ostrzeżeniem).
    LongLine(usize),
    /// Wypisuje nazwy zmiennych środowiska procesu CLI.
    Env,
    /// Czeka na wiadomość sterującą i odpowiada `steer:<treść>`.
    Steer,
}

impl Scenario {
    /// Polecenie ze znacznikiem.
    pub fn prompt(&self) -> String {
        let tag = match self {
            Scenario::Ok => "ok".to_owned(),
            Scenario::Permission(n) => format!("permission:{n}"),
            Scenario::Slow { n, interval_ms } => format!("slow:{n}:{interval_ms}"),
            Scenario::Hang => "hang".to_owned(),
            Scenario::Crash => "crash".to_owned(),
            Scenario::ErrorResult => "error-result".to_owned(),
            Scenario::Garbage => "garbage".to_owned(),
            Scenario::LongLine(n) => format!("long-line:{n}"),
            Scenario::Env => "env".to_owned(),
            Scenario::Steer => "steer".to_owned(),
        };
        format!("{TAG_PREFIX}{tag}] zadanie testowe")
    }

    /// Odczytuje scenariusz z polecenia; brak/nieznany znacznik = `Ok`.
    pub fn parse(prompt: &str) -> Scenario {
        let Some(start) = prompt.find(TAG_PREFIX) else {
            return Scenario::Ok;
        };
        let rest = &prompt[start + TAG_PREFIX.len()..];
        let Some(end) = rest.find(']') else {
            return Scenario::Ok;
        };
        let parts: Vec<&str> = rest[..end].split(':').collect();
        let num = |i: usize| parts.get(i).and_then(|p| p.parse::<u64>().ok());
        match parts.first().copied() {
            Some("permission") => {
                Scenario::Permission(num(1).and_then(|n| u32::try_from(n).ok()).unwrap_or(1))
            }
            Some("slow") => Scenario::Slow {
                n: num(1).and_then(|n| u32::try_from(n).ok()).unwrap_or(3),
                interval_ms: num(2).unwrap_or(200),
            },
            Some("hang") => Scenario::Hang,
            Some("crash") => Scenario::Crash,
            Some("error-result") => Scenario::ErrorResult,
            Some("garbage") => Scenario::Garbage,
            Some("long-line") => {
                Scenario::LongLine(num(1).and_then(|n| usize::try_from(n).ok()).unwrap_or(1024))
            }
            Some("env") => Scenario::Env,
            Some("steer") => Scenario::Steer,
            _ => Scenario::Ok,
        }
    }
}

/// Tekst wyniku scenariusza `Permission`: decyzje po kolei, np. `decisions=allow,deny`.
pub fn permission_result(decisions: &[bool]) -> String {
    let list: Vec<&str> = decisions
        .iter()
        .map(|d| if *d { "allow" } else { "deny" })
        .collect();
    format!("decisions={}", list.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for s in [
            Scenario::Ok,
            Scenario::Permission(20),
            Scenario::Slow {
                n: 5,
                interval_ms: 300,
            },
            Scenario::Hang,
            Scenario::Crash,
            Scenario::ErrorResult,
            Scenario::Garbage,
            Scenario::LongLine(9_000_000),
            Scenario::Env,
            Scenario::Steer,
        ] {
            assert_eq!(Scenario::parse(&s.prompt()), s);
        }
        assert_eq!(Scenario::parse("zwykłe polecenie"), Scenario::Ok);
        assert_eq!(Scenario::parse("[alfa-fake:nieznany]"), Scenario::Ok);
        assert_eq!(permission_result(&[true, false]), "decisions=allow,deny");
    }
}
