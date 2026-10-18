//! Wrogie i nietypowe struktury: `/Count` drzewa stron, strumienie obiektów (także w pliku
//! zaszyfrowanym), pośrednie `/Length`, nakładające się strumienie.

use std::time::Instant;

use lopdf::{
    Dictionary, Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream,
};

use super::tests_build::{POLISH_FONT, Pdf, with_fonts};
use super::*;

fn run(bytes: &[u8]) -> Extracted {
    let limits = Limits::with_chars(100_000);
    extract(bytes, &limits, Instant::now() + limits.time)
}

fn note_of(out: Extracted) -> String {
    match out {
        Extracted::Note(note) => note,
        Extracted::Text { text, .. } => panic!("oczekiwano notki, jest tekst: {text}"),
    }
}

fn text_of(out: Extracted) -> String {
    match out {
        Extracted::Text { text, .. } => text,
        Extracted::Note(note) => panic!("oczekiwano tekstu, jest notka: {note}"),
    }
}

const HELLO: &[u8] = b"BT /F1 12 Tf 72 720 Td (Za\\204\\363\\202\\206) Tj ET";

/// Drzewo stron: strona i pusty węzeł `/Pages` z `/Count` = `count`.
fn page_tree_with_count(count: &str) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let root = pdf.reserve();
    let content = pdf.stream("", b"");
    let page = pdf.add(format!(
        "<< /Type /Page /Parent {root} 0 R /MediaBox [0 0 595 842] /Contents {content} 0 R >>"
    ));
    let empty = pdf.add(format!(
        "<< /Type /Pages /Parent {root} 0 R /Kids [] /Count {count} >>"
    ));
    pdf.set(
        root,
        format!("<< /Type /Pages /Kids [{page} 0 R {empty} 0 R] /Count 2 /Resources << >> >>"),
    );
    let catalog = pdf.add(format!("<< /Type /Catalog /Pages {root} 0 R >>"));
    pdf.finish(catalog)
}

#[test]
fn hostile_page_count_does_not_reserve_memory() {
    for count in ["100000000000", "9223372036854775807"] {
        let note = note_of(run(&page_tree_with_count(count)));
        assert!(note.contains("nie zawiera tekstu"), "{count}: {note}");
    }
}

/// Skompresowany strumień obiektów (`/Type` podany jako `kind`) z indeksem `index` i treścią `body`.
fn object_stream(kind: &str, index: &str, body: &[u8]) -> Stream {
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(kind.as_bytes().to_vec()));
    dict.set(
        "N",
        i64::try_from(index.split_whitespace().count() / 2).unwrap(),
    );
    dict.set("First", i64::try_from(index.len()).unwrap());
    let mut stream = Stream::new(dict, [index.as_bytes(), body].concat());
    stream.compress().unwrap();
    stream
}

/// Surowy obiekt strumienia (słownik + treść) do budowniczego `Pdf`.
fn raw_stream(stream: &Stream) -> Vec<u8> {
    let mut dict = String::new();
    for (key, value) in &stream.dict {
        let value = match value {
            Object::Name(n) => format!("/{}", String::from_utf8_lossy(n)),
            Object::Integer(i) => i.to_string(),
            other => panic!("nieobsługiwana wartość: {other:?}"),
        };
        dict.push_str(&format!("/{} {value} ", String::from_utf8_lossy(key)));
    }
    [
        format!("<< {dict}>>\nstream\n").as_bytes(),
        &stream.content,
        b"\nendstream",
    ]
    .concat()
}

/// Dokument, w którym czcionka `/F1` siedzi w strumieniu obiektów, a treść strony ma pośrednie
/// `/Length` wskazujące na liczbę w tym samym strumieniu.
fn font_in_object_stream() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let font = pdf.reserve();
    let length = pdf.reserve();
    let index = format!("{font} 0 {length} {} ", POLISH_FONT.len() + 1);
    let body = format!("{POLISH_FONT} {}", HELLO.len());
    pdf.add(raw_stream(&object_stream(
        "ObjStm",
        &index,
        body.as_bytes(),
    )));
    let content = pdf.add(
        [
            format!("<< /Length {length} 0 R >>\nstream\n").as_bytes(),
            HELLO,
            b"\nendstream",
        ]
        .concat(),
    );
    let root = pdf.reserve();
    let page = pdf.add(format!(
        "<< /Type /Page /Parent {root} 0 R /MediaBox [0 0 595 842] /Contents {content} 0 R >>"
    ));
    pdf.set(
        root,
        format!("<< /Type /Pages /Kids [{page} 0 R] /Count 1 /Resources << /Font << /F1 {font} 0 R >> >> >>"),
    );
    let catalog = pdf.add(format!("<< /Type /Catalog /Pages {root} 0 R >>"));
    let mut bytes = pdf.finish(catalog);
    // Zarezerwowane numery bez obiektu najwyższego poziomu: psujemy ich nagłówek (`obj`), żeby
    // jedyną definicją był obiekt ze strumienia obiektów.
    for id in [font, length] {
        let needle = format!("\n{id} 0 obj\nnull");
        let at = bytes
            .windows(needle.len())
            .position(|w| w == needle.as_bytes())
            .unwrap();
        let obj = at + needle.len() - 8;
        bytes[obj..obj + 3].copy_from_slice(b"xxx");
    }
    bytes
}

#[test]
fn object_streams_and_indirect_lengths_are_read() {
    let text = text_of(run(&font_in_object_stream()));
    assert!(text.contains("Zażółć"), "{text}");
}

/// Indeks z `copies` obiektami pod tym samym przesunięciem dużego napisu.
fn hostile_index(copies: u32) -> (String, Vec<u8>) {
    let index: String = (0..copies).map(|i| format!("{} 0 ", 1000 + i)).collect();
    let body = [b"(".as_slice(), &b"a".repeat(200_000), b")"].concat();
    (index, body)
}

#[test]
fn repeated_object_stream_offsets_hit_the_budget() {
    let (index, body) = hostile_index(2000);
    let mut pdf = Pdf::new();
    pdf.add(raw_stream(&object_stream("ObjStm", &index, &body)));
    let root = pdf.pages(&[HELLO], "<< >>");
    let note = note_of(run(&pdf.finish(root)));
    assert!(note.contains("zbyt złożony"), "{note}");
}

/// PDF zaszyfrowany RC4 pustym hasłem użytkownika z dodatkowym strumieniem obiektów.
fn encrypted_with_object_stream(index: &str, body: &[u8], drop_font: bool) -> Vec<u8> {
    let mut doc = Document::load_mem(&with_fonts(&[HELLO])).unwrap();
    let font = doc
        .objects
        .iter()
        .find(|(_, o)| {
            o.as_dict().is_ok_and(|d| {
                d.get(b"BaseFont").ok() == Some(&Object::Name(b"Helvetica".to_vec()))
            })
        })
        .map(|(id, _)| *id)
        .unwrap();
    let index = index.replace("FONT", &font.0.to_string());
    if drop_font {
        doc.objects.remove(&font);
    }
    // `ObjStX`: zapis `lopdf` pomija obiekty `ObjStm`; nazwę podmieniamy w bajtach po zapisie.
    doc.add_object(object_stream("ObjStX", &index, body));
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &doc,
        owner_password: "wlasc",
        user_password: "",
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    doc.encrypt(&state).unwrap();
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    let at = out.windows(7).position(|w| w == b"/ObjStX").unwrap();
    out[at..at + 7].copy_from_slice(b"/ObjStm");
    out
}

#[test]
fn encrypted_object_streams_are_decrypted_and_bounded() {
    let font_index = "FONT 0 ";
    let open = encrypted_with_object_stream(font_index, POLISH_FONT.as_bytes(), true);
    let text = text_of(run(&open));
    assert!(text.contains("Zażółć"), "{text}");

    let (index, body) = hostile_index(2000);
    let hostile = encrypted_with_object_stream(&index, &body, false);
    let note = note_of(run(&hostile));
    assert!(note.contains("zbyt złożony"), "{note}");
}

#[test]
fn overlapping_stream_lengths_are_refused() {
    let mut bytes = with_fonts(&[HELLO]);
    let at = bytes.len() - 6;
    bytes.splice(at..at, b"% << /Length 99999999 >>\n".iter().copied());
    let note = note_of(run(&bytes));
    assert!(note.contains("zbyt złożony"), "{note}");
}

#[test]
fn headers_and_placeholders_count_against_the_char_limit() {
    let mut contents: Vec<&[u8]> = vec![HELLO];
    contents.extend(std::iter::repeat_n(b"".as_slice(), 50));
    let pdf = with_fonts(&contents);
    for max_chars in [250, 300, 1000] {
        let limits = Limits {
            max_chars,
            ..Limits::with_chars(max_chars)
        };
        match extract(&pdf, &limits, Instant::now() + limits.time) {
            Extracted::Text { text, truncated } => {
                assert!(truncated, "{text}");
                assert!(text.chars().count() <= max_chars, "{max_chars}: {text}");
                assert!(text.contains("tekst niepełny"), "{text}");
            }
            Extracted::Note(note) => assert!(note.contains("limit"), "{note}"),
        }
    }
}

#[test]
fn modern_files_with_xref_and_object_streams_are_read() {
    let mut doc = Document::load_mem(&with_fonts(&[HELLO, HELLO])).unwrap();
    let mut modern = Vec::new();
    doc.save_modern(&mut modern).unwrap();
    assert!(
        modern.windows(7).any(|w| w == b"/ObjStm"),
        "plik ze strumieniem obiektów"
    );
    let text = text_of(run(&modern));
    assert!(text.contains("--- Strona 2 ---\nZażółć"), "{text}");
}
