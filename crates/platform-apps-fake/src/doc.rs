//! Dokument atrapy Office w pamięci i jego zapis tekstowy (bajty pliku „.docx”/„.xlsx” atrapy).
//!
//! Format (UTF-8, linia = wpis; `\n`, `\t`, `\\` w treści są kodowane):
//! `ALFA-FAKE-OFFICE 1`, `macro <nazwa>`, `para <tekst>`, `table` / `row <k1>\t<k2>` / `end`,
//! `sheet <nazwa>`, `cell <A1> <n|t|b|f|e> <wartość>`.

use std::collections::BTreeMap;

use platform_apps_contract::{CellValue, cell_name, parse_cell};

const MAGIC: &str = "ALFA-FAKE-OFFICE 1";

/// Blok dokumentu Word.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Akapit.
    Para(String),
    /// Tabela.
    Table(Vec<Vec<String>>),
}

/// Komórka arkusza.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// Wartość.
    Value(CellValue),
    /// Formuła (wartość nieprzeliczona — tryb ręczny jak w implementacji).
    Formula(String),
}

/// Arkusz.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sheet {
    /// Nazwa.
    pub name: String,
    /// Komórki (kolumna, wiersz).
    pub cells: BTreeMap<(u32, u32), Cell>,
}

impl Sheet {
    /// Używany zakres (`A1` dla pustego).
    pub fn used_range(&self) -> String {
        let cols = self.cells.keys().map(|(c, _)| *c);
        let rows = self.cells.keys().map(|(_, r)| *r);
        match (
            cols.clone().min(),
            cols.max(),
            rows.clone().min(),
            rows.max(),
        ) {
            (Some(c1), Some(c2), Some(r1), Some(r2)) => {
                format!("{}:{}", cell_name(c1, r1), cell_name(c2, r2))
            }
            _ => "A1".into(),
        }
    }
}

/// Dokument atrapy (Word: bloki; Excel: arkusze).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FakeDocument {
    /// Makra (projekt VBA) — atrapa „uruchamia” je, gdy makra nie są wyłączone.
    pub macros: Vec<String>,
    /// Bloki Worda.
    pub blocks: Vec<Block>,
    /// Arkusze Excela.
    pub sheets: Vec<Sheet>,
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

fn unesc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

impl FakeDocument {
    /// Dokument Word z akapitami.
    pub fn word(paragraphs: &[&str]) -> Self {
        Self {
            blocks: paragraphs
                .iter()
                .map(|p| Block::Para((*p).into()))
                .collect(),
            ..Self::default()
        }
    }

    /// Dokument Excel z jednym pustym arkuszem.
    pub fn excel(sheet: &str) -> Self {
        Self {
            sheets: vec![Sheet {
                name: sheet.into(),
                cells: BTreeMap::new(),
            }],
            ..Self::default()
        }
    }

    /// Dodaje makro (builder).
    #[must_use]
    pub fn with_macro(mut self, name: &str) -> Self {
        self.macros.push(name.into());
        self
    }

    /// Dodaje tabelę (builder).
    #[must_use]
    pub fn with_table(mut self, rows: &[&[&str]]) -> Self {
        let rows = rows
            .iter()
            .map(|r| r.iter().map(|c| (*c).to_owned()).collect())
            .collect();
        self.blocks.push(Block::Table(rows));
        self
    }

    /// Ustawia komórkę pierwszego arkusza (builder; `None` gdy adres niepoprawny lub brak arkusza).
    #[must_use]
    pub fn with_cell(mut self, a1: &str, value: CellValue) -> Self {
        if let (Some(at), Some(sheet)) = (parse_cell(a1), self.sheets.first_mut()) {
            sheet.cells.insert(at, Cell::Value(value));
        }
        self
    }

    /// Zapis do bajtów pliku atrapy.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![MAGIC.to_owned()];
        out.extend(self.macros.iter().map(|m| format!("macro {}", esc(m))));
        for b in &self.blocks {
            match b {
                Block::Para(p) => out.push(format!("para {}", esc(p))),
                Block::Table(rows) => {
                    out.push("table".into());
                    for r in rows {
                        let cells: Vec<String> = r.iter().map(|c| esc(c)).collect();
                        out.push(format!("row {}", cells.join("\t")));
                    }
                    out.push("end".into());
                }
            }
        }
        for s in &self.sheets {
            out.push(format!("sheet {}", esc(&s.name)));
            for ((c, r), cell) in &s.cells {
                let (kind, v) = match cell {
                    Cell::Formula(f) => ("f", esc(f)),
                    Cell::Value(CellValue::Empty) => continue,
                    Cell::Value(CellValue::Number(n)) => ("n", n.to_string()),
                    Cell::Value(CellValue::Bool(b)) => ("b", b.to_string()),
                    Cell::Value(CellValue::Text(t)) => ("t", esc(t)),
                    Cell::Value(CellValue::Error { error }) => ("e", esc(error)),
                };
                out.push(format!("cell {} {kind} {v}", cell_name(*c, *r)));
            }
        }
        (out.join("\n") + "\n").into_bytes()
    }

    /// Odczyt z bajtów (`None` = plik uszkodzony / nie z atrapy).
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(bytes).ok()?;
        let mut lines = text.lines();
        if lines.next()? != MAGIC {
            return None;
        }
        let mut doc = Self::default();
        let mut table: Option<Vec<Vec<String>>> = None;
        for line in lines {
            let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
            match (tag, table.as_mut()) {
                ("row", Some(t)) => t.push(rest.split('\t').map(unesc).collect()),
                ("end", Some(_)) => doc.blocks.push(Block::Table(table.take()?)),
                ("macro", None) => doc.macros.push(unesc(rest)),
                ("para", None) => doc.blocks.push(Block::Para(unesc(rest))),
                ("table", None) => table = Some(Vec::new()),
                ("sheet", None) => doc.sheets.push(Sheet {
                    name: unesc(rest),
                    cells: BTreeMap::new(),
                }),
                ("cell", None) => {
                    let mut parts = rest.splitn(3, ' ');
                    let at = parse_cell(parts.next()?)?;
                    let kind = parts.next()?;
                    let v = unesc(parts.next().unwrap_or(""));
                    let cell = match kind {
                        "f" => Cell::Formula(v),
                        "n" => Cell::Value(CellValue::Number(v.parse().ok()?)),
                        "b" => Cell::Value(CellValue::Bool(v == "true")),
                        "e" => Cell::Value(CellValue::Error { error: v }),
                        _ => Cell::Value(CellValue::Text(v)),
                    };
                    doc.sheets.last_mut()?.cells.insert(at, cell);
                }
                _ => return None,
            }
        }
        table.is_none().then_some(doc)
    }

    /// Tekst Worda (akapity i wiersze tabel rozdzielone tabulatorem).
    pub fn word_text(&self) -> String {
        let mut out = Vec::new();
        for b in &self.blocks {
            match b {
                Block::Para(p) => out.push(p.clone()),
                Block::Table(rows) => out.extend(rows.iter().map(|r| r.join("\t"))),
            }
        }
        out.join("\n")
    }

    /// Tabele Worda.
    pub fn tables(&self) -> Vec<Vec<Vec<String>>> {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Table(t) => Some(t.clone()),
                Block::Para(_) => None,
            })
            .collect()
    }

    /// Arkusz po nazwie (bez wielkości liter) albo pierwszy.
    pub fn sheet(&self, name: Option<&str>) -> Option<&Sheet> {
        match name {
            Some(n) => self.sheets.iter().find(|s| s.name.eq_ignore_ascii_case(n)),
            None => self.sheets.first(),
        }
    }

    /// Arkusz do zmiany.
    pub fn sheet_mut(&mut self, name: Option<&str>) -> Option<&mut Sheet> {
        match name {
            Some(n) => self
                .sheets
                .iter_mut()
                .find(|s| s.name.eq_ignore_ascii_case(n)),
            None => self.sheets.first_mut(),
        }
    }
}
