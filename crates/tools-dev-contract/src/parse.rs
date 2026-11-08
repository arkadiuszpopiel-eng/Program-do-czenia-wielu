//! Odczyt wyników: podsumowania testów (cargo, pytest, vitest/jest, go, dotnet) z nazwami
//! nieudanych testów i stan repozytorium z `git status --porcelain=v1 --branch`.

use crate::{GitFile, GitStatus, Runner};

/// Najwięcej nazw nieudanych testów w wyniku.
pub const MAX_FAILURES: usize = 50;

/// Liczniki z wyjścia testów.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    /// Zaliczone.
    pub passed: u32,
    /// Niezaliczone.
    pub failed: u32,
    /// Pominięte.
    pub skipped: u32,
    /// Nazwy niezaliczonych (≤ [`MAX_FAILURES`]).
    pub failures: Vec<String>,
    /// Czy znaleziono jakiekolwiek podsumowanie.
    pub found: bool,
}

/// Liczba przy słowie: po dwukropku („Passed: 5”), tuż przed („5 passed”) albo tuż po („ahead 2”).
fn number_near(line: &str, word: &str) -> Option<u32> {
    let lower = line.to_lowercase();
    lower
        .match_indices(word)
        .find_map(|(at, _)| number_at(&lower, at, word.len()))
}

fn number_at(lower: &str, at: usize, len: usize) -> Option<u32> {
    let rest = &lower[at + len..];
    let digits = |s: &str| -> String {
        s.trim_start_matches([':', ' '])
            .chars()
            .take_while(char::is_ascii_digit)
            .collect()
    };
    if rest.trim_start().starts_with(':') {
        return digits(rest).parse().ok();
    }
    let before: String = lower[..at]
        .trim_end()
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if before.is_empty() {
        digits(rest).parse().ok()
    } else {
        before.parse().ok()
    }
}

fn push_failure(c: &mut Counts, name: &str) {
    let name = name.trim();
    if !name.is_empty() && c.failures.len() < MAX_FAILURES && !c.failures.iter().any(|f| f == name)
    {
        c.failures.push(name.to_owned());
    }
}

/// Liczniki z wyjścia testów danego rodzaju.
pub fn test_counts(runner: Runner, out: &str) -> Counts {
    let mut c = Counts::default();
    for line in out.lines() {
        let l = line.trim();
        match runner {
            Runner::Cargo => {
                if l.starts_with("test result:") {
                    c.found = true;
                    c.passed += number_near(l, "passed").unwrap_or(0);
                    c.failed += number_near(l, "failed").unwrap_or(0);
                    c.skipped += number_near(l, "ignored").unwrap_or(0);
                } else if let Some(name) = l
                    .strip_prefix("test ")
                    .and_then(|r| r.strip_suffix(" ... FAILED"))
                {
                    push_failure(&mut c, name);
                }
            }
            Runner::Pytest => {
                if l.starts_with('=')
                    && (l.contains(" passed") || l.contains(" failed") || l.contains(" error"))
                {
                    c.found = true;
                    c.passed = number_near(l, "passed").unwrap_or(0);
                    c.failed = number_near(l, "failed").unwrap_or(0)
                        + number_near(l, "error").unwrap_or(0);
                    c.skipped = number_near(l, "skipped").unwrap_or(0);
                } else if let Some(name) = l.strip_prefix("FAILED ") {
                    push_failure(&mut c, name.split(" - ").next().unwrap_or(name));
                }
            }
            Runner::Npm | Runner::Pnpm | Runner::Yarn => {
                let lower = l.to_lowercase();
                // vitest: „Tests  2 failed | 10 passed (12)”; jest: „Tests:       1 failed, 5 passed, 6 total”.
                if lower.starts_with("tests ") || lower.starts_with("tests:") {
                    c.found = true;
                    c.passed = number_near(l, "passed").unwrap_or(0);
                    c.failed = number_near(l, "failed").unwrap_or(0);
                    c.skipped = number_near(l, "skipped").unwrap_or(0)
                        + number_near(l, "todo").unwrap_or(0);
                } else if let Some(name) = l
                    .strip_prefix("FAIL ")
                    .or_else(|| l.strip_prefix("× "))
                    .or_else(|| l.strip_prefix("● "))
                {
                    push_failure(&mut c, name);
                }
            }
            Runner::Go => {
                if let Some(name) = l.strip_prefix("--- FAIL: ") {
                    c.found = true;
                    c.failed += 1;
                    push_failure(&mut c, name.split(' ').next().unwrap_or(name));
                } else if l.starts_with("--- PASS: ") {
                    c.found = true;
                    c.passed += 1;
                } else if l.starts_with("--- SKIP: ") {
                    c.found = true;
                    c.skipped += 1;
                } else if l.starts_with("ok ") && c.passed == 0 {
                    // Bez `-v` go podaje tylko „ok pakiet” — pakiet liczony jako zaliczony.
                    c.found = true;
                    c.passed += 1;
                }
            }
            Runner::Dotnet => {
                // „Passed!  - Failed: 0, Passed: 12, Skipped: 0, Total: 12”
                if l.contains("Failed:") && l.contains("Passed:") {
                    c.found = true;
                    c.passed += number_near(l, "passed").unwrap_or(0);
                    c.failed += number_near(l, "failed").unwrap_or(0);
                    c.skipped += number_near(l, "skipped").unwrap_or(0);
                } else if let Some(name) = l.strip_prefix("Failed ") {
                    push_failure(&mut c, name.split(" [").next().unwrap_or(name));
                }
            }
        }
    }
    c
}

/// Stan repozytorium z `git status --porcelain=v1 --branch`.
pub fn git_status(out: &str) -> GitStatus {
    let mut s = GitStatus::default();
    for line in out.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            let (branch, rest) = head.split_once("...").unwrap_or((head, ""));
            s.branch = Some(branch.trim().to_owned());
            s.ahead = number_near(rest, "ahead").unwrap_or(0);
            s.behind = number_near(rest, "behind").unwrap_or(0);
        } else if line.len() > 3 {
            let (code, path) = line.split_at(2);
            s.files.push(GitFile {
                status: code.trim().to_owned(),
                path: path.trim().trim_matches('"').to_owned(),
            });
        }
    }
    s.clean = s.files.is_empty();
    s
}
