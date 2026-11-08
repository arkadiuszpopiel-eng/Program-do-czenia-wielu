//! Pomocniki testów `app-plugins`: wtyczki w WAT (komponenty kompilowane w teście — bez binariów
//! w repo; ten sam szablon co w `plugin-runtime-impl`), Broker na prawdziwym silniku (profil
//! `C:\Users\user`), pliki i dziennik cofania w pamięci, sieć-atrapa, magazyn w katalogu tymczasowym.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_plugins::{HttpsGet, HttpsResponse, PluginsApp, PluginsDeps};
use async_trait::async_trait;
use base64::Engine as _;
use compliance_contract::{DenyLists, PathEnv};
use core_bus_fake::FakeBus;
use platform_contract::FsPort;
use platform_fake::FakeFs;
use safety_broker_contract::{ApprovalDecision, Holder};
use safety_broker_fake::FakeBroker;
use tools_common_contract::ToolCtx;
use undo_journal_contract::UndoLimits;
use undo_journal_fake::FakeUndoJournal;

/// Katalog notatek właściciela (zakres zdolności wtyczek w testach).
pub const NOTES: &str = r"C:\Users\user\Documents\notes";
/// Plik w zakresie.
pub const NOTE_A: &str = r"C:\Users\user\Documents\notes\a.txt";

const TEMPLATE: &str = r#"
(component
  (import "alfa:plugin/host@0.1.0" (instance $host
    (export "call" (func (param "op" string) (param "args" string) (result (result string (error string)))))
  ))
  (core module $Mem
    (memory (export "memory") 2)
    (global $bump (mut i32) (i32.const 131072))
    (func (export "realloc") (param $old i32) (param $old_size i32) (param $align i32) (param $new_size i32) (result i32)
      (local $p i32)
      (local.set $p (i32.and (i32.add (global.get $bump) (i32.sub (local.get $align) (i32.const 1)))
                             (i32.sub (i32.const 0) (local.get $align))))
      (global.set $bump (i32.add (local.get $p) (local.get $new_size)))
      (block $ok
        (br_if $ok (i32.le_u (global.get $bump) (i32.mul (memory.size) (i32.const 65536))))
        (br_if $ok (i32.ne (memory.grow (i32.add (i32.shr_u (i32.sub (global.get $bump) (i32.mul (memory.size) (i32.const 65536))) (i32.const 16)) (i32.const 1))) (i32.const -1)))
        unreachable)
      (local.get $p))
  )
  (core instance $mem (instantiate $Mem))
  (alias core export $mem "memory" (core memory $memory))
  (alias core export $mem "realloc" (core func $realloc))
  (alias export $host "call" (func $host_call))
  (core func $call_lowered (canon lower (func $host_call) (memory $memory) (realloc $realloc)))
  (core module $Main
    (import "env" "memory" (memory 1))
    (import "env" "realloc" (func $realloc (param i32 i32 i32 i32) (result i32)))
    (import "host" "call" (func $call (param i32 i32 i32 i32 i32)))
    (func $ret (param $disc i32) (param $p i32) (param $l i32) (result i32)
      (local $r i32)
      (local.set $r (call $realloc (i32.const 0) (i32.const 0) (i32.const 4) (i32.const 12)))
      (i32.store8 (local.get $r) (local.get $disc))
      (i32.store offset=4 (local.get $r) (local.get $p))
      (i32.store offset=8 (local.get $r) (local.get $l))
      (local.get $r))
    BODY
  )
  (core instance $main (instantiate $Main
    (with "env" (instance (export "memory" (memory $memory)) (export "realloc" (func $realloc))))
    (with "host" (instance (export "call" (func $call_lowered))))
  ))
  (func $invoke (param "tool" string) (param "input" string) (result (result string (error string)))
    (canon lift (core func $main "invoke") (memory $memory) (realloc $realloc)))
  (export "invoke" (func $invoke))
)
"#;

fn wat_str(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("\\{b:02x}")).collect()
}

/// Komponent z ciałem modułu głównego.
pub fn component(body: &str) -> Vec<u8> {
    wat::parse_str(TEMPLATE.replace("BODY", body)).unwrap()
}

/// Ciało: stały wynik `out`.
pub fn const_body(out: &[u8]) -> String {
    format!(
        r#"(data (i32.const 256) "{}")
    (func (export "invoke") (param i32 i32 i32 i32) (result i32)
      (call $ret (i32.const 0) (i32.const 256) (i32.const {})))"#,
        wat_str(out),
        out.len()
    )
}

/// Ciało: operacja hosta `op` z argumentami = wejście narzędzia; wynik hosta = wynik wtyczki.
pub fn proxy_body(op: &str) -> String {
    format!(
        r#"(data (i32.const 256) "{}")
    (func (export "invoke") (param i32 i32) (param $ip i32) (param $il i32) (result i32)
      (local $r i32)
      (local.set $r (call $realloc (i32.const 0) (i32.const 0) (i32.const 4) (i32.const 12)))
      (call $call (i32.const 256) (i32.const {}) (local.get $ip) (local.get $il) (local.get $r))
      (local.get $r))"#,
        wat_str(op.as_bytes()),
        op.len()
    )
}

/// Ciało: nieskończona pętla (paliwo się kończy → `plugin.trapped`).
pub const SPIN: &str = r#"(func (export "invoke") (param i32 i32 i32 i32) (result i32)
      (loop $l (br $l))
      unreachable)"#;

/// Bajty → base64 (jak wysyła UI).
pub fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Sieć-atrapa: odpowiedzi po adresie, zapis wywołań.
#[derive(Default)]
pub struct FakeNet {
    pub calls: Mutex<Vec<String>>,
}

#[async_trait]
impl HttpsGet for FakeNet {
    async fn get(&self, url: &str, _max: usize) -> Result<HttpsResponse, String> {
        self.calls.lock().unwrap().push(url.to_owned());
        Ok(HttpsResponse {
            status: 200,
            content_type: Some("text/plain".into()),
            body: format!("treść {url}"),
        })
    }
}

/// Środowisko.
pub struct Env {
    pub broker: Arc<FakeBroker>,
    pub fs: Arc<FakeFs>,
    pub journal: Arc<FakeUndoJournal>,
    pub net: Arc<FakeNet>,
    pub bus: Arc<FakeBus>,
    pub dir: tempfile::TempDir,
}

impl Env {
    /// Świeże środowisko z jedną notatką.
    pub fn new() -> Self {
        let fs = Arc::new(FakeFs::with_files([(
            PathBuf::from(NOTE_A),
            b"notatka A".to_vec(),
        )]));
        let port: Arc<dyn FsPort> = fs.clone();
        let journal =
            FakeUndoJournal::new(port, UndoLimits::default(), Arc::new(|| 1_000_000u64)).unwrap();
        Self {
            broker: Arc::new(FakeBroker::new().unwrap()),
            fs,
            journal: Arc::new(journal),
            net: Arc::new(FakeNet::default()),
            bus: Arc::new(FakeBus::default()),
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// Zależności wtyczek.
    pub fn deps(&self) -> PluginsDeps {
        PluginsDeps {
            broker: self.broker.clone(),
            fs: self.fs.clone(),
            journal: self.journal.clone(),
            env: PathEnv::windows_profile(r"C:\Users\user"),
            deny: DenyLists::baseline(),
            bus: Some(self.bus.clone()),
            dir: self.dir.path().join("plugins"),
            net: Some(self.net.clone()),
        }
    }

    /// Wtyczki nad środowiskiem.
    pub fn app(&self) -> PluginsApp {
        PluginsApp::new(self.deps())
    }
}

/// Kontekst agentki Delty w sesji `s1`.
pub fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_secs(5);
    c
}

/// Zatwierdza najbliższą prośbę Brokera (jak okno Broker-UI).
pub async fn approver(broker: Arc<FakeBroker>, decision: ApprovalDecision) {
    for _ in 0..300 {
        if broker.auto_approve(decision.clone()).await > 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
