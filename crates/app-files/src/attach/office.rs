//! Tekst z dokumentów Office Open XML dla modelu (C2.11): DOCX (akapity, tabulatory, podziały),
//! PPTX (slajdy po kolei) i XLSX (arkusze, wiersze, komórki z tekstem wspólnym). Czysty Rust:
//! archiwum `zip` i prosty skaner XML — bez makr, bez uruchamiania czegokolwiek. Limity chronią
//! przed bombą zip: najwyżej 2000 wpisów i 32 MiB rozpakowanego XML na dokument.

use std::io::{Cursor, Read};

/// Najwięcej rozpakowanych bajtów XML z jednego dokumentu.
const MAX_XML: u64 = 32 * 1024 * 1024;
/// Najwięcej wpisów w archiwum.
const MAX_ENTRIES: usize = 2000;

/// Rodzaj dokumentu po rozszerzeniu nazwy.
fn kind(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    ["docx", "pptx", "xlsx"]
        .into_iter()
        .find(|ext| lower.ends_with(&format!(".{ext}")))
}

/// Czy plik wygląda na dokument Office Open XML (rozszerzenie i nagłówek ZIP).
pub fn is_office(name: &str, bytes: &[u8]) -> bool {
    kind(name).is_some() && bytes.starts_with(b"PK\x03\x04")
}

struct Archive {
    zip: zip::ZipArchive<Cursor<Vec<u8>>>,
    budget: u64,
}

impl Archive {
    fn open(bytes: &[u8]) -> Option<Self> {
        let zip = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).ok()?;
        (zip.len() <= MAX_ENTRIES).then_some(Self {
            zip,
            budget: MAX_XML,
        })
    }

    fn names(&self) -> Vec<String> {
        // Nazwy nieczytelne (zły zapis znaków) pomijane — dokument i tak ich nie potrzebuje.
        self.zip
            .file_names()
            .filter_map(|n| n.ok().map(|n| n.into_owned()))
            .collect()
    }

    /// Wpis jako tekst w ramach wspólnego limitu rozpakowanych bajtów.
    fn read(&mut self, name: &str) -> Option<String> {
        let file = self.zip.by_name(name).ok()?;
        if file.size() > self.budget {
            return None;
        }
        let mut out = String::new();
        file.take(self.budget + 1).read_to_string(&mut out).ok()?;
        let used = out.len() as u64;
        if used > self.budget {
            return None;
        }
        self.budget -= used;
        Some(out)
    }
}

/// Zdarzenie skanera XML.
enum Ev<'a> {
    Open(&'a str, &'a str),
    Close(&'a str),
    Empty(&'a str),
    Text(String),
}

/// Nazwa znacznika bez przestrzeni nazw (`w:t` → `t`).
fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Encje XML (`&amp;`, `&#x41;`…); nieznane zostają bez zmian.
fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..end];
        let ch = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Prosty skaner XML: znaczniki i tekst (bez CDATA, komentarze i deklaracje pomijane).
fn scan(xml: &str, mut f: impl FnMut(Ev<'_>)) {
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        if lt > 0 {
            f(Ev::Text(decode(&rest[..lt])));
        }
        let tag = &rest[lt + 1..];
        let Some(gt) = tag.find('>') else { return };
        let body = &tag[..gt];
        rest = &tag[gt + 1..];
        if body.starts_with('?') || body.starts_with('!') {
            continue;
        }
        if let Some(name) = body.strip_prefix('/') {
            f(Ev::Close(local(name.trim())));
            continue;
        }
        let empty = body.ends_with('/');
        let body = body.trim_end_matches('/');
        let (name, attrs) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
        if empty {
            f(Ev::Empty(local(name)));
        } else {
            f(Ev::Open(local(name), attrs));
        }
    }
}

/// Wartość atrybutu (`r="A1"`).
fn attr<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    let at = attrs.find(&format!("{key}=\""))? + key.len() + 2;
    let rest = &attrs[at..];
    Some(&rest[..rest.find('"')?])
}

/// Tekst akapitów: `t` wewnątrz akapitu `p`, tabulator i podział linii (DOCX i PPTX).
fn paragraphs(xml: &str) -> String {
    let mut out = String::new();
    let mut in_text = false;
    scan(xml, |ev| match ev {
        Ev::Open("t", _) => in_text = true,
        Ev::Close("t") => in_text = false,
        Ev::Text(t) if in_text => out.push_str(&t),
        Ev::Empty("tab") => out.push('\t'),
        Ev::Empty("br" | "cr") | Ev::Close("p") => out.push('\n'),
        _ => {}
    });
    out
}

/// Numer z nazwy wpisu (`ppt/slides/slide12.xml` → 12) do sortowania.
fn number(name: &str, prefix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?
        .strip_suffix(".xml")?
        .parse()
        .ok()
}

fn sorted(names: &[String], prefix: &str) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = names
        .iter()
        .filter_map(|n| Some((number(n, prefix)?, n.clone())))
        .collect();
    out.sort();
    out
}

fn docx(a: &mut Archive) -> Option<String> {
    a.read("word/document.xml").map(|xml| paragraphs(&xml))
}

fn pptx(a: &mut Archive) -> Option<String> {
    let names = a.names();
    let mut out = String::new();
    for (n, name) in sorted(&names, "ppt/slides/slide") {
        let xml = a.read(&name)?;
        out.push_str(&format!("--- Slajd {n} ---\n{}\n", paragraphs(&xml).trim()));
    }
    (!out.is_empty()).then_some(out)
}

fn xlsx(a: &mut Archive) -> Option<String> {
    let shared: Vec<String> = match a.read("xl/sharedStrings.xml") {
        Some(xml) => {
            let mut list = Vec::new();
            let mut current: Option<String> = None;
            let mut in_text = false;
            scan(&xml, |ev| match ev {
                Ev::Open("si", _) => current = Some(String::new()),
                Ev::Close("si") => list.push(current.take().unwrap_or_default()),
                Ev::Open("t", _) => in_text = true,
                Ev::Close("t") => in_text = false,
                Ev::Text(t) if in_text => {
                    if let Some(c) = current.as_mut() {
                        c.push_str(&t);
                    }
                }
                _ => {}
            });
            list
        }
        None => Vec::new(),
    };
    let names = a.names();
    let mut out = String::new();
    for (n, name) in sorted(&names, "xl/worksheets/sheet") {
        let xml = a.read(&name)?;
        out.push_str(&format!("--- Arkusz {n} ---\n"));
        let mut row: Vec<String> = Vec::new();
        let mut cell_type = String::new();
        let mut value = String::new();
        let mut in_value = false;
        scan(&xml, |ev| match ev {
            Ev::Open("c", attrs) => {
                cell_type = attr(attrs, "t").unwrap_or("n").to_owned();
                value.clear();
            }
            Ev::Open("v" | "t", _) => in_value = true,
            Ev::Close("v" | "t") => in_value = false,
            Ev::Text(t) if in_value => value.push_str(&t),
            Ev::Close("c") => {
                let shown = if cell_type == "s" {
                    value
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| shared.get(i).cloned())
                        .unwrap_or_default()
                } else {
                    value.clone()
                };
                row.push(shown);
            }
            Ev::Close("row") => {
                if row.iter().any(|c| !c.is_empty()) {
                    out.push_str(&row.join("\t"));
                    out.push('\n');
                }
                row.clear();
            }
            _ => {}
        });
    }
    (!out.is_empty()).then_some(out)
}

/// Tekst dokumentu albo `None` (nie Office, uszkodzony, za duży albo pusty).
pub fn text(name: &str, bytes: &[u8]) -> Option<String> {
    if !is_office(name, bytes) {
        return None;
    }
    let mut archive = Archive::open(bytes)?;
    let raw = match kind(name)? {
        "docx" => docx(&mut archive),
        "pptx" => pptx(&mut archive),
        _ => xlsx(&mut archive),
    }?;
    let lines: Vec<&str> = raw.lines().map(str::trim_end).collect();
    let text = lines.join("\n").trim().to_owned();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            for (name, body) in entries {
                zip.start_file(*name, opts).unwrap();
                zip.write_all(body.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn docx_paragraphs_tabs_and_entities() {
        let doc = archive(&[(
            "word/document.xml",
            r#"<?xml version="1.0"?><w:document><w:body><w:p><w:r><w:t>Zażółć &amp; gęślą</w:t></w:r><w:r><w:tab/><w:t xml:space="preserve"> jaźń</w:t></w:r></w:p><w:p><w:r><w:t>Druga &#x41;</w:t></w:r></w:p></w:body></w:document>"#,
        )]);
        assert_eq!(
            text("raport.DOCX", &doc).as_deref(),
            Some("Zażółć & gęślą\t jaźń\nDruga A")
        );
        assert_eq!(text("raport.txt", &doc), None, "rozszerzenie");
        assert_eq!(text("x.docx", b"nie zip"), None);
    }

    #[test]
    fn pptx_slides_in_order_and_xlsx_cells() {
        let deck = archive(&[
            (
                "ppt/slides/slide10.xml",
                "<p:sld><a:p><a:t>Dziesiąty</a:t></a:p></p:sld>",
            ),
            (
                "ppt/slides/slide2.xml",
                "<p:sld><a:p><a:t>Drugi</a:t></a:p></p:sld>",
            ),
        ]);
        let t = text("deck.pptx", &deck).unwrap();
        assert!(
            t.find("Slajd 2").unwrap() < t.find("Slajd 10").unwrap(),
            "{t}"
        );
        let book = archive(&[
            (
                "xl/sharedStrings.xml",
                "<sst><si><t>Nazwa</t></si><si><t>Jabłka</t></si></sst>",
            ),
            (
                "xl/worksheets/sheet1.xml",
                r#"<worksheet><sheetData><row><c r="A1" t="s"><v>0</v></c><c r="B1"><v>12.5</v></c></row><row><c r="A2" t="s"><v>1</v></c><c r="B2" t="inlineStr"><is><t>dużo</t></is></c></row></sheetData></worksheet>"#,
            ),
        ]);
        assert_eq!(
            text("ceny.xlsx", &book).as_deref(),
            Some("--- Arkusz 1 ---\nNazwa\t12.5\nJabłka\tdużo")
        );
    }

    #[test]
    fn limits_and_broken_input() {
        assert_eq!(decode("a &bogus; &#xZZ; &"), "a &bogus; &#xZZ; &");
        let empty = archive(&[("word/document.xml", "<w:document/>")]);
        assert_eq!(text("pusty.docx", &empty), None);
        let mut big = String::from("<w:p><w:t>");
        big.push_str(&"x".repeat(1024));
        big.push_str("</w:t></w:p>");
        let doc = archive(&[("word/document.xml", &big)]);
        assert_eq!(text("a.docx", &doc).map(|t| t.len()), Some(1024));
    }

    /// Regresja klasy RUSTSEC-2026-0368/0369 (dowiązania w archiwum): wpis-dowiązanie nie jest
    /// rozwiązywany — dokument czytany jest tylko z pamięci, nigdy z pliku spoza archiwum.
    #[test]
    fn symlink_entry_is_never_followed() {
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("tajne.xml");
        std::fs::write(&secret, "<w:p><w:t>tajne</w:t></w:p>").unwrap();
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            zip.add_symlink("word/document.xml", secret.to_string_lossy(), opts)
                .unwrap();
            zip.finish().unwrap();
        }
        assert_eq!(text("link.docx", &buf.into_inner()), None);
    }
}
