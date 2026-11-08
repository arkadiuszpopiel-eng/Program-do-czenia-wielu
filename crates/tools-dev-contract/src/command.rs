//! Polecenia narzędzi programisty: wykrycie rodzaju projektu z plików w katalogu roboczym,
//! wiersz polecenia testów i Git dla `cmd.exe`. Argumenty od modelu przechodzą przez wąską listę
//! znaków ([`safe_token`], [`safe_path`]) — żadnych metaznaków powłoki, cudzysłowów ani opcji
//! zaczynających się od `-` (wstrzyknięcie przełącznika do `git`).

use crate::{GitAction, GitArgs, Runner};

/// Najdłuższy argument (ścieżka, rewizja, filtr).
pub const MAX_ARG: usize = 200;

/// Rewizja / filtr / gałąź: litery, cyfry, `._/~:@-` (bez `^` — znak ucieczki `cmd.exe`), bez `-` na
/// początku i bez `..`.
pub fn safe_token(s: &str) -> Result<&str, String> {
    let t = s.trim();
    let ok = !t.is_empty()
        && t.len() <= MAX_ARG
        && !t.starts_with('-')
        && !t.contains("..")
        && t.chars()
            .all(|c| c.is_alphanumeric() || "._/~:@-".contains(c));
    if ok {
        Ok(t)
    } else {
        Err(format!(
            "`{s}` — dozwolone litery, cyfry i znaki ._/~:@- (bez `-` na początku i bez `..`)"
        ))
    }
}

/// Ścieżka względna w katalogu roboczym: bez dysku, bez `..`, bez metaznaków; spacje dozwolone
/// (ścieżka trafia w cudzysłowy).
pub fn safe_path(s: &str) -> Result<String, String> {
    let t = s.trim().replace('\\', "/");
    let ok = !t.is_empty()
        && t.len() <= MAX_ARG
        && !t.starts_with('/')
        && !t.starts_with('-')
        && !t.contains(':')
        && !t.split('/').any(|seg| seg == "..")
        && t.chars()
            .all(|c| c.is_alphanumeric() || " ._/-+=,".contains(c));
    if ok {
        Ok(t)
    } else {
        Err(format!(
            "`{s}` — podaj ścieżkę względną w katalogu roboczym, bez `..` i znaków specjalnych"
        ))
    }
}

/// Rodzaj projektu z nazw plików w katalogu roboczym i treści `package.json`.
pub fn detect_runner(files: &[String], package_json: Option<&str>) -> Option<Runner> {
    let has = |name: &str| files.iter().any(|f| f.eq_ignore_ascii_case(name));
    let ends = |ext: &str| files.iter().any(|f| f.to_ascii_lowercase().ends_with(ext));
    if has("Cargo.toml") {
        return Some(Runner::Cargo);
    }
    if let Some(pkg) = package_json {
        let has_test = serde_json::from_str::<serde_json::Value>(pkg)
            .ok()
            .and_then(|v| v["scripts"]["test"].as_str().map(str::to_owned))
            .is_some_and(|t| !t.contains("no test specified"));
        if has_test {
            return Some(if has("pnpm-lock.yaml") {
                Runner::Pnpm
            } else if has("yarn.lock") {
                Runner::Yarn
            } else {
                Runner::Npm
            });
        }
    }
    if has("go.mod") {
        return Some(Runner::Go);
    }
    if ends(".sln") || ends(".csproj") || ends(".fsproj") {
        return Some(Runner::Dotnet);
    }
    if has("pytest.ini")
        || has("pyproject.toml")
        || has("setup.cfg")
        || has("tox.ini")
        || has("tests")
    {
        return Some(Runner::Pytest);
    }
    None
}

/// Wiersz polecenia testów (`filter` już sprawdzony).
pub fn test_command(runner: Runner, filter: Option<&str>) -> String {
    let f = filter.map(|f| format!(" {f}")).unwrap_or_default();
    match runner {
        Runner::Cargo => format!("cargo test --color never{f}"),
        Runner::Npm => match filter {
            Some(x) => format!("npm test --silent -- {x}"),
            None => "npm test --silent".into(),
        },
        Runner::Pnpm => match filter {
            Some(x) => format!("pnpm test -- {x}"),
            None => "pnpm test".into(),
        },
        Runner::Yarn => format!("yarn test{f}"),
        Runner::Pytest => match filter {
            Some(x) => format!("python -m pytest -q -k {x}"),
            None => "python -m pytest -q".into(),
        },
        Runner::Go => match filter {
            Some(x) => format!("go test ./... -run {x}"),
            None => "go test ./...".into(),
        },
        Runner::Dotnet => match filter {
            Some(x) => format!("dotnet test --nologo --filter {x}"),
            None => "dotnet test --nologo".into(),
        },
    }
}

/// Wiersz polecenia Git (tylko odczyt; `-c core.quotepath=off` — polskie nazwy plików czytelne).
pub fn git_command(a: &GitArgs) -> Result<String, String> {
    let base = "git -c core.quotepath=off --no-pager";
    let path = match &a.path {
        Some(p) => format!(" -- \"{}\"", safe_path(p)?),
        None => String::new(),
    };
    let limit = a.limit.unwrap_or(20).clamp(1, 200);
    Ok(match a.action {
        GitAction::Status => format!("{base} status --porcelain=v1 --branch{path}"),
        GitAction::Diff => {
            let staged = if a.staged.unwrap_or(false) {
                " --staged"
            } else {
                ""
            };
            let rev = match &a.rev {
                Some(r) => format!(" {}", safe_token(r)?),
                None => String::new(),
            };
            format!("{base} diff --stat --patch --no-color{staged}{rev}{path}")
        }
        GitAction::Log => {
            let rev = match &a.rev {
                Some(r) => format!(" {}", safe_token(r)?),
                None => String::new(),
            };
            format!(
                "{base} log -n {limit} --date=short --pretty=format:%h%x09%ad%x09%an%x09%s{rev}{path}"
            )
        }
        GitAction::Branches => format!("{base} branch --list -vv --no-color"),
        GitAction::Show => {
            let rev = safe_token(a.rev.as_deref().unwrap_or("HEAD"))?;
            format!("{base} show --stat --patch --no-color {rev}{path}")
        }
    })
}
