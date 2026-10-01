//! Pomocniki testów runtime: skryptowany model (providers-fake), skryptowane narzędzia
//! (tools-fs-fake), magazyn checkpointów w pamięci, magistrala-atrapa.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use agent_runtime_contract::{MemCheckpointStore, RunSpec, contract_tests::sample_spec};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
use core_bus_fake::FakeBus;
use providers_contract::{ProviderEvent, StopReason, ToolArguments, Usage};
use providers_fake::{FAKE_MODEL, FakeProvider, Script, Step};
use tools_common_contract::{Tool, Toolset};
use tools_fs_fake::FakeFsTools;

pub const TOOLS: [&str; 6] = [
    "fs_list",
    "fs_read",
    "fs_write",
    "fs_move",
    "fs_delete",
    "fs_mkdir",
];

pub struct World {
    pub provider: FakeProvider,
    pub fs: FakeFsTools,
    pub store: MemCheckpointStore,
    pub bus: Arc<FakeBus>,
    pub runtime: Runtime,
}

pub fn world_with(config: RuntimeConfig, extra: Vec<Arc<dyn Tool>>) -> World {
    let provider = FakeProvider::new("fake");
    let fs = FakeFsTools::new();
    let store = MemCheckpointStore::default();
    let bus = Arc::new(FakeBus::default());
    let mut tools = fs.tools();
    tools.extend(extra);
    let runtime = Runtime::new(RuntimeDeps {
        provider: Arc::new(provider.clone()),
        tools,
        checkpoints: Arc::new(store.clone()),
        bus: Some(bus.clone()),
        config,
    });
    World {
        provider,
        fs,
        store,
        bus,
        runtime,
    }
}

pub fn world() -> World {
    world_with(RuntimeConfig::default(), Vec::new())
}

pub fn spec() -> RunSpec {
    let mut s = sample_spec(FAKE_MODEL, &TOOLS);
    s.goal = "Uporządkuj folder /Users/ala/Documents".into();
    s
}

fn usage(input: u64, output: u64) -> Step {
    Step::Emit(ProviderEvent::Usage(Usage {
        input_tokens: input,
        output_tokens: output,
        ..Usage::default()
    }))
}

/// Tura: tekst (opcjonalnie) + wywołanie narzędzia.
pub fn say_and_call(text: &str, id: &str, name: &str, args: serde_json::Value) -> Script {
    let mut steps = vec![Step::Emit(ProviderEvent::Started {
        model: FAKE_MODEL.into(),
        response_id: None,
    })];
    let mut index = 0;
    if !text.is_empty() {
        steps.push(Step::Emit(ProviderEvent::TextDelta {
            index: 0,
            text: text.into(),
        }));
        index = 1;
    }
    steps.push(Step::Emit(ProviderEvent::ToolCallStart {
        index,
        id: id.into(),
        name: name.into(),
    }));
    steps.push(Step::Emit(ProviderEvent::ToolCallEnd {
        index,
        id: id.into(),
        arguments: ToolArguments::Parsed { value: args },
    }));
    steps.push(usage(10, 5));
    steps.push(Step::Emit(ProviderEvent::stop(StopReason::ToolUse)));
    Script::new(steps)
}

/// Tura: samo wywołanie narzędzia.
pub fn call(id: &str, name: &str, args: serde_json::Value) -> Script {
    say_and_call("", id, name, args)
}

/// Tura: odpowiedź końcowa.
pub fn answer(text: &str) -> Script {
    Script::text(FAKE_MODEL, &[text])
}
