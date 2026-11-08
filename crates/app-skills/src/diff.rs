//! Diff wierszy (LCS) do podglądu propozycji umiejętności: wersja zainstalowana → proponowana.
//! Wejście to sformatowany JSON przepisu (dziesiątki–setki wierszy), więc tablica O(n·m) wystarcza;
//! powyżej limitu — diff „całość usunięta / całość dodana" (bez utraty informacji).

use app_api::dto::{DiffKind, DiffLine};

/// Najwięcej komórek tablicy LCS.
const MAX_CELLS: usize = 4_000_000;

fn line(kind: DiffKind, text: &str) -> DiffLine {
    DiffLine {
        kind,
        text: text.to_owned(),
    }
}

/// Diff `old` → `new` (wiersze).
pub fn diff_lines(old: &str, new: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    if a.len().saturating_mul(b.len()) > MAX_CELLS {
        let mut out: Vec<DiffLine> = a.iter().map(|l| line(DiffKind::Removed, l)).collect();
        out.extend(b.iter().map(|l| line(DiffKind::Added, l)));
        return out;
    }
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::with_capacity(n.max(m));
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(line(DiffKind::Same, a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(line(DiffKind::Removed, a[i]));
            i += 1;
        } else {
            out.push(line(DiffKind::Added, b[j]));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| line(DiffKind::Removed, l)));
    out.extend(b[j..].iter().map(|l| line(DiffKind::Added, l)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(d: &[DiffLine]) -> String {
        d.iter()
            .map(|l| match l.kind {
                DiffKind::Same => '=',
                DiffKind::Added => '+',
                DiffKind::Removed => '-',
            })
            .collect()
    }

    #[test]
    fn diff_marks_changes_and_keeps_order() {
        let d = diff_lines("a\nb\nc", "a\nx\nc\nd");
        assert_eq!(kinds(&d), "=-+=+");
        assert_eq!(d[2].text, "x");
        assert_eq!(kinds(&diff_lines("", "a\nb")), "++");
        assert_eq!(kinds(&diff_lines("a", "")), "-");
        assert_eq!(kinds(&diff_lines("a\nb", "a\nb")), "==");
    }
}
