//! Łańcuch dostaw: podmiana modułu między przeglądem a zatwierdzeniem, po zatwierdzeniu
//! (restart = ponowne ładowanie z weryfikacją SHA-256), brak modułu, podmiana rekordu.

use plugin_runtime_contract::{
    LoadError, PluginApproval, PluginError, PluginRecord, PluginSource, PluginStore, Plugins,
    samples,
};

use crate::common::*;

#[tokio::test(flavor = "multi_thread")]
async fn module_swapped_before_approval_is_refused() {
    let h = harness();
    let wasm = word_count_wasm(0);
    let r = h
        .runtime
        .propose(
            samples::manifest("licznik", "1.0.0", &wasm),
            wasm.clone(),
            PluginSource::User,
        )
        .await
        .unwrap();
    tamper(
        &h.store,
        &r.manifest.wasm_sha256,
        &component(&const_body(0, b"{}")),
    );
    let e = h
        .runtime
        .approve(
            &r.manifest.id,
            &r.manifest.version,
            PluginApproval::ui(&r.review_hash),
        )
        .await;
    assert!(
        matches!(e, Err(PluginError::Load(LoadError::HashMismatch { .. }))),
        "{e:?}"
    );
    assert!(h.runtime.tools().is_empty());
}

async fn installed_then_restarted(mutate: impl FnOnce(&Harness, &PluginRecord)) -> Harness {
    let h = harness();
    let wasm = word_count_wasm(0);
    let r = h
        .install(samples::manifest("licznik", "1.0.0", &wasm), wasm)
        .await;
    mutate(&h, &r);
    harness_with(h.store.clone(), h.host.clone())
}

#[tokio::test(flavor = "multi_thread")]
async fn tampered_module_after_approval_refused_on_load() {
    let evil = component(&const_body(0, br#"{"words":999}"#));
    let h = installed_then_restarted(|h, r| tamper(&h.store, &r.manifest.wasm_sha256, &evil)).await;
    let out = h
        .tool("plugin_word_count")
        .call(serde_json::json!({"text": "a b"}), &ctx())
        .await;
    assert!(!out.is_ok() && out.data["error"] == "load", "{out:?}");
    assert!(
        out.text.contains("różni się od zatwierdzonego"),
        "{}",
        out.text
    );
    assert!(h.event_names().contains(&"plugin.load_failed".to_owned()));

    let h = installed_then_restarted(|h, r| h.store.delete_wasm(&r.manifest.wasm_sha256).unwrap())
        .await;
    let out = h
        .tool("plugin_word_count")
        .call(serde_json::json!({"text": "a"}), &ctx())
        .await;
    assert!(!out.is_ok() && out.text.contains("brak modułu"), "{out:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn tampered_record_after_approval_is_inactive() {
    let h = installed_then_restarted(|h, _| {
        let mut records = h.store.load_records().unwrap();
        records[0].manifest.capabilities = vec![fs_read_tree(r"C:\Users\user\.ssh\keys")];
        h.store.save_records(&records).unwrap();
    })
    .await;
    assert!(h.runtime.tools().is_empty() && h.runtime.tool_catalog().is_empty());
}
