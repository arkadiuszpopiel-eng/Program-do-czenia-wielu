//! Zegar wydajnościowy systemu (QPC) w nanosekundach. To ta sama podstawa czasu, w której WASAPI
//! znaczy pakiety mikrofonu i pozycję odtwarzania — potok głosowy musi liczyć „teraz” tym samym
//! zegarem, inaczej cisza po wypowiedzi wychodzi zerowa i tura użytkownika nigdy się nie kończy
//! (błąd z laptopa, 2026-10-09: mikrofon słuchał, odpowiedzi nie było).

#![allow(unsafe_code)]

/// Bieżąca chwila zegara QPC w nanosekundach od startu systemu.
#[cfg(windows)]
pub fn qpc_nanos() -> u64 {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

    let (mut ticks, mut frequency) = (0i64, 0i64);
    // SAFETY: obie funkcje zapisują wyłącznie do przekazanych zmiennych lokalnych.
    let ok = unsafe {
        QueryPerformanceFrequency(&mut frequency).is_ok()
            && QueryPerformanceCounter(&mut ticks).is_ok()
    };
    if !ok || frequency <= 0 || ticks < 0 {
        return 0;
    }
    let (ticks, frequency) = (ticks as u128, frequency as u128);
    u64::try_from(ticks * 1_000_000_000 / frequency).unwrap_or(u64::MAX)
}

/// Poza Windows nie ma WASAPI — zegar od pierwszego wywołania (spójny sam ze sobą).
#[cfg(not(windows))]
pub fn qpc_nanos() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;

    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    u64::try_from(ORIGIN.get_or_init(Instant::now).elapsed().as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::qpc_nanos;

    #[test]
    fn clock_runs_forward_at_wall_speed() {
        let a = qpc_nanos();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let elapsed = qpc_nanos() - a;
        assert!((40_000_000..500_000_000).contains(&elapsed), "{elapsed} ns");
    }

    /// Ta sama podstawa co `Instant` (na Windows oba to QPC): różnica dwóch zegarów jest stała.
    #[test]
    fn matches_the_monotonic_clock_rate() {
        let (i0, q0) = (std::time::Instant::now(), qpc_nanos());
        std::thread::sleep(std::time::Duration::from_millis(100));
        let (di, dq) = (i0.elapsed().as_nanos() as i128, (qpc_nanos() - q0) as i128);
        assert!((di - dq).abs() < 5_000_000, "Instant {di} ns, QPC {dq} ns");
    }
}
