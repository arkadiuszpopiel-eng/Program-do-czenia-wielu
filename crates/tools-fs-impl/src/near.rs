//! Podpowiedź przy ścieżce, której nie ma: mały model przepisuje długie ścieżki z literówką
//! („…\Sesje\Nova rozmowa” zamiast „Nowa rozmowa”) i kończył zadanie przekonany, że katalog
//! roboczy nie istnieje (próba obowiązku „tidy”, 1.11). Szukamy tylko w katalogu roboczym sesji
//! (tam model ma już dostęp do odczytu) i nigdy nie podpowiadamy ścieżek z deny-listy. Podpowiedź
//! niczego nie otwiera — model musi wywołać narzędzie ponownie z poprawioną ścieżką.

use std::path::{Component, Path, PathBuf};

use platform_contract::PlatformError;
use tools_common_contract::{ToolErrorKind, ToolOutcome};

use crate::core::{Core, platform_failure, shown};

/// Najwięcej poprawionych członów jednej ścieżki.
const MAX_FIXES: usize = 2;

/// Odległość edycyjna (Levenshtein) na znakach, bez rozróżniania wielkości liter.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Dopuszczalna liczba literówek dla nazwy tej długości.
fn tolerance(name: &str) -> usize {
    match name.chars().count() {
        0..=4 => 1,
        _ => 2,
    }
}

impl Core {
    /// Jedyna nazwa w `dir` bliska `name` (literówka), gdy taka jest.
    fn closest(&self, dir: &Path, name: &str) -> Option<PathBuf> {
        let entries = self.fs.list_dir(dir).ok()?;
        let mut best: Vec<(usize, PathBuf)> = entries
            .into_iter()
            .filter_map(|e| {
                let n = e.path.file_name()?.to_string_lossy().into_owned();
                // Różnica długości większa niż tolerancja wyklucza dopasowanie — bez liczenia
                // odległości (człon od modelu może mieć tysiące znaków).
                if n.chars().count().abs_diff(name.chars().count()) > tolerance(name) {
                    return None;
                }
                let d = distance(&n, name);
                (d > 0 && d <= tolerance(name)).then_some((d, e.path))
            })
            .collect();
        best.sort_by_key(|(d, _)| *d);
        match best.as_slice() {
            [(_, p)] => Some(p.clone()),
            [(d1, p), (d2, _), ..] if d1 < d2 => Some(p.clone()),
            _ => None,
        }
        .filter(|p| !self.denied(&p.to_string_lossy()))
    }

    /// Istniejąca ścieżka w katalogu roboczym różniąca się od `path` literówkami w najwyżej
    /// [`MAX_FIXES`] członach — jako zdanie dla modelu („ Czy chodziło o „…”?”).
    pub(crate) fn near_miss(&self, path: &str, workdir: Option<&str>) -> String {
        let Some(wd) = workdir.filter(|w| !w.is_empty()) else {
            return String::new();
        };
        let Some(fixed) = self.fix_path(Path::new(path), Path::new(wd)) else {
            return String::new();
        };
        let fixed = fixed.to_string_lossy().into_owned();
        format!(" Czy chodziło o „{}”?", shown(&fixed, Some(wd)))
    }

    /// Błąd platformy jak [`platform_failure`], przy braku ścieżki — z podpowiedzią literówki.
    pub(crate) fn failure_near(
        &self,
        e: &PlatformError,
        action: &str,
        path: &str,
        workdir: Option<&str>,
    ) -> ToolOutcome {
        match e {
            PlatformError::NotFound(_) => ToolOutcome::failed(
                ToolErrorKind::NotFound,
                format!(
                    "Nie wykonano: {action} — {e}.{}",
                    self.near_miss(path, workdir)
                ),
            ),
            _ => platform_failure(e, action),
        }
    }

    fn fix_path(&self, path: &Path, workdir: &Path) -> Option<PathBuf> {
        // Od rodzica katalogu roboczego — literówka bywa w nazwie samego katalogu sesji. Człony
        // powyżej porównujemy z tolerancją: model przepisywał „…\Local\Tepm\…” (pomiar F3, 1.11).
        let root = workdir.parent().unwrap_or(workdir);
        let fold = |c: Component<'_>| c.as_os_str().to_string_lossy().to_lowercase();
        let base = root.components().count();
        if path.components().count() < base {
            return None;
        }
        let mut fixes = 0;
        for (a, b) in path.components().zip(root.components()) {
            if fold(a) == fold(b) {
                continue;
            }
            let (Component::Normal(a), Component::Normal(b)) = (a, b) else {
                return None;
            };
            let (a, b) = (a.to_string_lossy(), b.to_string_lossy());
            if distance(&a, &b) > tolerance(&b) {
                return None;
            }
            fixes += 1;
        }
        let mut current = root.to_path_buf();
        for part in path.components().skip(base) {
            let Component::Normal(name) = part else {
                return None;
            };
            let next = current.join(name);
            if self.fs.exists(&next) {
                current = next;
                continue;
            }
            fixes += 1;
            if fixes > MAX_FIXES {
                return None;
            }
            current = self.closest(&current, &name.to_string_lossy())?;
        }
        // Także ścieżka końcowa (człony po poprawce dopasowane dokładnie) — nigdy z deny-listy.
        let safe = !self.denied(&current.to_string_lossy());
        (fixes > 0 && safe && current.starts_with(workdir) && self.fs.exists(&current))
            .then_some(current)
    }
}

/// Nazwy Kosza, które model wpisuje jako katalog celu.
const TRASH: &[&str] = &["kosz", "trash", "recycle bin", "$recycle.bin", "recycler"];

/// Kopia albo przeniesienie „do Kosza” ścieżką (`/Kosz/plik`, `C:\$Recycle.Bin\…`): Kosz nie jest
/// katalogiem dla narzędzi — model kopiował tam plik zamiast wywołać `fs_delete` i powtarzał
/// błąd do końca budżetu (pomiar F3 fs-24, 2.11). Tylko ścieżka od korzenia dysku albo
/// `$Recycle.Bin` gdziekolwiek; zwykły katalog „Kosz” w katalogu roboczym zostaje katalogiem.
pub(crate) fn trash_target(raw: &str) -> Option<Box<ToolOutcome>> {
    let rest = raw.trim();
    let rest = match rest.as_bytes() {
        [d, b':', ..] if d.is_ascii_alphabetic() => &rest[2..],
        _ => rest,
    };
    let sep = ['/', '\\'];
    let first = rest
        .split(sep)
        .find(|s| !s.is_empty())
        .map(str::to_lowercase);
    let hit = rest.starts_with(sep) && first.is_some_and(|f| TRASH.contains(&f.as_str()))
        || rest
            .split(sep)
            .any(|s| s.eq_ignore_ascii_case("$recycle.bin"));
    hit.then(|| {
        Box::new(ToolOutcome::failed(
            ToolErrorKind::InvalidArgs,
            format!(
                "Nie wykonano: „{raw}” — Kosz nie jest katalogiem dla narzędzi plików. Żeby przenieść \
                 plik do Kosza, wywołaj fs_delete z jego ścieżką (można to cofnąć)."
            ),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_counts_typos() {
        assert_eq!(distance("Nowa rozmowa", "Nova rozmowa"), 1);
        assert_eq!(distance("raport.md", "RAPORT.MD"), 0);
        assert_eq!(distance("Temp", "Tepm"), 2);
        assert_eq!(distance("a", "xyz"), 3);
    }

    #[test]
    fn short_names_tolerate_one_typo() {
        assert_eq!(tolerance("ab"), 1);
        assert_eq!(tolerance("Nowa rozmowa"), 2);
    }
}
