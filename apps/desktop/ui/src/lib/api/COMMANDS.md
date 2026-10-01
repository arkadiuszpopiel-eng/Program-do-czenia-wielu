# Kontrakt IPC UI ↔ rdzeń (Tauri 2) — Fala 1

Źródło: `src/lib/api/client.ts` (interfejs `AlfaClient`), `types.ts`, `types-hub.ts`, `types-system.ts`,
adapter `tauri-client.ts`. Atrapa referencyjna: `fake/` (zachowanie, scenariusze błędów).
Implementacja: `crates/app-core` (metoda `AppCore::<komenda>`), powłoka `apps/desktop/src-tauri` tylko deleguje.
Gdy kontrakty `*-contract` dostaną generator typów (ADR 0013: tauri-specta / ts-rs), typy TS
z `types*.ts` zostaną zastąpione wygenerowanymi 1:1 — nazwy pól są już w `snake_case` (serde).

## Zasady

- **Argumenty komend** przekazywane z JS w `camelCase` (Tauri 2 mapuje na parametry `snake_case`
  funkcji `#[tauri::command]`). **Pola obiektów** (DTO) — `snake_case`, jak serializuje serde.
- **Jeden kanał zdarzeń** rdzeń → UI: `alfa://events`, ładunek `AlfaEvent[]` (paczka). Rdzeń
  grupuje zdarzenia co klatkę (~16 ms); UI i tak buforuje je w `RafBatcher` i aktualizuje DOM
  najwyżej raz na klatkę (PLAN §14.7). Duże dane (pliki, obrazy) — ścieżki / protokół zasobów.
- **Markdown z LLM renderuje Rust** (pulldown-cmark + ammonia, ADR 0009): `TextDelta.blocks[]`
  niosą `html_sanitized` per blok; wysyłane są tylko zmienione bloki (otwarty + nowo zamknięte).
  UI nie parsuje Markdownu i wstawia HTML wyłącznie w `SanitizedHtml.svelte`.
- **Intencje** (oznaczone ⟶): UI nie wykonuje akcji, tylko prosi rdzeń (natywny dialog, Broker,
  TTS…). Zatwierdzenia akcji agentek i zmiany poziomu autonomii **nigdy** nie dzieją się w WebView —
  komenda otwiera okno Brokera.
- **Sekrety**: `accounts_add.input.secret` to jedyne miejsce, gdzie klucz przechodzi przez IPC
  (kreator); rdzeń zapisuje go w Windows Credential Manager i nigdy nie odsyła do UI (`Account`
  ma tylko `key_stored: bool`). Hasła paczek `.alfa` — tylko w argumentach `transfer_*`.
- **Błędy** komend: odrzucenie `invoke` z komunikatem tekstowym (PL/EN wg `ui.locale`).
- **Role agentek** w DTO (`role_id`, `role_ids`) to identyfikatory `personas-contract`
  (`conductor`, `operator`, `coder`, …) — bez aliasów po stronie UI.
- Uprawnienia okien (capabilities): okno główne — wszystkie komendy poniżej; okno `quick` —
  `app_bootstrap`, `quick_*`; okno `pill` — `voice_stop_speech`, `voice_set_muted` + zdarzenia.

## Komendy

| Komenda                                                           | Argumenty                                | Wynik                          | Uwagi                                                                                              |
| ----------------------------------------------------------------- | ---------------------------------------- | ------------------------------ | -------------------------------------------------------------------------------------------------- |
| `app_bootstrap`                                                   | —                                        | `AppBootstrap`                 | język, ustawienia, układ (per maszyna), aktywna sesja, nadpisania skrótów, `onboarding_done`       |
| `app_complete_onboarding`                                         | —                                        | `()`                           |                                                                                                    |
| `app_open_system_settings`                                        | `uri`                                    | `()`                           | ⟶ tylko `ms-settings:privacy-microphone`, `ms-settings:storagesense` (lista dozwolonych w rdzeniu) |
| `app_save_layout`                                                 | `layout: LayoutPrefs`                    | `()`                           | szerokości paneli → `config/machine/<id>.toml`; otwarte panele per sesja                           |
| `app_set_active_session`                                          | `sessionId: string \| null`              | `()`                           |                                                                                                    |
| `sessions_list`                                                   | —                                        | `SessionSummary[]`             |                                                                                                    |
| `sessions_create`                                                 | `template: SessionTemplate`              | `SessionSummary`               | `empty \| coding \| research \| voice \| admin`                                                    |
| `sessions_rename`                                                 | `sessionId, title`                       | `()`                           |                                                                                                    |
| `sessions_set_pinned`                                             | `sessionId, pinned`                      | `()`                           |                                                                                                    |
| `sessions_set_archived`                                           | `sessionId, archived`                    | `()`                           |                                                                                                    |
| `sessions_remove`                                                 | `sessionId`                              | `UndoTicket`                   | cofnięcie przez 10 s, potem crypto-shredding                                                       |
| `sessions_undo_remove`                                            | `token`                                  | `()`                           |                                                                                                    |
| `sessions_duplicate_as_template`                                  | `sessionId`                              | `SessionSummary`               | ⟶                                                                                                  |
| `sessions_export`                                                 | `sessionId`                              | `ExportResult`                 | ⟶ natywny dialog zapisu `.alfa`                                                                    |
| `sessions_search`                                                 | `query`                                  | `SessionSearchHit[]`           | pełnotekstowe (FTS5); `snippet` jako zwykły tekst                                                  |
| `sessions_mark_read`                                              | `sessionId`                              | `()`                           |                                                                                                    |
| `sessions_get_draft` / `sessions_save_draft`                      | `sessionId` / `sessionId, text`          | `string` / `()`                | szkic per sesja (UI zapisuje z opóźnieniem 400 ms)                                                 |
| `turns_list`                                                      | `sessionId`                              | `TurnsSnapshot`                | całe drzewo gałęzi + adnotacje widoku (ocena, ukrycie)                                             |
| `turns_send`                                                      | `sessionId, options: SendOptions`        | `SendResult`                   | `parent_id` = liść widocznej gałęzi; offline → `assistant_turn_id: null`, tura `queued`            |
| `turns_regenerate`                                                | `sessionId, turnId, profile`             | `string` (id nowej tury)       | nowy wariant (rodzeństwo); `profile`: `local \| cloud \| null`                                     |
| `turns_edit_and_resend`                                           | `sessionId, turnId, text`                | `SendResult`                   | nowa gałąź (rodzeństwo tury użytkownika) — historia append-only                                    |
| `turns_continue`                                                  | `sessionId, turnId`                      | `string`                       | tura-dziecko z `continues = turnId`                                                                |
| `turns_stop`                                                      | `sessionId`                              | `()`                           | anulowanie ≤ 100 ms → `Stop { reason: cancelled }`                                                 |
| `turns_rate`                                                      | `turnId, rating: 'up' \| 'down' \| null` | `()`                           | adnotacja, nie zmienia tury                                                                        |
| `turns_set_hidden`                                                | `turnId, hidden`                         | `()`                           | „ukryj z widoku" (audyt zostaje)                                                                   |
| `turns_remember`                                                  | `turnId, scope`                          | `()`                           | ⟶ pamięć: `session \| project \| global \| agent`                                                  |
| `turns_read_aloud`                                                | `turnId`                                 | `()`                           | ⟶ TTS głosem agentki                                                                               |
| `turns_save_code`                                                 | `turnId, blockIndex`                     | `()`                           | ⟶ natywny dialog zapisu                                                                            |
| `turns_run_code`                                                  | `turnId, blockIndex`                     | `BrokerIntentResult`           | ⟶ zawsze przez Brokera                                                                             |
| `turns_undo_step`                                                 | `undoToken`                              | `()`                           | ⟶ dziennik cofania (`fs.*`); token `"<sesja>:u<krok>"` z `ToolStep.undo_token`                     |
| `agents_list`                                                     | `sessionId`                              | `AgentState[]`                 |                                                                                                    |
| `agents_set_roles`                                                | `sessionId, agent, roleIds`              | `()`                           | natychmiast, do dziennika                                                                          |
| `agents_apply_cast`                                               | `sessionId, template`                    | `()`                           | `standard \| solo \| coding \| research`                                                           |
| `costs_summary`                                                   | `sessionId: string \| null`              | `CostSummary`                  | kwoty w groszach (`Money.minor`)                                                                   |
| `costs_set_monthly_limit`                                         | `enabled, monthly: Money`                | `()`                           | limit PLN z możliwością wyłączenia                                                                 |
| `settings_schema`                                                 | —                                        | `SettingsPageDef[]`            | drzewo §15 z manifestów modułów (etykiety PL/EN)                                                   |
| `settings_values`                                                 | —                                        | `Record<string, SettingValue>` |                                                                                                    |
| `settings_set`                                                    | `key, value`                             | `()`                           | walidacja wg JSON Schema w `core-config`; polityki Jądra tylko przez Brokera                       |
| `settings_reset`                                                  | `key`                                    | `SettingValue`                 | zwraca domyślną                                                                                    |
| `settings_set_shortcut`                                           | `actionId, chord: string \| null`        | `()`                           | `null` = domyślny, `""` = wyłączony; skróty globalne rejestruje rdzeń (konflikty → `Toast`)        |
| `timeline_list`                                                   | `sessionId, filter: TimelineFilter`      | `TimelineEvent[]`              |                                                                                                    |
| `files_list`                                                      | `sessionId`                              | `ArtifactInfo[]`               |                                                                                                    |
| `files_preview`                                                   | `artifactId`                             | `ArtifactPreview`              | tekst jako zwykły tekst; obraz jako URL protokołu zasobów                                          |
| `files_act`                                                       | `artifactId, action`                     | `()`                           | ⟶ `open \| reveal \| copy \| save_as`                                                              |
| `accounts_catalog`                                                | —                                        | `ProviderInfo[]`               | z `providers-catalog/*.toml`                                                                       |
| `accounts_list`                                                   | —                                        | `Account[]`                    | bez sekretów                                                                                       |
| `accounts_add`                                                    | `input: AddAccountInput`                 | `Account`                      | sekret → Credential Manager                                                                        |
| `accounts_test`                                                   | `accountId`                              | `TestReport`                   | połączenie + wykrycie modeli                                                                       |
| `accounts_assign`                                                 | `accountId, assignment`                  | `()`                           | klasy zadań, agentki, STT/TTS                                                                      |
| `accounts_set_limit`                                              | `accountId, enabled, monthly`            | `()`                           |                                                                                                    |
| `accounts_remove`                                                 | `accountId`                              | `()`                           | usuwa też wpis z Credential Managera                                                               |
| `transfer_export`                                                 | `request: ExportRequest`                 | `ExportResult`                 | ⟶ natywny dialog; sekrety nigdy                                                                    |
| `transfer_export_secrets`                                         | `password`                               | `ExportResult`                 | ⟶ natywny dialog; osobna paczka sekretów, zawsze szyfrowana hasłem (min. 8 znaków)                 |
| `transfer_inspect`                                                | `password, path: string \| null`         | `InspectResult`                | ⟶ dialog otwarcia (gdy `path = null`) + dry-run                                                    |
| `transfer_import`                                                 | `request: ImportRequest`                 | `ImportResult`                 | snapshot przed importem                                                                            |
| `transfer_rollback`                                               | `snapshotId`                             | `()`                           |                                                                                                    |
| `permissions_get`                                                 | `sessionId`                              | `PermissionsState`             |                                                                                                    |
| `permissions_request_level`                                       | `level, sessionId`                       | `BrokerIntentResult`           | ⟶ obniżenie od razu (`applied`); podniesienie — okno Brokera (`opened_broker`; bez okna — odmowa)  |
| `permissions_open_approval`                                       | `approvalId`                             | `BrokerIntentResult`           | ⟶ przenosi do karty w oknie Brokera                                                                |
| `models_local_list`                                               | —                                        | `LocalModelInfo[]`             | modele lokalne z manifestu `providers-local`                                                       |
| `models_local_download` / `models_local_cancel`                   | `modelId: string \| null`                | `()`                           | pobieranie (wznawiane, SHA-256); `null` = domyślny / wszystkie; postęp: `LocalModelProgress`       |
| `device_profile` / `device_measure`                               | —                                        | `DeviceProfile`                |                                                                                                    |
| `voice_devices`                                                   | —                                        | `AudioDevice[]`                |                                                                                                    |
| `voice_start_mic_test` / `voice_stop_mic_test`                    | `deviceId` / —                           | `()`                           | poziomy przez `MicLevel` (≤ 30/s)                                                                  |
| `voice_set_mic_enabled` / `voice_set_muted` / `voice_stop_speech` | `enabled` / `muted` / —                  | `()`                           |                                                                                                    |
| `system_status`                                                   | —                                        | `SystemStatus`                 | online, kolejka, 429, klucze, mikrofon, dysk                                                       |
| `system_retry_queue`                                              | —                                        | `()`                           |                                                                                                    |
| `quick_ask`                                                       | `text`                                   | `QuickAskResult`               | sesja „Szybkie pytania" wg `quick.session_mode`                                                    |
| `quick_expand_to_main`                                            | `sessionId`                              | `()`                           | pokazuje okno główne z tą sesją                                                                    |
| `quick_hide`                                                      | —                                        | `()`                           | `Esc` w oknie Szybkiego pytania                                                                    |

## Zdarzenia (`alfa://events`, ładunek `AlfaEvent[]`)

| `type`                              | Pola                                                 | Kiedy                                                                                             |
| ----------------------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `TurnAppended`                      | `session_id, turn: Turn`                             | nowa tura (użytkownik / agentka / wariant / gałąź)                                                |
| `TurnStatus`                        | `session_id, turn_id, status`                        | zmiana stanu bez treści (np. kolejka offline → wysłana)                                           |
| `TextDelta`                         | `session_id, turn_id, text, blocks: RenderedBlock[]` | strumień; `text` = surowa delta (aria-live), `blocks` = zmienione bloki HTML                      |
| `ThinkingDelta`                     | `session_id, turn_id, elapsed_ms, done`              | blok „myślenie" (bez treści)                                                                      |
| `ToolCall`                          | `session_id, turn_id, step: ToolStep`                | krok narzędzia (`running` → `done`/`error`, `undo_token`)                                         |
| `ApprovalPending`                   | `session_id, turn_id, approval`                      | karta „czeka na zatwierdzenie" (także zmiana `status`)                                            |
| `Usage`                             | `session_id, turn_id, usage`                         | tokeny, koszt (PLN), opóźnienie, model                                                            |
| `Stop`                              | `session_id, turn_id, reason`                        | `end \| refusal \| tool_use \| max_tokens \| cancelled`                                           |
| `Error`                             | `session_id, turn_id, error: TurnError`              | `offline \| rate_limited (retry_at) \| no_keys \| provider \| context_overflow \| budget_blocked` |
| `SessionUpdated` / `SessionRemoved` | `session` / `session_id`                             | lista sesji, kropka aktywności, nieprzeczytane                                                    |
| `AgentsChanged`                     | `session_id, agents`                                 | obsada i stan (mówi / pracuje / czeka)                                                            |
| `ActivityChanged`                   | `session_id, activity \| null`                       | kapsuła aktywności                                                                                |
| `CostsChanged`                      | `session_id, costs`                                  | koszt sesji / dnia / miesiąca, kontekst                                                           |
| `SystemStatusChanged`               | `status`                                             | stany z PLAN §14.4                                                                                |
| `TimelineAppended`                  | `event: TimelineEvent`                               | Oś czasu v0                                                                                       |
| `AccountChanged`                    | `account`                                            | Hub kont                                                                                          |
| `MicLevel`                          | `level: 0..1`                                        | test mikrofonu, pigułka (≤ 30/s)                                                                  |
| `VoicePill`                         | `state: { agent, mic, level }`                       | pigułka głosowa                                                                                   |
| `Toast`                             | `kind, message: {pl, en}`                            | komunikaty rdzenia (np. konflikt skrótu globalnego)                                               |
| `OpenSession`                       | `session_id`                                         | przejdź do sesji (zasobnik, `alfa://session/…`, Szybkie pytanie → pełne okno)                     |
| `LocalModelProgress`                | `model_id, state, bytes, total, error`               | pobieranie modelu lokalnego: `downloading \| done \| failed \| cancelled`                         |

## Okna

| Okno (label) | Strona       | Rozmiar                      | Uwagi                                                                                                                |
| ------------ | ------------ | ---------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `main`       | `index.html` | 1200 × 800, min. 400 × 500   | własny pasek tytułu: `data-tauri-drag-region`; miejsce na natywne przyciski — zmienna CSS `--alfa-titlebar-controls` |
| `quick`      | `quick.html` | 640 × auto, środek ekranu    | skrót globalny `Ctrl+Alt+Space` (rdzeń), przezroczyste tło                                                           |
| `pill`       | `pill.html`  | 220 × 48, zawsze na wierzchu | bez frameworka; zdarzenia `VoicePill` / `MicLevel`                                                                   |

Skróty obsługiwane przez rdzeń (nie przez WebView): `Ctrl+Alt+Space` (Szybkie pytanie),
`Ctrl+Shift+F12` (STOP WSZYSTKIEGO), przytrzymanie `Spacji` (PTT, hook `WH_KEYBOARD_LL`).
UI blokuje `F5` / `Ctrl+R` / `Ctrl+Shift+R`.
