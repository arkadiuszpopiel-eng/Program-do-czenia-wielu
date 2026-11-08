//! Wspólna bramka GUI dla `tools-window`, `tools-uia`, `tools-input`, `tools-screen`
//! (PLAN §7, §8.2; THREAT_MODEL S11, S26): okno celu → aplikacja → `gui.control(aplikacja)`
//! z Brokera (`decide` → `verify`), odmowa wobec okien chronionych zanim cokolwiek trafi do
//! Brokera, krok weryfikacji po akcji jako zdarzenie (F6-04), mapowanie `GuiError` na wynik
//! czytelny dla modelu. Port i tak sprawdza strażnikiem tuż przed akcją (obrona w głąb).

use std::sync::Arc;

use core_bus_contract::{EventBus, Level};
use platform_contract::{
    DesktopPort, DesktopWindow, GuiError, PlatformError, WindowId, WindowState, image_file_name,
};
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{AppSelector, Capability};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::{
    Authorization, BrokerGate, DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome,
    action_request, base_facts, tool_event,
};

/// Pseudo-aplikacja „cały pulpit” w `gui.control` (lista okien, zrzut monitora/obszaru).
pub const DESKTOP_APP: &str = "desktop.exe";
/// Zdarzenie: krok weryfikacji po akcji GUI (F6-04) — bez treści.
pub const EVENT_VERIFY: &str = "tool.gui.verify";

/// Wynik pośredni narzędzia (błąd = gotowy wynik dla modelu).
pub type Step<T> = Result<T, Box<ToolOutcome>>;

fn internal(text: String) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(ToolErrorKind::Internal, text))
}

/// `gui.control(desktop.exe)`.
pub fn desktop_capability() -> Step<Capability> {
    AppSelector::parse(DESKTOP_APP)
        .map(Capability::GuiControl)
        .map_err(|e| internal(format!("Zdolność pulpitu: {e}.")))
}

/// `gui.control(<obraz okna>)` (nazwa pliku procesu).
pub fn app_capability(window: &DesktopWindow, action: &str) -> Step<Capability> {
    AppSelector::parse(&image_file_name(&window.image))
        .map(Capability::GuiControl)
        .map_err(|_| Box::new(protected(action)))
}

/// Odmowa wobec okna chronionego (Alfa, Broker, Broker-UI, helper, proces nieznany).
pub fn protected(action: &str) -> ToolOutcome {
    ToolOutcome::denied(
        DenialReason::KernelBlock {
            rule: KernelRule::GuiControlOfKernelProcess,
        },
        action,
    )
}

/// Okno celu: istnieje i nie jest chronione (strażnik portu).
pub fn target_window(desktop: &dyn DesktopPort, id: WindowId, action: &str) -> Step<DesktopWindow> {
    let w = desktop
        .window(id)
        .map_err(|e| Box::new(gui_outcome(&e, action)))?;
    if w.protected || desktop.guard().is_protected(w.pid, &w.image) {
        return Err(Box::new(protected(action)));
    }
    Ok(w)
}

/// Zgoda Brokera na zdolność (`decide` → zatwierdzenie → `verify`); `private` = akcja odsłania
/// treść ekranu (składnik A trifecty).
pub async fn authorize(
    gate: &BrokerGate,
    ctx: &ToolCtx,
    manifest: &ToolManifest,
    cap: &Capability,
    private: bool,
    action: &str,
) -> Step<Authorization> {
    let mut facts = base_facts(manifest, ctx);
    facts.touches_private_data = private;
    let auth = gate
        .authorize(action_request(ctx, cap.clone(), facts), ctx)
        .await
        .map_err(|e| Box::new(e.into_outcome(action)))?;
    if let Err(e) = gate.verify(&auth, cap, &ctx.holder) {
        gate.release(std::slice::from_ref(&auth)).await;
        return Err(Box::new(e.into_outcome(action)));
    }
    Ok(auth)
}

/// Błąd GUI → wynik dla modelu (po polsku, z podpowiedzią, co dalej).
pub fn gui_outcome(e: &GuiError, action: &str) -> ToolOutcome {
    let failed = |kind, hint: &str| {
        ToolOutcome::failed(kind, format!("Nie wykonano: {action} — {e}. {hint}"))
    };
    match e {
        GuiError::ProtectedTarget(_) => protected(action),
        GuiError::UserInterrupted { sent } => {
            let mut o = ToolOutcome::cancelled(action);
            o.text = format!(
                "Przerwano: {action} — użytkownik użył myszy lub klawiatury (fizyczne wejście ma pierwszeństwo). \
                 Wysłano {sent} paczek zdarzeń. Nie ponawiaj od razu; poczekaj albo poproś użytkownika o przekazanie sterowania."
            );
            o.data = serde_json::json!({ "interrupted_by_user": true, "sent_batches": sent });
            o
        }
        GuiError::UserActive => {
            let mut o = ToolOutcome::cancelled(action);
            o.text = format!(
                "Nie zaczęłam: {action} — użytkownik właśnie używa myszy lub klawiatury. Spróbuj za chwilę albo poproś o przekazanie sterowania."
            );
            o.data = serde_json::json!({ "user_active": true });
            o
        }
        GuiError::Cancelled => ToolOutcome::cancelled(action),
        GuiError::Policy(m) => {
            let mut o = ToolOutcome::denied(DenialReason::Policy, action);
            o.text = format!(
                "Odmowa: {action} — {m}. Nie ponawiaj tej samej akcji; wybierz inną drogę."
            );
            o
        }
        GuiError::Timeout { .. } => failed(
            ToolErrorKind::Timeout,
            "Aplikacja nie odpowiada przez UI Automation — spróbuj zrzutu ekranu (screen_capture) albo później.",
        ),
        GuiError::ElementNotFound(_) => failed(
            ToolErrorKind::NotFound,
            "Odśwież drzewo (uia_tree) albo listę okien (window_list).",
        ),
        GuiError::PatternUnsupported(_) => failed(
            ToolErrorKind::Unsupported,
            "Wybierz inny element albo inną akcję.",
        ),
        GuiError::Elevated => failed(
            ToolErrorKind::Unsupported,
            "Okno administratora wymaga helpera uiAccess — poproś użytkownika.",
        ),
        GuiError::TargetChanged { .. } => failed(
            ToolErrorKind::Io,
            "Sprawdź okna (window_list) i spróbuj ponownie.",
        ),
        GuiError::Platform(PlatformError::Unsupported(_)) => failed(ToolErrorKind::Unsupported, ""),
        GuiError::Platform(_) => failed(ToolErrorKind::Io, ""),
    }
}

/// Argumenty muszą być obiektem JSON (serde przyjąłby też tablicę jako strukturę).
pub fn object_args(args: &serde_json::Value) -> Step<()> {
    if args.is_object() {
        Ok(())
    } else {
        Err(Box::new(ToolOutcome::failed(
            ToolErrorKind::InvalidArgs,
            "Niepoprawne argumenty: oczekiwano obiektu JSON zgodnego ze schematem narzędzia.",
        )))
    }
}

/// Uruchamia wywołanie portu (blokujące: UIA z limitem czasu, `SendInput` z tempem, GDI) poza
/// wątkami asynchronicznymi (`spawn_blocking`).
pub async fn blocking<T, F>(work: F) -> Step<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| internal(format!("Wątek portu GUI zakończył się błędem: {e}.")))
}

/// Okno w wyniku narzędzia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WindowBrief {
    /// Identyfikator okna (do argumentu `window`).
    pub window: u64,
    /// Tytuł (treść niezaufana).
    pub title: String,
    /// Aplikacja (plik wykonywalny).
    pub app: String,
    /// Lewa krawędź (px).
    pub x: i32,
    /// Górna krawędź (px).
    pub y: i32,
    /// Szerokość (px).
    pub width: i32,
    /// Wysokość (px).
    pub height: i32,
    /// `normal` | `minimized` | `maximized`.
    pub state: String,
    /// Na pierwszym planie.
    pub focused: bool,
    /// Monitor.
    pub monitor: u32,
}

/// Skrót okna dla modelu.
pub fn brief(w: &DesktopWindow) -> WindowBrief {
    WindowBrief {
        window: w.id.0,
        title: w.title.clone(),
        app: image_file_name(&w.image),
        x: w.rect.left,
        y: w.rect.top,
        width: w.rect.width(),
        height: w.rect.height(),
        state: match w.state {
            WindowState::Normal => "normal",
            WindowState::Minimized => "minimized",
            WindowState::Maximized => "maximized",
        }
        .into(),
        focused: w.focused,
        monitor: w.monitor,
    }
}

/// Publikuje zdarzenie narzędzia (bez treści ekranu i bez wpisywanego tekstu).
pub async fn emit(
    bus: Option<&Arc<dyn EventBus>>,
    name: &str,
    payload: serde_json::Value,
    ctx: &ToolCtx,
) {
    if let Some(bus) = bus {
        let _ = bus
            .publish(tool_event(name, Level::Info, payload, ctx))
            .await;
    }
}

/// Krok weryfikacji po akcji GUI (F6-04): czy stan po akcji zgadza się z zamiarem.
pub async fn emit_verify(
    bus: Option<&Arc<dyn EventBus>>,
    ctx: &ToolCtx,
    tool: &str,
    window: WindowId,
    ok: bool,
    check: &str,
) {
    let payload = serde_json::json!({ "tool": tool, "window": window.0, "ok": ok, "check": check });
    emit(bus, EVENT_VERIFY, payload, ctx).await;
}

#[cfg(test)]
#[path = "gui_tests.rs"]
mod tests;
