//! Nazwy urządzeń Windows w ścieżkach narzędzi. `C:\praca\con`, `nul.txt` czy `COM1` nie są
//! plikami: Win32 mapuje je na konsolę, urządzenie puste albo port — odczyt może zawisnąć,
//! a zapis „udaje się", choć nic nie powstaje. Narzędzia agentek odrzucają takie segmenty.

/// Nazwy zastrzeżone (bez numerowanych `COMn` / `LPTn`).
const DEVICES: [&str; 6] = ["con", "prn", "aux", "nul", "conin$", "conout$"];

/// Czy segment ścieżki jest nazwą urządzenia (także z rozszerzeniem: `nul.txt`, `COM1.log`).
pub(crate) fn is_device_segment(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_lowercase();
    if DEVICES.contains(&stem.as_str()) {
        return true;
    }
    // COM0–COM9, LPT0–LPT9 oraz warianty z cyframi w indeksie górnym (¹ ² ³).
    ["com", "lpt"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|rest| {
            let mut chars = rest.chars();
            matches!(
                (chars.next(), chars.next()),
                (Some('0'..='9' | '¹' | '²' | '³'), None)
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::is_device_segment;

    #[test]
    fn device_names_are_recognised_with_any_case_and_extension() {
        for name in [
            "con", "CON", "nul", "NUL.txt", "aux.log", "prn", "COM1", "com9.dat", "LPT0", "lpt¹",
            "CONIN$", "conout$", "nul ",
        ] {
            assert!(is_device_segment(name), "{name}");
        }
    }

    #[test]
    fn ordinary_names_are_not_devices() {
        for name in [
            "",
            "konto",
            "console",
            "nullable.rs",
            "com",
            "com10",
            "comx",
            "lpt",
            "auxiliary",
            "raport.con",
            "C:",
            "notatki.txt",
            "printer",
            "COM1a",
        ] {
            assert!(!is_device_segment(name), "{name}");
        }
    }
}
