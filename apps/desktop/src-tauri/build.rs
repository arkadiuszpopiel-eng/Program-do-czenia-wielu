//! Build powłoki: `tauri-build` z manifestem komend aplikacji — dla każdej komendy z
//! `ui/src/lib/api/COMMANDS.md` powstają uprawnienia `allow-<komenda>`/`deny-<komenda>`, a
//! capabilities okien (`capabilities/*.json`) nadają je minimalnie, per okno (PLAN §8.2).
//! Ta sama lista jest sprawdzana testem w `crates/app-core` (`app_core::COMMANDS` = COMMANDS.md).

use std::path::Path;

fn commands_from_md(text: &str) -> Vec<String> {
    let Some(section) = text.split("## Komendy").nth(1) else {
        return Vec::new();
    };
    let section = section.split("## Zdarzenia").next().unwrap_or_default();
    let mut out = Vec::new();
    for line in section.lines().filter(|l| l.starts_with("| `")) {
        if let Some(first) = line.split('|').nth(1) {
            out.extend(first.split('`').skip(1).step_by(2).map(str::to_owned));
        }
    }
    out
}

fn main() {
    let md = Path::new("../ui/src/lib/api/COMMANDS.md");
    println!("cargo:rerun-if-changed={}", md.display());
    let text = match std::fs::read_to_string(md) {
        Ok(text) => text,
        Err(e) => {
            println!("cargo:warning=brak COMMANDS.md ({e}) — manifest komend pusty");
            String::new()
        }
    };
    let commands: Vec<&'static str> = commands_from_md(&text)
        .into_iter()
        .map(|c| &*Box::leak(c.into_boxed_str()))
        .collect();
    let commands: &'static [&'static str] = Box::leak(commands.into_boxed_slice());
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(commands));
    if let Err(e) = tauri_build::try_build(attributes) {
        println!("cargo:warning=tauri-build: {e:#}");
        std::process::exit(1);
    }
}
