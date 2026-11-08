//! `alfa-voice-eval` — zestaw dev/test F2 (evals/F2/README.md): sprawdzenie manifestu i audio,
//! próbki syntetyczne, zamrożenie podziału test, przebieg na prawdziwym STT (`whisper-cli`),
//! gramatyce komend i automacie dialogu, ocena z progami ACCEPTANCE F2. Działa wyłącznie
//! lokalnie: nie wysyła audio ani transkrypcji nigdzie poza wskazane pliki.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use voice_cmd_contract::{Grammar, GrammarRecognizer};
use voice_dialog_contract::default_machine;
use voice_pipeline_impl::eval::{
    self, Engines, ItemResult, ManifestEntry, PrefixStt, Split, TimedStt, WhisperCli, audio,
    freeze_list, parse_manifest, parse_results, run_item, score, to_markdown, to_ndjson,
    validate_manifest, verify_frozen,
};
use voice_stt_contract::Stt;

const USAGE: &str = "\
użycie:
  alfa-voice-eval check  <manifest> [--audio-root KATALOG]
  alfa-voice-eval synth  <manifest> <katalog-wyjściowy>
  alfa-voice-eval freeze <manifest> --audio-root KATALOG
  alfa-voice-eval verify <manifest> --audio-root KATALOG --frozen PLIK
  alfa-voice-eval run    <manifest> --audio-root KATALOG --whisper-cli EXE --model PLIK
                         [--split dev|test] [--lang pl] [--out PLIK]
                         [--mode prefix|timeline] [--work KATALOG]
  alfa-voice-eval score  <manifest> <wyniki.ndjson> [--split dev|test] [--session-log PLIK]";

struct Args {
    positional: Vec<String>,
    options: BTreeMap<String, String>,
}

impl Args {
    fn parse(raw: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut positional = Vec::new();
        let mut options = BTreeMap::new();
        let mut raw = raw.peekable();
        while let Some(a) = raw.next() {
            match a.strip_prefix("--") {
                Some(name) => {
                    let value = raw
                        .next()
                        .ok_or_else(|| format!("--{name}: brak wartości"))?;
                    options.insert(name.to_owned(), value);
                }
                None => positional.push(a),
            }
        }
        Ok(Self {
            positional,
            options,
        })
    }

    fn pos(&self, i: usize, what: &str) -> Result<&str, String> {
        self.positional
            .get(i)
            .map(String::as_str)
            .ok_or_else(|| format!("brak argumentu: {what}"))
    }

    fn path(&self, name: &str) -> Result<PathBuf, String> {
        self.options
            .get(name)
            .map(PathBuf::from)
            .ok_or_else(|| format!("brak opcji --{name}"))
    }

    fn split(&self) -> Result<Option<Split>, String> {
        match self.options.get("split").map(String::as_str) {
            None => Ok(None),
            Some("dev") => Ok(Some(Split::Dev)),
            Some("test") => Ok(Some(Split::Test)),
            Some(other) => Err(format!("--split: oczekiwano dev|test, jest „{other}”")),
        }
    }
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn load_manifest(path: &str) -> Result<Vec<ManifestEntry>, String> {
    let entries = parse_manifest(&read(Path::new(path))?).map_err(|e| e.join("\n"))?;
    let errors = validate_manifest(&entries);
    if errors.is_empty() {
        Ok(entries)
    } else {
        Err(errors.join("\n"))
    }
}

fn out(text: &str) -> Result<(), String> {
    std::io::stdout()
        .write_all(text.as_bytes())
        .map_err(|e| format!("stdout: {e}"))
}

fn check(args: &Args) -> Result<(), String> {
    let entries = load_manifest(args.pos(1, "manifest")?)?;
    let mut errors = Vec::new();
    if let Some(root) = args.options.get("audio-root") {
        for e in &entries {
            if let Err(err) = audio::check_audio(e, Path::new(root)) {
                errors.push(err);
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    let count = |s| entries.iter().filter(|e| e.split == s).count();
    out(&format!(
        "manifest OK: {} pozycji (dev {}, test {})\n",
        entries.len(),
        count(Split::Dev),
        count(Split::Test)
    ))
}

fn synth(args: &Args) -> Result<(), String> {
    let entries = load_manifest(args.pos(1, "manifest")?)?;
    let dir = Path::new(args.pos(2, "katalog wyjściowy")?);
    let mut n = 0;
    for e in &entries {
        let Some(spec) = e.synth else { continue };
        let path = dir.join(&e.audio);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        std::fs::write(&path, audio::wav16(&audio::synth_audio(&spec)))
            .map_err(|err| format!("{}: {err}", path.display()))?;
        n += 1;
    }
    out(&format!(
        "wygenerowano {n} próbek syntetycznych w {}\n",
        dir.display()
    ))
}

fn freeze(args: &Args) -> Result<(), String> {
    let entries = load_manifest(args.pos(1, "manifest")?)?;
    out(&freeze_list(&entries, &args.path("audio-root")?).map_err(|e| e.join("\n"))?)
}

fn verify(args: &Args) -> Result<(), String> {
    let entries = load_manifest(args.pos(1, "manifest")?)?;
    let list = read(&args.path("frozen")?)?;
    let errors = verify_frozen(&entries, &args.path("audio-root")?, &list);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    out("zestaw test zgodny z zamrożonym\n")
}

async fn run(args: &Args) -> Result<(), String> {
    let entries = eval::select(&load_manifest(args.pos(1, "manifest")?)?, args.split()?);
    let root = args.path("audio-root")?;
    let cli = WhisperCli {
        exe: args.path("whisper-cli")?,
        model: args.path("model")?,
        lang: args
            .options
            .get("lang")
            .cloned()
            .unwrap_or_else(|| "pl".into()),
        work: args.path("work").unwrap_or_else(|_| root.join(".work")),
    };
    let timeline = match args.options.get("mode").map(String::as_str) {
        None | Some("prefix") => false,
        Some("timeline") => true,
        Some(other) => {
            return Err(format!(
                "--mode: oczekiwano prefix|timeline, jest „{other}”"
            ));
        }
    };
    let commands = GrammarRecognizer::new(Grammar::default_pl_en());
    let dialog = default_machine();
    let mut results: Vec<ItemResult> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let pcm = audio::load_item_audio(e, &root)?;
        let stt: Box<dyn Stt> = if timeline {
            Box::new(TimedStt::new(
                cli.file_words(&root.join(&e.audio), e.segment)?,
                &cli.lang,
            ))
        } else {
            Box::new(PrefixStt::new(cli.clone()))
        };
        let engines = Engines {
            stt: stt.as_ref(),
            commands: &commands,
            dialog: &dialog,
        };
        results.push(run_item(&engines, e, &pcm, i as u64 + 1).await?);
        eprintln!("[{}/{}] {}", i + 1, entries.len(), e.id);
    }
    let text = to_ndjson(&results);
    match args.options.get("out") {
        Some(path) => std::fs::write(path, text).map_err(|e| format!("{path}: {e}")),
        None => out(&text),
    }
}

/// Zwraca `Ok(false)`, gdy któreś kryterium nie spełnia progu.
fn score_cmd(args: &Args) -> Result<bool, String> {
    let entries = eval::select(&load_manifest(args.pos(1, "manifest")?)?, args.split()?);
    let results = parse_results(&read(Path::new(args.pos(2, "wyniki")?))?)?;
    let mut report = score(&entries, &results);
    if let Some(log) = args.options.get("session-log") {
        report.false_per_hour = Some(eval::false_interruptions_per_hour(&read(Path::new(log))?)?);
    }
    out(&to_markdown(&report))?;
    Ok(eval::verdicts(&report)
        .iter()
        .all(|v| v.pass != Some(false)))
}

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("alfa-voice-eval: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = match args.positional.first().map(String::as_str) {
        Some("check") => check(&args).map(|()| true),
        Some("synth") => synth(&args).map(|()| true),
        Some("freeze") => freeze(&args).map(|()| true),
        Some("verify") => verify(&args).map(|()| true),
        Some("score") => score_cmd(&args),
        Some("run") => tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|e| e.to_string())
            .and_then(|rt| rt.block_on(run(&args)))
            .map(|()| true),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("alfa-voice-eval: {e}");
            ExitCode::FAILURE
        }
    }
}
