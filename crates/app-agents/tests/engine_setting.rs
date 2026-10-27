//! Silnik gier w aplikacji: ustawienie `engine.nextgo.root` (zapis jak z Ustawień, warstwa
//! maszyny) → katalog śledzony przez `watch_root` → narzędzia `engine_*` z `engine_tools` →
//! wywołanie przez Brokera. Bez `ALFA_NEXTGO_ROOT` — katalog bez zbudowanego silnika; z nim —
//! prawdziwy silnik (stan, dokumentacja, start i zatrzymanie serwera).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use app_agents::engine::{EngineRoot, engine_tools, watch_root};
use compliance_contract::{DenyLists, PathEnv};
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope};
use core_config_fake::FakeConfigStore;
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, ToolOutcome, ToolStatus, Toolset};
use tools_engine_contract::{ROOT_SETTING, manifests};
use tools_engine_impl::EngineTools;
use watchdog_contract::ManualClock;

struct H {
    config: Arc<FakeConfigStore>,
    root: EngineRoot,
    tools: EngineTools,
    dir: tempfile::TempDir,
}

fn harness() -> H {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_string_lossy().into_owned();
    let env = PathEnv::windows_profile(&home);
    let policy = KernelPolicy::baseline(&home, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    for m in manifests() {
        broker.script(&m.id, ScriptedDecision::Allow);
    }
    let config = Arc::new(FakeConfigStore::new(MachineId::new("pc")));
    let root = watch_root(config.clone());
    let tools = engine_tools(
        root.clone(),
        &dir.path().join("lokalne"),
        broker,
        env,
        DenyLists::baseline(),
    );
    H {
        config,
        root,
        tools,
        dir,
    }
}

impl H {
    /// Zapis jak z Ustawień: warstwa maszyny (zakres ustawienia `machine`), źródło — użytkownik.
    async fn set(&self, value: Option<&str>) {
        let key = ConfigKey::new(ROOT_SETTING).unwrap();
        self.config
            .set(
                &key,
                value.map(|v| json!(v)),
                &Scope::Global,
                &ConfigLayer::Machine(MachineId::new("pc")),
                Origin::User,
            )
            .await
            .unwrap();
    }

    async fn root_becomes(&self, want: Option<&Path>) {
        for _ in 0..200 {
            if self.root.read().unwrap().as_deref() == want {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!(
            "katalog silnika {:?}, oczekiwano {want:?}",
            self.root.read().unwrap()
        );
    }

    async fn call(&self, name: &str, args: serde_json::Value) -> ToolOutcome {
        let tool: Arc<dyn Tool> = self
            .tools
            .tools()
            .into_iter()
            .find(|t| t.manifest().name == name)
            .unwrap();
        let mut ctx = ToolCtx::new(Holder::agent("s1", "delta"))
            .with_workdir(&self.dir.path().to_string_lossy());
        ctx.approval_timeout = Duration::from_millis(200);
        tool.call(args, &ctx).await
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn setting_switches_engine_tools_on_and_off() {
    let h = harness();
    h.root_becomes(None).await;
    let off = h.call("engine_status", json!({})).await;
    assert!(
        off.text
            .to_lowercase()
            .contains("nie wskazano katalogu silnika"),
        "{}",
        off.text
    );
    assert!(
        off.text.contains("Zaawansowane"),
        "wskazuje, gdzie ustawić: {}",
        off.text
    );

    // Katalog bez zbudowanego silnika: narzędzia mówią, czego brakuje, nic nie uruchamiają.
    let empty = h.dir.path().join("silnik");
    std::fs::create_dir_all(&empty).unwrap();
    h.set(Some(&empty.to_string_lossy())).await;
    h.root_becomes(Some(&empty)).await;
    let status = h.call("engine_status", json!({})).await;
    assert!(
        status.text.contains("nie jest zbudowany"),
        "{}",
        status.text
    );
    let start = h.call("engine_start", json!({})).await;
    assert_ne!(start.status, ToolStatus::Ok, "{}", start.text);

    // Wyczyszczenie pola w Ustawieniach wyłącza narzędzia.
    h.set(None).await;
    h.root_becomes(None).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn setting_points_tools_at_the_real_engine() {
    let Some(real) = std::env::var_os("ALFA_NEXTGO_ROOT").map(PathBuf::from) else {
        eprintln!("pominięte: ustaw ALFA_NEXTGO_ROOT na katalog zbudowanego silnika");
        return;
    };
    let h = harness();
    // Wartość jak wpisana ręcznie: w cudzysłowie, ze spacjami w ścieżce.
    h.set(Some(&format!("\"{}\"", real.display()))).await;
    h.root_becomes(Some(&real)).await;
    let status = h.call("engine_status", json!({})).await;
    assert!(status.text.contains("gotowy"), "{}", status.text);
    let docs = h.call("engine_docs", json!({})).await;
    assert!(docs.text.contains("README.md"), "{}", docs.text);
    let start = h.call("engine_start", json!({"scene": "demo"})).await;
    assert_eq!(start.status, ToolStatus::Ok, "{}", start.text);
    let scene = h.call("engine_inspect", json!({"what": "scene"})).await;
    assert_eq!(scene.status, ToolStatus::Ok, "{}", scene.text);
    let stop = h.call("engine_stop", json!({})).await;
    assert_eq!(stop.status, ToolStatus::Ok, "{}", stop.text);
    assert!(!real.join("captures").exists() && !real.join("previews").exists());
}
