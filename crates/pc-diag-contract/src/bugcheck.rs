//! Kody niebieskiego ekranu (BugCheck) z dziennika: nazwa i prawdopodobna przyczyna po polsku,
//! żeby raport mówił „sterownik karty graficznej”, a nie „0x00000116”.

/// Najczęstsze kody: (kod, nazwa, przyczyna).
const CODES: &[(u32, &str, &str)] = &[
    (0x0A, "IRQL_NOT_LESS_OR_EQUAL", "sterownik albo pamięć RAM"),
    (0x19, "BAD_POOL_HEADER", "sterownik uszkadza pamięć jądra"),
    (
        0x1A,
        "MEMORY_MANAGEMENT",
        "pamięć RAM albo sterownik — sprawdź test pamięci",
    ),
    (0x1E, "KMODE_EXCEPTION_NOT_HANDLED", "sterownik"),
    (
        0x3B,
        "SYSTEM_SERVICE_EXCEPTION",
        "sterownik albo uszkodzone pliki systemu",
    ),
    (
        0x50,
        "PAGE_FAULT_IN_NONPAGED_AREA",
        "pamięć RAM, dysk albo sterownik",
    ),
    (
        0x7A,
        "KERNEL_DATA_INPAGE_ERROR",
        "odczyt pliku stronicowania z dysku — dysk, kabel albo mostek USB",
    ),
    (0x7E, "SYSTEM_THREAD_EXCEPTION_NOT_HANDLED", "sterownik"),
    (
        0x9F,
        "DRIVER_POWER_STATE_FAILURE",
        "sterownik przy uśpieniu albo wybudzeniu",
    ),
    (0xC2, "BAD_POOL_CALLER", "sterownik"),
    (
        0xD1,
        "DRIVER_IRQL_NOT_LESS_OR_EQUAL",
        "sterownik (często sieciowy)",
    ),
    (
        0xEF,
        "CRITICAL_PROCESS_DIED",
        "proces systemu — pliki systemu albo dysk",
    ),
    (
        0xF4,
        "CRITICAL_OBJECT_TERMINATION",
        "dysk systemowy albo jego połączenie",
    ),
    (
        0x101,
        "CLOCK_WATCHDOG_TIMEOUT",
        "procesor — przegrzanie, napięcia albo podkręcanie",
    ),
    (
        0x116,
        "VIDEO_TDR_FAILURE",
        "karta graficzna przestała odpowiadać — sterownik, przegrzanie albo zasilanie karty",
    ),
    (
        0x117,
        "VIDEO_TDR_TIMEOUT_DETECTED",
        "karta graficzna przestała odpowiadać — sterownik albo przegrzanie",
    ),
    (
        0x124,
        "WHEA_UNCORRECTABLE_ERROR",
        "błąd sprzętu — procesor, pamięć, przegrzanie albo zasilanie",
    ),
    (
        0x133,
        "DPC_WATCHDOG_VIOLATION",
        "sterownik (często dysku albo karty graficznej)",
    ),
    (
        0x139,
        "KERNEL_SECURITY_CHECK_FAILURE",
        "sterownik albo uszkodzona pamięć",
    ),
    (
        0x154,
        "UNEXPECTED_STORE_EXCEPTION",
        "dysk albo jego sterownik",
    ),
    (
        0x1E0,
        "CRITICAL_PROCESS_DIED",
        "proces systemu — pliki systemu albo dysk",
    ),
];

/// Pierwszy kod `0x…` z treści zdarzenia (BugCheck 1001).
pub fn code_in(message: &str) -> Option<u32> {
    let lower = message.to_ascii_lowercase();
    let start = lower.find("0x")?;
    let hex: String = lower[start + 2..]
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect();
    u32::from_str_radix(&hex, 16).ok()
}

/// Opis kodu: „0x116 VIDEO_TDR_FAILURE — karta graficzna…”.
pub fn describe(code: u32) -> String {
    match CODES.iter().find(|(c, _, _)| *c == code) {
        Some((_, name, why)) => format!("0x{code:X} {name} — {why}"),
        None => format!("0x{code:X} — kod spoza katalogu (wyszukaj nazwę kodu)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_codes_from_messages() {
        let m = "Nastąpił ponowny rozruch komputera… 0x00000116 (0xffffe68f5b3e1010, 0xfff…)";
        assert_eq!(code_in(m), Some(0x116));
        assert!(describe(0x116).starts_with("0x116 VIDEO_TDR_FAILURE — karta graficzna"));
        assert!(describe(0xDEAD).contains("spoza katalogu"));
        assert_eq!(code_in("bez kodu"), None);
    }
}
