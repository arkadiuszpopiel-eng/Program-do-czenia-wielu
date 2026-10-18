use std::time::{Duration, Instant};

use lopdf::{Document, EncryptionState, EncryptionVersion, Permissions};

use super::cmap::CMap;
use super::lexer::{Lexer, Tok};
use super::tests_build::{Pdf, with_fonts};
use super::*;

fn limits() -> Limits {
    Limits::with_chars(100_000)
}

fn run(bytes: &[u8], limits: &Limits) -> Extracted {
    extract(bytes, limits, Instant::now() + limits.time)
}

fn text_of(out: Extracted) -> String {
    match out {
        Extracted::Text { text, .. } => text,
        Extracted::Note(note) => panic!("oczekiwano tekstu, jest notka: {note}"),
    }
}

fn note_of(out: Extracted) -> String {
    match out {
        Extracted::Note(note) => note,
        Extracted::Text { text, .. } => panic!("oczekiwano notki, jest tekst: {text}"),
    }
}

const PAGE_1: &[u8] =
    b"BT /F1 12 Tf 72 720 Td (Za\\204\\363\\202\\206 g\\201\\203l\\200 ja\\205\\207) Tj \
    0 -14 Td (\\210 \\211x) Tj ET";
const PAGE_2: &[u8] = b"BT /F2 10 Tf 72 720 Td [<00040005> -20 <00060007> -400 <000100020003>] TJ ET \
    BT /F1 12 Tf 72 700 Td (Kwota:) Tj 100 0 Td (123,00) Tj ( z\\202) Tj 0 -14 Td (Za\\204) Tj (\\363\\202\\206) Tj ET";

#[test]
fn polish_text_from_differences_and_to_unicode_in_page_order() {
    let pdf = with_fonts(&[PAGE_1, PAGE_2]);
    let text = text_of(run(&pdf, &limits()));
    assert_eq!(
        text,
        "--- Strona 1 ---\nZażółć gęślą jaźń\nĄ x\n\n--- Strona 2 ---\nŁódź ąłż\nKwota: 123,00 zł\nZażółć"
    );
}

#[test]
fn public_entry_caches_and_ignores_other_files() {
    let pdf = with_fonts(&[PAGE_1]);
    assert_eq!(super::text("raport.docx", &pdf, 1000), None);
    assert!(is_pdf("SKAN.PDF"));
    let first = super::text("umowa.pdf", &pdf, 1000);
    assert!(
        matches!(&first, Some(Extracted::Text { text, truncated: false }) if text.contains("Zażółć"))
    );
    assert_eq!(
        super::text("kopia.pdf", &pdf, 1000),
        first,
        "ten sam plik z pamięci"
    );
    let big = vec![b' '; 25 * MIB + 1];
    let Some(Extracted::Note(note)) = super::text("duzy.pdf", &big, 1000) else {
        panic!("notka o rozmiarze");
    };
    assert!(note.contains("za duży"), "{note}");
}

/// PDF zaszyfrowany RC4-128 z hasłem użytkownika `user` (puste — tylko hasło właściciela).
fn encrypted(user: &str) -> Vec<u8> {
    let mut doc = Document::load_mem(&with_fonts(&[PAGE_1])).unwrap();
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &doc,
        owner_password: "wlasc",
        user_password: user,
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    doc.encrypt(&state).unwrap();
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

#[test]
fn password_protected_pdf_gets_a_note_and_owner_only_one_is_read() {
    let locked = note_of(run(&encrypted("tajne"), &limits()));
    assert!(locked.contains("zabezpieczony hasłem"), "{locked}");
    let open = text_of(run(&encrypted(""), &limits()));
    assert!(open.contains("Zażółć gęślą jaźń"), "{open}");
}

#[test]
fn broken_truncated_and_foreign_files_get_a_note() {
    let full = with_fonts(&[PAGE_1, PAGE_2]);
    for bytes in [
        b"to nie jest PDF".to_vec(),
        b"%PDF-1.7\nsmieci bez obiektow".to_vec(),
        full[..full.len() / 3].to_vec(),
        Vec::new(),
    ] {
        let note = note_of(run(&bytes, &limits()));
        assert!(note.contains("uszkodzony"), "{note}");
    }
    // Ucięty koniec (bez xref): odbudowa po obiektach daje tekst albo notkę — nigdy panikę.
    let _ = run(&full[..full.len() - 120], &limits());
}

#[test]
fn limits_cut_the_text_with_a_note() {
    let long: Vec<u8> = [
        b"BT /F1 12 Tf 72 720 Td ".as_slice(),
        &b"(slowo ) Tj ".repeat(200),
        b"ET",
    ]
    .concat();
    let pdf = with_fonts(&[&long, PAGE_2]);
    let few = Limits {
        max_chars: 400,
        ..limits()
    };
    let Extracted::Text {
        text,
        truncated: true,
    } = run(&pdf, &few)
    else {
        panic!("tekst ucięty");
    };
    assert!(
        text.contains("tekst niepełny — limit 400 znaków tekstu; przeczytane strony: 1 z 2"),
        "{text}"
    );
    assert!(text.chars().count() <= 400, "{}", text.chars().count());

    let one_page = Limits {
        max_pages: 1,
        ..limits()
    };
    let text = text_of(run(&pdf, &one_page));
    assert!(
        text.contains("limit 1 stron") && !text.contains("Strona 2"),
        "{text}"
    );

    let ops = Limits {
        max_ops: 3,
        ..limits()
    };
    assert!(note_of(run(&pdf, &ops)).contains("limit operacji"));
    let late = extract(&pdf, &limits(), Instant::now() - Duration::from_millis(1));
    assert!(note_of(late).contains("limit czasu"));
    let bytes = Limits {
        content_total: 10,
        ..limits()
    };
    assert!(note_of(run(&pdf, &bytes)).contains("rozpakowanych danych"));
    let page = Limits {
        page_content: 10,
        ..limits()
    };
    assert!(note_of(run(&pdf, &page)).contains("zbyt złożony"));
    let objects = Limits {
        max_objects: 3,
        ..limits()
    };
    assert!(note_of(run(&pdf, &objects)).contains("zbyt złożony"));
}

#[test]
fn scans_and_empty_pages_get_a_note() {
    let mut pdf = Pdf::new();
    let image = pdf.stream(
        "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
        b"\x00",
    );
    let root = pdf.pages(
        &[
            b"q 100 0 0 100 0 0 cm /Im1 Do Q",
            b"BI /W 1 /H 1 /CS /G /BPC 8 ID \x00 EI Q",
        ],
        &format!("<< /XObject << /Im1 {image} 0 R >> >>"),
    );
    let note = note_of(run(&pdf.finish(root), &limits()));
    assert!(note.contains("skan"), "{note}");

    let mut pdf = Pdf::new();
    let root = pdf.pages(&[b"", b"0 0 m 100 100 l S"], "<< >>");
    assert!(note_of(run(&pdf.finish(root), &limits())).contains("nie zawiera tekstu"));

    let mixed = with_fonts(&[b"q /Im1 Do Q", PAGE_1]);
    let text = text_of(run(&mixed, &limits()));
    assert!(
        text.starts_with("--- Strona 1 ---\n[pusta strona]"),
        "{text}"
    );
}

#[test]
fn form_xobjects_are_read_and_recursion_is_bounded() {
    let mut pdf = Pdf::new();
    let font = pdf.add(super::tests_build::POLISH_FONT);
    let form = pdf.reserve();
    let body = b"BT /F1 12 Tf 0 0 Td (Formularz) Tj ET /Fm1 Do";
    let dict = format!(
        "<< /Length {} /Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << /Font << /F1 \
         {font} 0 R >> /XObject << /Fm1 {form} 0 R >> >> >>\nstream\n",
        body.len()
    );
    pdf.set(form, [dict.as_bytes(), body, b"\nendstream"].concat());
    let root = pdf.pages(
        &[b"/Fm1 Do"],
        &format!("<< /XObject << /Fm1 {form} 0 R >> >>"),
    );
    let text = text_of(run(&pdf.finish(root), &limits()));
    assert_eq!(
        text.matches("Formularz").count(),
        limits().form_depth,
        "{text}"
    );
}

#[test]
fn hostile_cmaps_and_content_do_not_panic_or_hang() {
    let map = CMap::parse(
        b"1 beginbfrange <00000000> <FFFFFFFF> <0041> endbfrange 1 beginbfrange <00> <05> <> endbfrange \
          2 beginbfchar <01> endbfchar 1 begincodespacerange <00> endcodespacerange \
          1 beginbfrange <10> <12> [<0061> <0062>] endbfrange",
    );
    assert_eq!(map.lookup(4, 5).as_deref(), Some("F"));
    assert_eq!(map.lookup(1, 3), None);
    assert_eq!(map.lookup(1, 0x11).as_deref(), Some("b"));
    assert_eq!(map.lookup(1, 0x12), None);

    let brackets = b"[".repeat(10_000);
    let dicts = b"<<".repeat(10_000);
    let saves = [b"q ".repeat(1000), b"Q ".repeat(2000)].concat();
    let hostile: [&[u8]; 6] = [
        b"(niezamkniety \\",
        &brackets,
        &dicts,
        b"BI /W 1 ID bez konca",
        b"1e308 0 0 1e308 0 0 cm BT /F1 1e308 Tf (x) Tj ET",
        &saves,
    ];
    for content in hostile {
        let mut lex = Lexer::new(content);
        let mut n = 0;
        while lex.next().is_some() {
            n += 1;
            assert!(n <= content.len() + 1, "leksykon musi postępować");
        }
        let _ = run(&with_fonts(&[content]), &limits());
    }
    let mut lex = Lexer::new(b"/A#42 (a\\(b\\)\\101) <41 4> 4. -.5 >> }");
    let toks: Vec<Tok<'_>> = std::iter::from_fn(|| lex.next()).collect();
    assert_eq!(
        toks,
        vec![
            Tok::Name(b"AB".as_slice().into()),
            Tok::Str(b"a(b)A".to_vec()),
            Tok::Str(b"A@".to_vec()),
            Tok::Num(4.0),
            Tok::Num(-0.5),
            Tok::DictClose,
            Tok::Op(b"}"),
        ]
    );
}
