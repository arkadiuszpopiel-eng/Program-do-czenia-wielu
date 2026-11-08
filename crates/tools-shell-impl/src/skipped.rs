//! Cele usuwania w katalogach pomijanych przez snapshot zakresu (`node_modules`, `target` cargo,
//! `.venv`, `__pycache__`): cofanie ich nie obejmuje, więc usunięcie nie jest „odwracalne
//! w zakresie” i idzie jak trwałe (przegląd 2026-10-21).

/// Polecenia usuwania (jak w analizie powłoki); słowa po nich to cele.
const DELETE_WORDS: [&str; 8] = [
    "del",
    "erase",
    "rd",
    "rmdir",
    "remove-item",
    "rm",
    "ri",
    "clear-content",
];

/// Najgłębsze zagnieżdżenie (`cmd /c "rd /s /q node_modules"`), jak w analizie powłoki.
const MAX_DEPTH: u8 = 3;

/// Czy nazwa to katalog pomijany przez snapshot (ostrożnie — po samej nazwie).
fn skipped(name: &str) -> bool {
    undo_journal_contract::SNAPSHOT_SKIPPED_DIRS
        .iter()
        .chain(std::iter::once(
            &undo_journal_contract::SNAPSHOT_CARGO_TARGET,
        ))
        .any(|s| s.eq_ignore_ascii_case(name))
}

/// Czy cel któregoś polecenia usuwania leży w katalogu pomijanym przez snapshot. Liczą się tylko
/// słowa po poleceniu usuwania (także w poleceniu zagnieżdżonym w cudzysłowie) — wywołanie
/// programu z `.venv\Scripts` albo przekierowanie wyniku do pliku nie.
pub(crate) fn deletes_unsnapshotted(command: &str) -> bool {
    targets_at(command, 0)
}

fn targets_at(command: &str, depth: u8) -> bool {
    let mut deleting = false;
    for t in tools_shell_contract::tokens(command) {
        if depth < MAX_DEPTH && t.contains(char::is_whitespace) && targets_at(&t, depth + 1) {
            return true;
        }
        let lower = t.to_lowercase();
        let base = lower.rsplit(['\\', '/']).next().unwrap_or(&lower);
        if DELETE_WORDS.contains(&base.strip_suffix(".exe").unwrap_or(base)) {
            deleting = true;
            continue;
        }
        if deleting && lower.split(['/', '\\', ':']).any(skipped) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::deletes_unsnapshotted as d;

    #[test]
    fn delete_targets_in_skipped_dirs() {
        for cmd in [
            "Remove-Item -Recurse -Force node_modules",
            r"rm -rf .\target\debug",
            r#"cmd /c "rd /s /q node_modules""#,
            r#"powershell -Command "Remove-Item -Recurse .venv""#,
            "Remove-Item -Path:__pycache__ -Recurse",
            r"del C:\Users\ala\Projekt\node_modules\x.js",
        ] {
            assert!(d(cmd), "{cmd}");
        }
        for cmd in [
            r".venv\Scripts\python -m pytest > raport.txt",
            r"node_modules\.bin\eslint . > lint.txt",
            "cargo build --target-dir target 2> err.txt",
            "Remove-Item a.txt",
            "Get-ChildItem node_modules",
        ] {
            assert!(!d(cmd), "{cmd}");
        }
    }
}
