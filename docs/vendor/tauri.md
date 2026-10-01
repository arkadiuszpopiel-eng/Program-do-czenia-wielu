# Tauri 2.12 — API używane przez powłokę (`apps/desktop/src-tauri`)

Wersje przypięte (Cargo.toml powłoki, Cargo.lock obok): `tauri =2.12.0` (cechy `tray-icon`,
`protocol-asset`), `tauri-build =2.7.0`, `tauri-plugin-global-shortcut =2.4.0` (global-hotkey 0.8),
`tauri-plugin-notification =2.5.1`, `tauri-plugin-single-instance =2.5.2`. Licencje: MIT OR Apache-2.0.
Sygnatury sprawdzone na źródłach tych wersji (crates.io) i kompilacją krzyżową `--target
x86_64-pc-windows-msvc` z atrapą app-core (clippy `-D warnings`, także `--features e2e`).
Nie używać Tauri 3 (alpha) ani API z `unstable`.

## Budowa i ACL
- `build.rs`: `tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(&[..])))`
  — lista komend z COMMANDS.md; powstają uprawnienia `allow-<komenda>` (podkreślenia → myślniki).
- `capabilities/{main,quick,pill}.json` — `windows: [label]`; tylko `core:event:allow-listen/unlisten`,
  `core:window:allow-start-dragging` (main, pill) i `allow-<komenda>`; bez `core:default`.
- `tauri.conf.json`: `app.windows = []` (okna tworzone w `setup`), `security.csp` jako mapa dyrektyw
  (`script-src 'self'`, bez `unsafe-inline` dla skryptów; style `unsafe-inline` — atrybuty `style`
  Svelte), `devCsp` (+ `ws://localhost:1421`), `assetProtocol { enable, scope: ["$HOME/Alfa/**"] }`,
  `plugins.deep-link.desktop.schemes = ["alfa"]` (rejestracja schematu w instalatorze NSIS; samo
  parsowanie URI — `app_core::protocol`, lista dozwolonych akcji).

## Aplikacja
- `tauri::Builder::default().plugin(..).invoke_handler(..).on_window_event(|w: &Window, e: &WindowEvent|)
  .setup(|app| -> Result<(), Box<dyn Error>>).run(tauri::generate_context!())`.
- Kolejność wtyczek: **single-instance pierwsza** (`tauri_plugin_single_instance::init(|app, argv, cwd|)`).
- Stan: `app.manage(T)`, `app.state::<T>().inner()`; `app.package_info().version`.
- Async: `tauri::async_runtime::{block_on, spawn}`; `JoinHandle::abort()`.
- Komendy: `#[tauri::command] async fn x(core: tauri::State<'_, AppCore>, arg: T) -> Result<R, String>`
  (async z pożyczką ⇒ musi zwracać `Result`; przyszłość `Send`); `tauri::generate_handler![..]` zwraca
  `impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static`. Argumenty JS camelCase → snake_case.
- Zdarzenia: `use tauri::Emitter; app.emit("alfa://events", payload)` (`S: Serialize + Clone`; nazwy:
  alfanumeryczne, `-`, `/`, `:`, `_`).

## Okna
- `WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))` + `.title()`,
  `.inner_size(w,h)`, `.min_inner_size(w,h)`, `.center()`, `.resizable()`, `.decorations()`,
  `.transparent()`, `.always_on_top()`, `.skip_taskbar()`, `.visible()`, `.data_directory(PathBuf)`
  (wspólny folder WebView2 dla wszystkich okien = jeden proces przeglądarki),
  `.additional_browser_args(&str)` (tylko feature `e2e`: `--remote-debugging-port=9222`; nadpisuje
  domyślne argumenty Tauri, więc powtarzamy `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`),
  `.build() -> tauri::Result<WebviewWindow>`.
- `app.get_webview_window(label)`, `w.show()/hide()/unminimize()/set_focus()/is_visible()/center()/destroy()`.
- `WindowEvent::CloseRequested { api, .. }` → `api.prevent_close()` (zamknięcie = ukrycie);
  `WindowEvent::Focused(false)` → chowanie okna Szybkiego pytania.
- DevTools: domyślnie tylko w buildzie debug (bez cechy `devtools` w release).

## Zasobnik i menu
- `tauri::menu::{Menu::with_items(app, &[&item, ..]), MenuItem::with_id(app, id, text, enabled,
  accel: Option<&str>), CheckMenuItem::with_id(app, id, text, enabled, checked, accel),
  PredefinedMenuItem::separator(app)}`; `CheckMenuItem::is_checked()/set_checked()`.
- `TrayIconBuilder::with_id(id).tooltip().menu(&menu).show_menu_on_left_click(false)
  .on_menu_event(|app, MenuEvent|).on_tray_icon_event(|tray, TrayIconEvent|).icon(img).build(app)`;
  `event.id().as_ref()`; `TrayIconEvent::Click { button: MouseButton::Left, button_state:
  MouseButtonState::Up, .. }`; ikona: `app.default_window_icon().cloned()`.

## Wtyczki
- Skróty: `tauri_plugin_global_shortcut::Builder::new().with_handler(|app, shortcut: &Shortcut,
  event| ..).build()`; `event.state() == ShortcutState::Pressed`; `Shortcut::new(Some(Modifiers::CONTROL
  | Modifiers::ALT), Code::Space)`, porównanie po `shortcut.id()`; rejestracja
  `app.global_shortcut().register(shortcut)` (`GlobalShortcutExt`) — błąd = konflikt (np. PowerToys).
- Powiadomienia: `use tauri_plugin_notification::NotificationExt;
  app.notification().builder().title(..).body(..).show()`. Okna nie dostają uprawnień wtyczki.

## Pułapki
- `tauri::generate_context!` czyta `frontendDist` (`../ui/dist`) w czasie kompilacji — CI buduje UI
  przed `cargo clippy`.
- Na Linuksie powłoka się nie kompiluje (brak webkit2gtk) — kompilator powłoki to job `tauri`
  (windows-latest). `deny.toml` (wrappers `windows`) dotyczy workspace; powłoka ciągnie windows-rs
  0.62 przez tauri/wry (ta sama wersja co `platform-windows-impl`).
