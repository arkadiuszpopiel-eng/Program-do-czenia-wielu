//! Wtyczki Wasm przez komendy `AppCore` (F8-05): strona „Wtyczki” w drzewie ustawień, propozycja →
//! zatwierdzenie hashem przejrzanej wersji → Delta (Wykonawczyni, grupa `plugin`) od razu widzi
//! narzędzie `plugin_*` i dostaje jego niezaufany wynik; wyłączenie zabiera narzędzie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::ErrorCode;
use app_core::dto::PluginStateView;
use base64::Engine as _;
use common::agents::*;
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;

/// Komponent zwracający zawsze `{"words":2}` (import hosta, eksport `invoke`).
const PLUGIN: &str = r#"
(component
  (import "alfa:plugin/host@0.1.0" (instance $host
    (export "call" (func (param "op" string) (param "args" string) (result (result string (error string)))))
  ))
  (core module $Mem
    (memory (export "memory") 2)
    (global $bump (mut i32) (i32.const 4096))
    (func (export "realloc") (param i32 i32) (param $align i32) (param $size i32) (result i32)
      (local $p i32)
      (local.set $p (i32.and (i32.add (global.get $bump) (i32.sub (local.get $align) (i32.const 1)))
                             (i32.sub (i32.const 0) (local.get $align))))
      (global.set $bump (i32.add (local.get $p) (local.get $size)))
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
    (data (i32.const 256) "{\"words\":2}")
    (func (export "invoke") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (local.set $r (call $realloc (i32.const 0) (i32.const 0) (i32.const 4) (i32.const 12)))
      (i32.store8 (local.get $r) (i32.const 0))
      (i32.store offset=4 (local.get $r) (i32.const 256))
      (i32.store offset=8 (local.get $r) (i32.const 11))
      (local.get $r))
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn approved_plugin_tool_reaches_the_agent_and_disable_removes_it() {
    let mut a = agents().await;
    let core = a.h.core.clone();
    let pages = core.settings_schema().await.unwrap();
    assert!(pages.iter().any(|p| p.id == "plugins"), "strona „Wtyczki”");
    let empty = core.plugins_list().await.unwrap();
    assert!(empty.available && empty.plugins.is_empty(), "{empty:?}");

    let wasm = wat::parse_str(PLUGIN).unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&wasm);
    let inspection = core.plugins_inspect(b64.clone()).await.unwrap();
    assert!(inspection.ok, "{inspection:?}");
    let manifest = plugin_runtime_contract::samples::manifest("licznik", "1.0.0", &wasm);
    let card = core
        .plugins_propose(serde_json::to_value(manifest).unwrap(), b64)
        .await
        .unwrap();
    assert_eq!(card.state, PluginStateView::Proposed);
    let refused = core
        .plugins_approve("licznik".into(), "1.0.0".into(), "0".repeat(64))
        .await
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    let installed = core
        .plugins_approve("licznik".into(), "1.0.0".into(), card.review_hash.clone())
        .await
        .unwrap();
    assert_eq!(installed.state, PluginStateView::Installed);

    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "c1",
        "plugin_word_count",
        &json!({ "text": "ala ma" }),
    ));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Policzyłam: 2."]));
    run_turn(&mut a, "Delta, policz słowa wtyczką.").await;
    let requests = a.h.provider.requests();
    let offered = requests
        .iter()
        .any(|r| r.tools.iter().any(|t| t.name == "plugin_word_count"));
    assert!(offered, "Wykonawczyni widzi narzędzie wtyczki");
    let results = requests
        .iter()
        .map(tool_results)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(results.contains("words"), "{results}");

    let off = core.plugins_disable("licznik".into()).await.unwrap();
    assert_eq!(off.state, PluginStateView::Disabled);
    a.h.provider.push(Script::text(FAKE_MODEL, &["Gotowe."]));
    let before = a.h.provider.requests().len();
    run_turn(&mut a, "Delta, jeszcze raz.").await;
    let after = a.h.provider.requests();
    assert!(
        after[before..]
            .iter()
            .all(|r| r.tools.iter().all(|t| t.name != "plugin_word_count")),
        "wyłączona wtyczka znika z rejestru"
    );
    let view = core.plugins_remove("licznik".into()).await.unwrap();
    assert!(view.plugins.is_empty());
}
