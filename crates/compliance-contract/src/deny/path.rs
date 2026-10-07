//! Normalizacja ścieżek w semantyce Windows (niezależnie od OS, na którym działa test):
//! zmienne środowiskowe, prefiksy urządzeń `\\?\`, separatory, wielkość liter, `.`/`..`,
//! końcowe kropki/spacje, strumienie ADS, udziały administracyjne i aliasy 8.3.
//!
//! To obrona leksykalna. Ostateczną kanonizację (dowiązania, junction, prawdziwe nazwy 8.3)
//! robi `platform-windows-impl` przez API systemu przed dostępem do pliku.

use std::collections::BTreeMap;

/// Zmienne środowiskowe do rozwijania ścieżek (nazwy bez rozróżniania wielkości liter).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathEnv {
    vars: BTreeMap<String, String>,
}

impl PathEnv {
    /// Puste środowisko.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dodaje zmienną (builder).
    #[must_use]
    pub fn with(mut self, name: &str, value: &str) -> Self {
        self.vars
            .insert(name.to_ascii_uppercase(), value.to_owned());
        self
    }

    /// Profil Windows: `USERPROFILE` + wyprowadzone `LOCALAPPDATA`, `APPDATA`, `SYSTEMDRIVE`.
    pub fn windows_profile(userprofile: &str) -> Self {
        Self::new().with("USERPROFILE", userprofile).derived()
    }

    /// Uzupełnia brakujące `LOCALAPPDATA`/`APPDATA`/`SYSTEMDRIVE` na podstawie `USERPROFILE`.
    #[must_use]
    pub fn derived(mut self) -> Self {
        if let Some(profile) = self.get("USERPROFILE").map(str::to_owned) {
            let base = profile.trim_end_matches(['\\', '/']).to_owned();
            self.vars
                .entry("LOCALAPPDATA".into())
                .or_insert_with(|| format!("{base}\\AppData\\Local"));
            self.vars
                .entry("APPDATA".into())
                .or_insert_with(|| format!("{base}\\AppData\\Roaming"));
            if let Some(drive) = base.get(..2).filter(|d| d.ends_with(':')) {
                self.vars
                    .entry("SYSTEMDRIVE".into())
                    .or_insert_with(|| drive.to_owned());
            }
        }
        self
    }

    /// Wartość zmiennej.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars
            .get(&name.to_ascii_uppercase())
            .map(String::as_str)
    }
}

/// Korzeń ścieżki po normalizacji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Root {
    /// Litera dysku (mała).
    Drive(char),
    /// Udział sieciowy `\\serwer\udział` (małe litery).
    Unc(String, String),
    /// Ścieżka względna (albo nierozpoznany prefiks urządzenia).
    Relative,
}

/// Ścieżka po normalizacji: korzeń + komponenty (małe litery, bez `.`/`..`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormPath {
    /// Korzeń.
    pub root: Root,
    /// Komponenty.
    pub comps: Vec<String>,
}

/// Normalizuje ścieżkę w semantyce Windows.
pub fn normalize(input: &str, env: &PathEnv) -> NormPath {
    let trimmed = input.trim().trim_matches('"');
    let unfiled = strip_file_url(trimmed);
    let expanded = expand_env(&unfiled, env).replace('/', "\\");
    let (root, rest) = split_root(&expanded, env);
    let mut comps: Vec<String> = Vec::new();
    for raw in rest.split('\\') {
        if raw == "." || raw.is_empty() {
            continue;
        }
        if raw == ".." {
            comps.pop();
            continue;
        }
        let no_stream = raw.split(':').next().unwrap_or(raw);
        let cleaned = no_stream.trim_end_matches(['.', ' ']);
        if cleaned.is_empty() {
            continue;
        }
        comps.push(cleaned.to_lowercase());
    }
    NormPath { root, comps }
}

fn strip_file_url(s: &str) -> String {
    let Some(rest) = s.get(..5).filter(|p| p.eq_ignore_ascii_case("file:")) else {
        return s.to_owned();
    };
    let rest = &s[rest.len()..];
    let decoded = percent_decode(rest);
    let body = decoded.trim_start_matches('/');
    let has_drive = body.as_bytes().get(1) == Some(&b':');
    if has_drive {
        body.to_owned()
    } else {
        format!("\\\\{body}")
    }
}

/// Dekoduje sekwencje `%XX` (używane w URL-ach `file:` i w hostach).
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| {
            std::str::from_utf8(h)
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        });
        match (bytes[i], hex) {
            (b'%', Some(value)) => {
                out.push(value);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn expand_env(s: &str, env: &PathEnv) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    if let Some(after) = rest.strip_prefix('~')
        && (after.is_empty() || after.starts_with(['\\', '/']))
        && let Some(home) = env.get("USERPROFILE")
    {
        out.push_str(home);
        rest = after;
    }
    while let Some(pos) = rest.find(['%', '$']) {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        match expand_one(tail, env) {
            Some((value, used)) => {
                out.push_str(&value);
                rest = &tail[used..];
            }
            None => {
                out.push_str(&tail[..1]);
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Rozwija jedną zmienną na początku `tail`: `%NAZWA%`, `$env:NAZWA`, `${env:NAZWA}`.
fn expand_one(tail: &str, env: &PathEnv) -> Option<(String, usize)> {
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '(' || c == ')';
    if let Some(body) = tail.strip_prefix('%') {
        let end = body.find('%')?;
        let name = &body[..end];
        if name.is_empty() || !name.chars().all(is_name) {
            return None;
        }
        return env.get(name).map(|v| (v.to_owned(), end + 2));
    }
    if let Some(body) = tail.strip_prefix("${") {
        let end = body.find('}')?;
        let name = strip_prefix_ci(&body[..end], "env:")?;
        return env.get(name).map(|v| (v.to_owned(), end + 3));
    }
    let body = strip_prefix_ci(tail.strip_prefix('$')?, "env:")?;
    let len = body.find(|c: char| !is_name(c)).unwrap_or(body.len());
    let name = &body[..len];
    if name.is_empty() {
        return None;
    }
    env.get(name).map(|v| (v.to_owned(), 1 + 4 + len))
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

fn split_root(s: &str, env: &PathEnv) -> (Root, String) {
    let mut s = s.to_owned();
    loop {
        let stripped = ["\\\\?\\", "\\\\.\\", "\\??\\"]
            .iter()
            .find_map(|p| s.strip_prefix(p).map(str::to_owned));
        match stripped {
            Some(rest) => match strip_prefix_ci(&rest, "unc\\") {
                Some(unc) => s = format!("\\\\{unc}"),
                None => s = rest,
            },
            None => break,
        }
    }
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        let drive = char::from(bytes[0]).to_ascii_lowercase();
        return (Root::Drive(drive), s[2..].to_owned());
    }
    if let Some(unc) = s.strip_prefix("\\\\") {
        let mut parts = unc.splitn(3, '\\');
        let server = parts.next().unwrap_or_default().to_lowercase();
        let share = parts.next().unwrap_or_default().to_lowercase();
        let rest = parts.next().unwrap_or_default().to_owned();
        let local = ["localhost", "127.0.0.1", "[::1]", "::1", "."].contains(&server.as_str());
        let admin = share.len() == 2 && share.ends_with('$');
        if local && admin {
            let drive = share.chars().next().unwrap_or('c');
            return (Root::Drive(drive), rest);
        }
        return (Root::Unc(server, share), rest);
    }
    if s.starts_with('\\') {
        let drive = env
            .get("SYSTEMDRIVE")
            .and_then(|d| d.chars().next())
            .unwrap_or('c')
            .to_ascii_lowercase();
        return (Root::Drive(drive), s);
    }
    (Root::Relative, s)
}

/// Czy komponent wejścia pasuje do nazwy wzorca (równość albo alias 8.3, np. `claude~1`).
pub fn comp_matches(input: &str, pattern: &str) -> bool {
    input == pattern || short_name_alias(input, pattern)
}

fn short_name_alias(comp: &str, name: &str) -> bool {
    let Some((stem, rest)) = comp.split_once('~') else {
        return false;
    };
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    let ext = &rest[digits..];
    let ext_ok = ext.is_empty() || (ext.starts_with('.') && ext.len() <= 4);
    if digits == 0 || stem.is_empty() || stem.len() > 6 || !stem.is_ascii() || !ext_ok {
        return false;
    }
    // Windows bierze pierwsze ≤ 6 znaków nazwy bez kropek i spacji (`.claude` → `CLAUDE~1`),
    // a po kilku kolizjach: 2 znaki + 4 cyfry szesnastkowe skrótu (`CL1A2B~1`).
    let squeezed: String = name.chars().filter(|c| *c != '.' && *c != ' ').collect();
    let expected_len = squeezed.chars().count().min(6);
    let plain = stem.len() == expected_len && squeezed.starts_with(stem);
    let hashed = stem.len() == 6
        && squeezed.starts_with(&stem[..2])
        && stem[2..].chars().all(|c| c.is_ascii_hexdigit());
    plain || hashed
}
