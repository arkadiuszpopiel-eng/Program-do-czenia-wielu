//! Wspólne pomocniki testów `plugin-runtime-impl`: wtyczki pisane w WAT (komponenty
//! kompilowane w teście — żadnych binariów w repo), Broker z prawdziwym silnikiem (profil
//! `C:\Users\user`), host w pamięci, magazyn w pamięci, magistrala-atrapa.

use std::sync::Arc;

use core_bus_fake::FakeBus;
use plugin_runtime_contract::{
    MemHost, MemPluginStore, PluginApproval, PluginLimits, PluginManifest, PluginRecord,
    PluginSource, PluginStore, PluginToolDecl, Plugins, samples, sha256_hex,
};
use plugin_runtime_impl::{PluginDeps, PluginRuntime, RuntimeConfig};
use safety_broker_contract::{Capability, Holder, HostPattern, PathScope};
use safety_broker_fake::FakeBroker;
use tools_common_contract::{Tool, ToolCtx, ToolOutcome};

/// Komponent: import hosta, moduł pamięci z alokatorem (bump), moduł główny z `invoke`.
const TEMPLATE: &str = r#"
(component
  (import "alfa:plugin/host@0.1.0" (instance $host
    (export "call" (func (param "op" string) (param "args" string) (result (result string (error string)))))
  ))
  (core module $Mem
    (memory (export "memory") PAGES)
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

/// Napis WAT z dowolnych bajtów (`\hh`).
pub fn wat_str(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("\\{b:02x}")).collect()
}

/// Komponent z ciałem modułu głównego (pamięć początkowa: `pages` stron).
pub fn component_pages(body: &str, pages: u32) -> Vec<u8> {
    let text = TEMPLATE
        .replace("PAGES", &pages.to_string())
        .replace("BODY", body);
    wat::parse_str(text).unwrap()
}

/// Komponent z ciałem modułu głównego (2 strony pamięci: dane do 128 KiB, potem alokator).
pub fn component(body: &str) -> Vec<u8> {
    component_pages(body, 2)
}

/// Dowolny tekst WAT → bajty.
pub fn wat(text: &str) -> Vec<u8> {
    wat::parse_str(text).unwrap()
}

/// Ciało: zwraca stały wynik (`disc` 0 = ok, 1 = err).
pub fn const_body(disc: u8, out: &[u8]) -> String {
    format!(
        r#"(data (i32.const 256) "{}")
    (func (export "invoke") (param i32 i32 i32 i32) (result i32)
      (call $ret (i32.const {disc}) (i32.const 256) (i32.const {})))"#,
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

/// Ciało licznika słów: ciągi znaków słownych (ASCII alnum, bajty ≥ 0x80) w wejściu JSON
/// minus klucz `text` → `{"words":N}`.
pub const WORD_COUNT: &str = r#"
    (data (i32.const 64) "{\"words\":")
    (func $is_word (param $c i32) (result i32)
      (i32.or (i32.ge_u (local.get $c) (i32.const 128))
        (i32.or (i32.and (i32.ge_u (local.get $c) (i32.const 48)) (i32.le_u (local.get $c) (i32.const 57)))
          (i32.or (i32.and (i32.ge_u (local.get $c) (i32.const 65)) (i32.le_u (local.get $c) (i32.const 90)))
                  (i32.and (i32.ge_u (local.get $c) (i32.const 97)) (i32.le_u (local.get $c) (i32.const 122)))))))
    (func (export "invoke") (param $tp i32) (param $tl i32) (param $ip i32) (param $il i32) (result i32)
      (local $i i32) (local $in i32) (local $n i32) (local $buf i32) (local $tmp i32) (local $digits i32)
      (block $done
        (loop $l
          (br_if $done (i32.ge_u (local.get $i) (local.get $il)))
          (if (call $is_word (i32.load8_u (i32.add (local.get $ip) (local.get $i))))
            (then
              (if (i32.eqz (local.get $in)) (then (local.set $n (i32.add (local.get $n) (i32.const 1)))))
              (local.set $in (i32.const 1)))
            (else (local.set $in (i32.const 0))))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (br $l)))
      (if (i32.gt_u (local.get $n) (i32.const 0)) (then (local.set $n (i32.sub (local.get $n) (i32.const 1)))))
      (local.set $buf (call $realloc (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 32)))
      (memory.copy (local.get $buf) (i32.const 64) (i32.const 9))
      (local.set $tmp (local.get $n))
      (local.set $digits (i32.const 1))
      (block $d (loop $dl
        (br_if $d (i32.lt_u (local.get $tmp) (i32.const 10)))
        (local.set $tmp (i32.div_u (local.get $tmp) (i32.const 10)))
        (local.set $digits (i32.add (local.get $digits) (i32.const 1)))
        (br $dl)))
      (local.set $i (local.get $digits))
      (local.set $tmp (local.get $n))
      (block $w (loop $wl
        (br_if $w (i32.eqz (local.get $i)))
        (local.set $i (i32.sub (local.get $i) (i32.const 1)))
        (i32.store8 (i32.add (i32.add (local.get $buf) (i32.const 9)) (local.get $i))
          (i32.add (i32.const 48) (i32.rem_u (local.get $tmp) (i32.const 10))))
        (local.set $tmp (i32.div_u (local.get $tmp) (i32.const 10)))
        (br $wl)))
      (i32.store8 (i32.add (i32.add (local.get $buf) (i32.const 9)) (local.get $digits)) (i32.const 125))
      (call $ret (i32.const 0) (local.get $buf) (i32.add (local.get $digits) (i32.const 10))))
"#;

/// Moduł licznika słów (`n` zmienia bajty — inna treść, to samo zachowanie).
pub fn word_count_wasm(n: u8) -> Vec<u8> {
    component(&format!(
        "{WORD_COUNT}\n(data (i32.const 2000) \"{}\")",
        wat_str(&[n])
    ))
}

/// Narzędzie ogólne „sonda” (wynik: dowolny obiekt).
pub fn probe_tool() -> PluginToolDecl {
    PluginToolDecl {
        name: "probe".into(),
        title: "Sonda".into(),
        description: "Narzędzie testowe wtyczki: wykonuje jedną operację i zwraca wynik.".into(),
        input_schema: serde_json::json!({"type": "object", "properties": {}, "additionalProperties": false}),
        output_schema: serde_json::json!({"type": "object"}),
        mutating: false,
    }
}

/// Manifest z narzędziem „sonda”, zdolnościami i limitami.
pub fn probe_manifest(
    id: &str,
    wasm: &[u8],
    caps: Vec<Capability>,
    limits: PluginLimits,
) -> PluginManifest {
    let mut m = samples::manifest(id, "1.0.0", wasm);
    m.tools = vec![probe_tool()];
    m.capabilities = caps;
    m.limits = limits;
    m
}

/// `fs.read` w poddrzewie.
pub fn fs_read_tree(path: &str) -> Capability {
    Capability::FsRead(PathScope::tree(path, &Default::default()).unwrap())
}

/// `fs.write` w poddrzewie.
pub fn fs_write_tree(path: &str) -> Capability {
    Capability::FsWrite(PathScope::tree(path, &Default::default()).unwrap())
}

/// `net.egress(host)`.
pub fn egress(host: &str) -> Capability {
    Capability::NetEgress(HostPattern::parse(host).unwrap())
}

/// Środowisko testu.
pub struct Harness {
    pub broker: Arc<FakeBroker>,
    pub host: Arc<MemHost>,
    pub store: Arc<MemPluginStore>,
    pub bus: Arc<FakeBus>,
    pub runtime: PluginRuntime,
}

/// Pliki hosta.
pub const FILES: [(&str, &str); 3] = [
    (r"C:\Users\user\Documents\notes\a.txt", "notatka A"),
    (r"C:\Users\user\.ssh\id_rsa", "SEKRET-KLUCZ"),
    (r"C:\Users\user\.claude\.credentials.json", "TOKEN-CLI"),
];

/// Środowisko nad wspólnym magazynem (ponowne uruchomienie = nowy runtime, ten sam magazyn).
pub fn harness_with(store: Arc<MemPluginStore>, host: Arc<MemHost>) -> Harness {
    let broker = Arc::new(FakeBroker::new().unwrap());
    let bus = Arc::new(FakeBus::default());
    let runtime = PluginRuntime::new(PluginDeps {
        broker: broker.clone(),
        host: host.clone(),
        store: store.clone(),
        bus: Some(bus.clone()),
        config: RuntimeConfig::default(),
    })
    .unwrap();
    Harness {
        broker,
        host,
        store,
        bus,
        runtime,
    }
}

/// Świeże środowisko.
pub fn harness() -> Harness {
    harness_with(
        Arc::new(MemPluginStore::default()),
        Arc::new(MemHost::with_files(&FILES)),
    )
}

impl Harness {
    /// Propozycja + zatwierdzenie (UI, przejrzany hash).
    pub async fn install(&self, manifest: PluginManifest, wasm: Vec<u8>) -> PluginRecord {
        let r = self
            .runtime
            .propose(manifest, wasm, PluginSource::User)
            .await
            .unwrap();
        self.runtime
            .approve(
                &r.manifest.id,
                &r.manifest.version,
                PluginApproval::ui(&r.review_hash),
            )
            .await
            .unwrap()
    }

    /// Narzędzie po nazwie dla modelu.
    pub fn tool(&self, name: &str) -> Arc<dyn Tool> {
        self.runtime
            .tools()
            .into_iter()
            .find(|t| t.manifest().name == name)
            .unwrap()
    }

    /// Instaluje „sondę” i wywołuje ją raz z argumentami.
    pub async fn probe(
        &self,
        id: &str,
        wasm: Vec<u8>,
        caps: Vec<Capability>,
        limits: PluginLimits,
        args: serde_json::Value,
    ) -> ToolOutcome {
        self.install(probe_manifest(id, &wasm, caps, limits), wasm)
            .await;
        self.tool("plugin_probe").call(args, &ctx()).await
    }

    /// Nazwy opublikowanych zdarzeń.
    pub fn event_names(&self) -> Vec<String> {
        self.bus
            .recorded()
            .iter()
            .map(|e| e.kind.as_str().to_owned())
            .collect()
    }
}

/// Kontekst agentki Delty w sesji `s1` (krótkie czekanie na zatwierdzenie).
pub fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = std::time::Duration::from_millis(300);
    c
}

/// Bajty modułu w magazynie (podmiana = atak na łańcuch dostaw).
pub fn tamper(store: &MemPluginStore, sha: &str, bytes: &[u8]) {
    store.put_wasm(sha, bytes).unwrap();
}

/// SHA-256 (hex).
pub fn sha(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}
