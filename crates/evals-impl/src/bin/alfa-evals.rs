//! `alfa-evals` — sprawdzenie zestawów ewaluacyjnych w CI i lokalnie (docs/modules/evals/SPEC.md):
//! lista zestawów, weryfikacja hashy SHA-256 (zamrożony ze zmianą → kod 1), hash manifestu.
//! Katalogów `holdout/` i `corpus/` nie czyta.

use std::io::Write as _;
use std::process::ExitCode;

use evals_contract::{SuiteCatalog, SuiteManifest, SuiteStatus};
use evals_impl::DirCatalog;

const USAGE: &str = "\
użycie:
  alfa-evals list   [KORZEŃ=evals]
  alfa-evals verify [KORZEŃ=evals]      # kod 1: zamrożony zestaw zmieniony albo zły manifest
  alfa-evals digest <manifest.json>";

fn run(args: &[String], out: &mut impl std::io::Write) -> Result<bool, String> {
    let root = args.get(1).map_or("evals", String::as_str);
    match args.first().map(String::as_str) {
        Some("list") => {
            let catalog = DirCatalog::open(root).map_err(|e| e.to_string())?;
            for s in catalog.suites() {
                let counts: Vec<String> = s
                    .case_counts
                    .iter()
                    .map(|(k, v)| format!("{}={v}", k.as_str()))
                    .collect();
                let _ = writeln!(
                    out,
                    "{}\t{}\tv{}\t{:?}\t{}\t{}",
                    s.suite,
                    s.wave,
                    s.version,
                    s.status,
                    s.manifest_path,
                    counts.join(",")
                );
            }
            Ok(catalog.problems().is_empty())
        }
        Some("verify") => {
            let catalog = DirCatalog::open(root).map_err(|e| e.to_string())?;
            let mut ok = true;
            for p in catalog.problems() {
                let _ = writeln!(out, "BŁĄD manifestu: {p}");
                ok = false;
            }
            for s in catalog.suites() {
                let report = catalog.verify(&s.suite).map_err(|e| e.to_string())?;
                let state = if report.is_intact() {
                    "zgodny"
                } else {
                    "ROZJAZD"
                };
                let _ = writeln!(
                    out,
                    "{}\t{:?}\t{state}\t{}",
                    s.suite, s.status, report.manifest_digest
                );
                for m in &report.mismatched {
                    let _ = writeln!(
                        out,
                        "  zmieniony {} (manifest {}, jest {})",
                        m.path, m.expected, m.actual
                    );
                }
                for m in &report.missing {
                    let _ = writeln!(out, "  brak {m}");
                }
                if !report.is_intact() && s.status == SuiteStatus::Frozen {
                    ok = false;
                }
            }
            Ok(ok)
        }
        Some("digest") => {
            let path = args.get(1).ok_or(USAGE)?;
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let manifest: SuiteManifest = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            manifest.validate().map_err(|e| e.to_string())?;
            let _ = writeln!(out, "{}", manifest.digest());
            Ok(true)
        }
        _ => Err(USAGE.to_owned()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdout = std::io::stdout().lock();
    match run(&args, &mut stdout) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "{e}");
            ExitCode::from(2)
        }
    }
}
