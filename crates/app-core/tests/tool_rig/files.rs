//! Pliki katalogu roboczego diagnostyki (w atrapie FS i na dysku tymczasowym — multimedia
//! czytają z dysku). Narzędzia zmieniające pliki dostają własne pliki, żeby kolejność wywołań
//! nie zmieniała wyników innych narzędzi.

pub const FILES: &[(&str, &[u8])] = &[
    ("notatki.txt", b"Ala ma kota.\nDruga linia notatki.\n"),
    ("dane.csv", b"nazwa;ilosc\njablka;3\ngruszki;5\n"),
    ("raport.docx", b"PK\x03\x04atrapa dokumentu"),
    ("tabela.xlsx", b"PK\x03\x04atrapa arkusza"),
    ("podkatalog/plik.txt", b"plik w podkatalogu\n"),
    ("obraz.png", PIXEL_PNG),
    ("dzwiek.wav", WAV),
    (
        "Cargo.toml",
        b"[package]\nname = \"diag\"\nversion = \"0.1.0\"\n",
    ),
    ("do-kopii.txt", b"kopia\n"),
    ("do-usuniecia.txt", b"do Kosza\n"),
    ("do-usuniecia-trwale.txt", b"trwale\n"),
    ("do-przeniesienia.txt", b"przenies\n"),
    ("do-zmiany-nazwy.txt", b"nazwa\n"),
];

/// Dokumenty Word/Excel w formacie atrapy Office (`FakeDocument`).
pub fn documents() -> Vec<(&'static str, Vec<u8>)> {
    use platform_apps_fake::FakeDocument;
    vec![
        (
            "raport.docx",
            FakeDocument::word(&["Raport diagnostyki", "Druga linia."]).to_bytes(),
        ),
        (
            "tabela.xlsx",
            FakeDocument::excel("Arkusz1")
                .with_table(&[&["nazwa", "ilosc"], &["jablka", "3"]])
                .to_bytes(),
        ),
    ]
}

/// PNG 1×1 (biały piksel).
pub const PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xDC, 0xCC, 0x59, 0xE7, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// WAV PCM 16 bit, mono, 16 kHz, 4 próbki ciszy.
pub const WAV: &[u8] = &[
    b'R', b'I', b'F', b'F', 44, 0, 0, 0, b'W', b'A', b'V', b'E', b'f', b'm', b't', b' ', 16, 0, 0,
    0, 1, 0, 1, 0, 0x80, 0x3E, 0, 0, 0, 0x7D, 0, 0, 2, 0, 16, 0, b'd', b'a', b't', b'a', 8, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
];
