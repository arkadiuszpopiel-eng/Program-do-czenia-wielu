//! Dług katalogu funkcji (docs/plan/45-testy-ai.md): kanwa artefaktów, „Wklej” ze schowka,
//! „Sprawdź teraz” aktualizacji i odsłuch głosu w Kreatorze — na rdzeniu, bez systemu:
//! schowek to atrapa wstrzyknięta w opcjach (prawdziwy schowek nietknięty), głos to port-szpieg
//! albo brak silnika TTS, a aktualizacje w buildzie testowym nie mają źródła wydań (bez sieci).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_core::dto::{
    AgentDraft, AudioDevice, CanvasKind, DictationAction, LocalizedText, UpdatePhase, VoiceFeatures,
};
use app_core::ports::{HeadlessShell, VoicePort};
use app_core::{AppCore, AppError, AppPaths, ErrorCode};
use async_trait::async_trait;
use common::{ScriptedProvider, harness, options};
use platform_contract::{ClipboardContent, ClipboardPort, PlatformError};
use platform_fake::FakeClipboard;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canvas_render_sanitizes_each_kind_and_refuses_bad_sources() {
    let h = harness().await;
    let md = h
        .core
        .canvas_render(
            CanvasKind::Markdown,
            "# Plan\n\n<b>x</b>".into(),
            "pl".into(),
        )
        .await
        .unwrap();
    assert_eq!(md, "<h1>Plan</h1>\n<p>&lt;b&gt;x&lt;/b&gt;</p>\n");

    let html = h
        .core
        .canvas_render(
            CanvasKind::Html,
            "<h1 onclick=\"alert(1)\">A</h1><script>fetch('x')</script><a href=\"javascript:x\">l</a>"
                .into(),
            "en".into(),
        )
        .await
        .unwrap();
    assert!(
        html.starts_with("<!doctype html><html lang=\"en\">"),
        "{html}"
    );
    assert!(html.contains("<h1>A</h1>"), "{html}");
    for banned in ["<script", "onclick", "javascript:", "fetch("] {
        assert!(!html.contains(banned), "{banned} w {html}");
    }

    let svg = h
        .core
        .canvas_render(
            CanvasKind::Svg,
            "<svg viewBox=\"0 0 1 1\" onload=\"x()\"><rect/></svg>".into(),
            "pl".into(),
        )
        .await
        .unwrap();
    assert!(
        svg.contains("<svg viewBox=\"0 0 1 1\"><rect></rect></svg>"),
        "{svg}"
    );
    assert!(
        svg.contains("lang=\"pl\"") && !svg.contains("onload"),
        "{svg}"
    );

    let limit = app_files::canvas::ARTIFACT_MAX_BYTES;
    for (kind, source) in [
        (CanvasKind::Html, " \n\t ".to_owned()),
        (CanvasKind::Markdown, "a".repeat(limit + 1)),
    ] {
        let err = h
            .core
            .canvas_render(kind, source, "pl".into())
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{kind:?}");
    }
    let at_limit = h
        .core
        .canvas_render(CanvasKind::Markdown, "a".repeat(limit), "pl".into())
        .await;
    assert!(at_limit.is_ok(), "dokładnie limit mieści się w kanwie");
}

/// Schowek, którego system nie oddaje (np. zajęty przez inny program).
struct BrokenClipboard;

impl ClipboardPort for BrokenClipboard {
    fn get(&self) -> Result<ClipboardContent, PlatformError> {
        Err(PlatformError::PermissionDenied("schowek zajęty".into()))
    }
    fn set(&self, _content: ClipboardContent) -> Result<(), PlatformError> {
        Ok(())
    }
    fn restore_previous(&self) -> Result<bool, PlatformError> {
        Ok(false)
    }
}

async fn core_with(
    clipboard: Option<Arc<dyn ClipboardPort>>,
    voice: Option<Arc<dyn VoicePort>>,
) -> (AppCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let mut opts = options(Some(provider), Arc::new(HeadlessShell::default()));
    opts.clipboard = clipboard;
    opts.voice = voice;
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    (core, dir)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn system_clipboard_text_returns_only_text_and_leaves_clipboard_intact() {
    let clipboard = Arc::new(FakeClipboard::new());
    let port: Arc<dyn ClipboardPort> = clipboard.clone();
    let (core, _dir) = core_with(Some(port), None).await;

    assert_eq!(core.system_clipboard_text().await.unwrap(), None, "pusty");
    let text = "Wklejka: zażółć gęślą jaźń\nlinia 2";
    clipboard.set(ClipboardContent::Text(text.into())).unwrap();
    assert_eq!(
        core.system_clipboard_text().await.unwrap().as_deref(),
        Some(text)
    );
    assert_eq!(
        clipboard.get().unwrap(),
        ClipboardContent::Text(text.into()),
        "odczyt nie zmienia schowka"
    );
    for other in [
        ClipboardContent::Files(vec![std::env::temp_dir().join("a.txt")]),
        ClipboardContent::ImagePng(vec![0x89, b'P', b'N', b'G']),
    ] {
        clipboard.set(other).unwrap();
        assert_eq!(core.system_clipboard_text().await.unwrap(), None);
    }

    let (broken, _dir) = core_with(Some(Arc::new(BrokenClipboard)), None).await;
    let err = broken.system_clipboard_text().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Internal);
    assert!(err.message.contains("schowek"), "{}", err.message);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn updates_check_without_release_feed_is_unavailable_and_changes_nothing() {
    if updater_impl::BUILTIN_FEED_URL.is_some() && updater_impl::BUILTIN_PUBLIC_KEY.is_some() {
        // Build z adresem wydań: sprawdzenie poszłoby do sieci — test go nie wywołuje.
        eprintln!("pominięto: build ma wbudowany adres wydań i klucz");
        return;
    }
    let h = harness().await;
    let before = h.core.updates_status().await.unwrap();
    let err = h.core.updates_check().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Unavailable, "{err:?}");
    assert_eq!(err.details.text("module"), Some("updater"));
    let after = h.core.updates_status().await.unwrap();
    assert_eq!(after.last_check, None, "nie zapisano sprawdzenia");
    assert_eq!(after.phase, before.phase);
    assert!(
        !matches!(after.phase, UpdatePhase::Checking | UpdatePhase::Failed),
        "{after:?}"
    );
    assert_eq!(after.available, None);
}

/// Port głosu, który zapisuje prośby o czytanie na głos.
#[derive(Default)]
struct SpyVoice {
    spoken: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl VoicePort for SpyVoice {
    async fn devices(&self) -> Result<Option<Vec<AudioDevice>>, AppError> {
        Ok(None)
    }
    async fn start_mic_test(&self, _device: Option<String>) -> Result<(), AppError> {
        Ok(())
    }
    async fn stop_mic_test(&self) -> Result<(), AppError> {
        Ok(())
    }
    async fn set_mic_enabled(&self, _enabled: bool) -> Result<(), AppError> {
        Ok(())
    }
    async fn set_muted(&self, _muted: bool) -> Result<(), AppError> {
        Ok(())
    }
    async fn stop_speech(&self) -> Result<(), AppError> {
        Ok(())
    }
    async fn read_aloud(
        &self,
        agent: &str,
        text: &str,
        _turn: Option<&str>,
    ) -> Result<(), AppError> {
        self.spoken
            .lock()
            .unwrap()
            .push((agent.to_owned(), text.to_owned()));
        Ok(())
    }
    async fn dictation(&self, _action: DictationAction) -> Result<VoiceFeatures, AppError> {
        Ok(VoiceFeatures::unavailable(LocalizedText::new(
            "test", "test",
        )))
    }
}

fn draft(name: Option<&str>, base: Option<&str>) -> AgentDraft {
    let voice = base.map(|b| {
        json!({"base": b, "pitch": 1.0, "rate": 1.0, "perceived_age": 30, "timbre": "ciepła",
               "design_prompt": "spokojny głos"})
    });
    serde_json::from_value(json!({
        "id": null, "name": name, "forms": null, "glyph": "Z",
        "color": "color.agent.custom-2", "character": "Spokojna.",
        "voice": voice,
        "role": {"id": "testowa", "name": "Testowa", "description": "Test.",
                 "prompt": "Odpowiadam krótko.", "model_policy": "conversation",
                 "tools": [], "read_only": true, "untrusted_isolated": false, "author": false},
        "limits": {"autonomy": "L1", "budget": null, "fs_write": [], "memory_scope": "agent",
                   "retain_days": 30, "triggers": []},
        "skills": []
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn builder_voice_preview_reads_greeting_with_base_speaker() {
    let spy = Arc::new(SpyVoice::default());
    let port: Arc<dyn VoicePort> = spy.clone();
    let (core, _dir) = core_with(Some(Arc::new(FakeClipboard::new())), Some(port)).await;

    core.builder_voice_preview(draft(Some("Zofia"), Some("pl-f2")))
        .await
        .unwrap();
    core.builder_voice_preview(draft(Some("Ola"), Some("pl-f1")))
        .await
        .unwrap();
    core.builder_voice_preview(draft(None, None)).await.unwrap();

    let spoken = spy.spoken.lock().unwrap().clone();
    let speakers: Vec<&str> = spoken.iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(speakers, ["beta", "alfa", "alfa"], "mówczyni bazowa szkicu");
    assert!(spoken[0].1.contains("jestem Zofia"), "{spoken:?}");
    assert!(spoken[1].1.contains("jestem Ola"), "{spoken:?}");
    assert!(spoken[2].1.contains("jestem nowa agentka"), "{spoken:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn builder_voice_preview_without_tts_engine_explains_why() {
    // Domyślny głos rdzenia na wirtualnym audio, bez silnika TTS w profilu testowym.
    let h = harness().await;
    let err = h
        .core
        .builder_voice_preview(draft(Some("Zofia"), Some("pl-f2")))
        .await
        .unwrap_err();
    assert_eq!(err.message, app_core::NO_TTS);
}
