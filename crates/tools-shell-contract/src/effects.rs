//! Skutki polecenia, których same czasowniki nie pokazują (audyt 2026-10-14, A-07): polecenie
//! zagnieżdżone w `powershell -Command "…"` / `cmd /c "…"` (analizowane jak zwykłe, do trzech
//! poziomów), polecenie zakodowane (`-EncodedCommand`) i `Invoke-Expression` (treść nieznana —
//! najostrzej), `git clean -f`, `robocopy /MIR` i `/PURGE`, nadpisanie pliku przez `>` albo
//! `Out-File` / `Set-Content`.

/// Powłoki, które przyjmują polecenie jako argument.
const SHELLS: [&str; 3] = ["powershell", "pwsh", "cmd"];
/// Przełączniki „wykonaj to polecenie”.
const COMMAND_FLAGS: [&str; 5] = ["-command", "-c", "/c", "/k", "-commandwithargs"];
/// Przełączniki polecenia zakodowanego (Base64) — treści nie analizujemy.
const ENCODED_FLAGS: [&str; 4] = ["-encodedcommand", "-enc", "-e", "-ec"];
/// Czasowniki wykonujące tekst jako polecenie.
const EVAL_VERBS: [&str; 2] = ["invoke-expression", "iex"];
/// Czasowniki nadpisujące plik (bez `-Append`).
const OVERWRITE_VERBS: [&str; 2] = ["out-file", "set-content"];

/// Wynik skanu skutków.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Effects {
    /// Usuwa albo nadpisuje dane.
    pub deletes: bool,
    /// Cel nieustalony statycznie (treść zakodowana, `git clean`, wynik wyrażenia).
    pub opaque: bool,
    /// Treść nieznana — może też łączyć się z siecią.
    pub network: bool,
    /// Cele nadpisania (np. za `>`).
    pub paths: Vec<String>,
    /// Polecenia zagnieżdżone do osobnej analizy (`true` — uruchamia je PowerShell).
    pub inner: Vec<(String, bool)>,
}

fn lower(t: &str) -> String {
    t.to_lowercase()
}

/// Skan tokenów (małe litery, cudzysłowy zgrupowane) i surowego tekstu (przekierowania).
pub(crate) fn scan(command: &str, toks: &[String], verb: impl Fn(&str) -> String) -> Effects {
    let mut fx = Effects::default();
    let lowered: Vec<String> = toks.iter().map(|t| lower(t)).collect();
    for (i, t) in toks.iter().enumerate() {
        let v = verb(t);
        if SHELLS.contains(&v.as_str()) {
            let rest = &lowered[i + 1..];
            if let Some(at) = rest
                .iter()
                .position(|f| COMMAND_FLAGS.contains(&f.as_str()))
            {
                let inner = toks[i + 1 + at + 1..].join(" ");
                if !inner.trim().is_empty() {
                    fx.inner.push((inner, v != "cmd"));
                }
            } else if rest.iter().any(|f| ENCODED_FLAGS.contains(&f.as_str())) {
                fx.deletes = true;
                fx.opaque = true;
                fx.network = true;
            }
        }
        if EVAL_VERBS.contains(&v.as_str()) {
            fx.deletes = true;
            fx.opaque = true;
            fx.network = true;
        }
        if v == "git"
            && lowered.get(i + 1).is_some_and(|s| s == "clean")
            && lowered[i + 2..].iter().any(|f| {
                f == "--force" || (f.starts_with('-') && !f.starts_with("--") && f.contains('f'))
            })
        {
            fx.deletes = true;
            fx.opaque = true;
        }
        if v == "robocopy" && lowered.iter().any(|f| f == "/mir" || f == "/purge") {
            fx.deletes = true;
        }
        if OVERWRITE_VERBS.contains(&v.as_str()) && !lowered.iter().any(|f| f == "-append") {
            fx.deletes = true;
        }
    }
    for target in redirect_targets(command) {
        fx.deletes = true;
        fx.paths.push(target);
    }
    fx
}

/// Cele przekierowania nadpisującego (`>`, `2>`; nie `>>`, nie `>&1`, nie `nul` / `$null`).
pub(crate) fn redirect_targets(command: &str) -> Vec<String> {
    let chars: Vec<char> = command.chars().collect();
    let mut out = Vec::new();
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match (c, quote) {
            ('"' | '\'', None) => quote = Some(c),
            (q, Some(open)) if q == open => quote = None,
            ('>', None) => {
                let doubled = chars.get(i + 1) == Some(&'>') || (i > 0 && chars[i - 1] == '>');
                let to_stream = chars.get(i + 1) == Some(&'&');
                if !doubled && !to_stream {
                    let target: String = chars[i + 1..]
                        .iter()
                        .skip_while(|c| c.is_whitespace())
                        .take_while(|c| !c.is_whitespace() && !matches!(c, ';' | '|' | '&'))
                        .collect();
                    let target = target.trim_matches(['"', '\'']).to_owned();
                    let sink = matches!(lower(&target).as_str(), "nul" | "$null" | "/dev/null");
                    if !target.is_empty() && !sink {
                        out.push(target);
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}
