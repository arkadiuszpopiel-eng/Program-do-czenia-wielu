# 02 — Architektura docelowa

Stan kodu opisany tutaj pochodzi z przeglądu repozytorium 2026-10-09 (odczyt kodu, bez budowania). Liczby
o crate'ach: [`../CATALOG.md`](../CATALOG.md). Obecna architektura: [`../ARCHITECTURE.md`](../ARCHITECTURE.md).

## Obraz całości

```
                 ┌──────────────────────────── KOMPUTER (host) ─────────────────────────────┐
  okno Tauri ───▶│  transport: IPC ─┐                                                       │
                 │                  ├─▶ BRAMA KLIENTÓW ─▶ AppCore ─▶ moduły ─▶ BROKER ─▶ system
  telefon  ──┐   │  transport:      │   (kto, co wolno,   (210 poleceń,        (zgody,      │
  przeglądarka├─▶ PRZEKAŹNIK ─▶ kanał E2E ─┘ numery zdarzeń) zdarzenia)        dziennik)    │
             │   │  (szyfrogram)                                │                            │
             │   │                                              └─▶ modele lokalne, głos     │
             │   └──────────────────────────────────────────────────────────────────────────┘
             └─▶ (bez komputera) model chmurowy przez API ─ rozmowa dopisywana później do historii
```

Trzy pojęcia:

- **Host** — proces z `AppCore`, modułami i Brokerem. Dziś żyje wewnątrz okna Tauri; docelowo może działać
  także bez okna (moduł `host-headless`).
- **Klient** — interfejs Svelte. Ten sam kod w trzech opakowaniach: okno Tauri, strona WWW (PWA), aplikacja
  Android (Tauri mobile).
- **Transport** — sposób przesłania poleceń i zdarzeń. Klient nie wie, którego używa.

## Warstwy i reguły zależności (bez zmian)

| Warstwa | Wzorzec nazw | Zależy od | Kto pilnuje |
| --- | --- | --- | --- |
| Kontrakty | `<m>-contract` | innych kontraktów, `lib-*` | `scripts/check-deps.sh` w CI |
| Implementacje | `<m>-impl` | kontraktów; nigdy od cudzego `-impl` | `check-deps.sh` |
| Atrapy | `<m>-fake` | kontraktów; tylko w testach | `check-deps.sh` |
| Biblioteki | `lib-*` | `lib-*` i kontraktów | `check-deps.sh` |
| Kompozycja | `app-*` | wszystkiego powyżej | `check-deps.sh` |
| Powłoki i klienci | `apps/*` | `app-*` przez `app-api` | przegląd |

Dodatkowo `deny.toml` dopuszcza crate `windows` tylko w crate'ach platformy. Plan v6 dodaje analogiczną regułę
dla każdego nowego systemu (np. `zbus`, `jni` tylko w `platform-linux-*` / `platform-android-*`).

## Szew 1 — fabryka platform

**Dziś.** Porty są zdefiniowane czysto (ok. 30 w `platform-contract`, m.in. `FsPort`, `ExecPort`,
`ClipboardPort`, `HotkeyPort`, `TrayPort`, `SecurePipePort`, `UiaPort`, `ScreenCapturePort`; poza nim
`AudioIo`, `SecretStore`, `KeyVault`, `ShellPort`). `AppOptions` przyjmuje każdy port jako
`Option<Arc<dyn …>>`. Ale korzeń kompozycji wpisuje typy Windows wprost w ok. 10 miejscach
(`app-core/src/parts/*`, `app-agents/src/sysnet.rs`, `app-gui`, `app-terminal`, `app-broker`, `app-safety`).

**Docelowo.** Jeden moduł `platform-select` z funkcją `platform() -> PlatformPorts`, wybierający implementację
przez `cfg(target_os)`. Korzeń kompozycji zna tylko `PlatformPorts`. Port, którego system nie ma, zwraca
`Unsupported` — a interfejs ukrywa zależną od niego funkcję na podstawie **mapy zdolności hosta** (niżej).

```rust
// szkic — źródłem prawdy będzie crate platform-contract
pub struct PlatformPorts {
    pub fs: Arc<dyn FsPort>,
    pub exec: Arc<dyn ExecPort>,
    pub secrets: Arc<dyn SecretStore>,
    pub audio: Option<Arc<dyn AudioIo>>,       // brak = głos niedostępny na tej platformie
    pub uia: Option<Arc<dyn UiaPort>>,         // brak = sterowanie komputerem niedostępne
    /* … pozostałe porty … */
    pub capabilities: HostCapabilities,        // co ten host potrafi — trafia do klientów
}
```

## Szew 2 — kontrakty Brokera niezależne od systemu

Najtrudniejszy szew, bo dotyka Jądra (zmiany tylko po przeglądzie właściciela). Dziś semantyka Windows jest
wpisana w kontrakty, a nie odgrodzona `cfg`:

| Miejsce | Dziś | Docelowo |
| --- | --- | --- |
| `safety-broker-contract/src/scope.rs` | ścieżki sprowadzane do małych liter z `\` | `PathScope` parametryzowany `PathFlavor` (Windows / POSIX): wielkość liter, separator, korzenie |
| `compliance-contract/src/deny/path.rs` | listy zakazów rozwijają `%ZMIENNE%` profilu Windows | `PathEnv` per system; listy zakazów jako dane per system |
| `safety-broker-contract/src/shell_guard.rs`, `tools-shell-contract` | reguły tylko dla `cmd` i PowerShell; `ShellKind = Pwsh / Powershell / Cmd` | analizator jako port z implementacją per powłoka; dochodzi `Posix` (bash/sh/zsh) |
| `safety-broker-contract/src/ipc.rs` (`Hello`) | tożsamość procesu = SID + ścieżka obrazu | tożsamość abstrakcyjna: Windows — SID; Linux — UID + `SO_PEERCRED` |
| `platform-contract` (listy `PROTECTED_IMAGES`, klasy UWP) | stałe Windows eksportowane z kontraktu | dane platformy dostarczane przez implementację |

Uwaga: otwarte ustalenia analizatora powłoki (S-01…S-04, S-06) trzeba rozstrzygnąć **przed** tą przebudową,
żeby nie przenosić znanych dziur do nowej struktury.

## Szew 3 — protokół niezależny od transportu

**Dziś.** 210 poleceń w jednej liście (`app-api/src/commands.rs`, makro `with_commands!`), ok. 38 zdarzeń
(`AlfaEvent` w `app-api/src/dto/events.rs`), DTO w `app-api/src/dto/`. Interfejs zna tylko `AlfaClient`
(`apps/desktop/ui/src/lib/api/client.ts`) — ma dwie implementacje: `TauriAlfaClient` i `FakeAlfaClient`.
To bardzo dobry punkt wyjścia. Brakuje pięciu rzeczy:

| Brak | Skutek dziś | Rozwiązanie |
| --- | --- | --- |
| Numery sekwencji zdarzeń | klient, który nie nadąża, po cichu gubi paczki (`pump.rs`) | każde zdarzenie dostaje `seq`; host trzyma bufor ostatnich N; klient po przerwie prosi „od `seq`”, a gdy bufor nie sięga — dostaje `resync` i czyta stan od nowa |
| Błędy strukturalne na łączu | powłoka spłaszcza `AppError` do tekstu | przez łącze idzie `{ code, message, details }`; tekst tłumaczy klient |
| Tożsamość klienta i uprawnienia | jedyne ACL to uprawnienia okien Tauri | każda sesja klienta ma `ClientId`, rodzaj (okno / urządzenie sparowane) i zestaw dozwolonych poleceń |
| Polecenia zakładające lokalną maszynę | okna dialogowe systemu, podgląd `asset:`, ścieżki bezwzględne w DTO | polecenia oznaczone `local_only`; dla klientów zdalnych odpowiedniki: wybór pliku z listy hosta, podgląd przesyłany strumieniem |
| Typy TS pisane ręcznie | ryzyko rozjazdu przy trzech klientach | generator z Rust (ADR 0013); CI sprawdza `git diff --exit-code` |

Ramka protokołu (szkic do ADR 0019):

```
→ { "v": 1, "id": 4711, "cmd": "turn_send", "args": { … } }
← { "v": 1, "id": 4711, "ok": { … } }            albo  "err": { "code": "…", "message": "…" }
← { "v": 1, "seq": 90211, "events": [ { "type": "TurnDelta", … }, … ] }
→ { "v": 1, "id": 4712, "cmd": "events_resume", "args": { "after_seq": 90180 } }
```

Wersja protokołu `v` jest negocjowana przy połączeniu; host obsługuje bieżącą i poprzednią, żeby telefon
z nieaktualną aplikacją nadal działał.

## Szew 4 — host bez okna

`AppCore::build(paths, options)` już działa bez Tauri (`shell: None`, `HeadlessShell`), ale nie ma pliku
wykonywalnego, który by go uruchomił. Moduł `host-headless` daje binarkę `alfa-host`: start przy logowaniu
(opcjonalnie), ikona w zasobniku przez okno Tauri uruchamiane na żądanie, brama klientów zdalnych. Okno Tauri
staje się wtedy jednym z klientów hosta — lokalnym, przez nazwany potok — zamiast jego właścicielem.

Przejście jest dwustopniowe, żeby nie ryzykować wersji Windows: najpierw brama klientów **wewnątrz** obecnego
procesu aplikacji (zdalny dostęp działa, gdy Alfa jest uruchomiona), potem wydzielenie hosta.

## Mapa zdolności hosta

Klient pyta hosta, co ten potrafi, i pokazuje tylko to. Dzięki temu jeden interfejs obsługuje Windows z pełnym
zestawem, Linuksa bez Office i telefon bez komputera.

| Zdolność | Windows | Linux | Android bez komputera |
| --- | --- | --- | --- |
| Rozmowa — model chmurowy | tak | tak | tak |
| Rozmowa — model lokalny | tak | tak | nie |
| Głos (STT/TTS) | tak (WASAPI) | planowane (PipeWire) | planowane (API systemu) |
| Narzędzia: pliki, sieć | tak | tak | nie |
| Narzędzia: powłoka | tak | po analizatorze POSIX | nie |
| Sterowanie komputerem (UIA, wejście, ekran) | tak | badawcze (AT-SPI, portale) | nie |
| Office, rejestr | tak | nie | nie |
| Pamięć i wyszukiwanie | tak | tak | tylko bieżąca rozmowa |

## Moduły planu v6

| Moduł | Rodzaj | Zadanie | Etap |
| --- | --- | --- | --- |
| `platform-select` | biblioteka kompozycji | wybór implementacji portów per system, mapa zdolności | B |
| `remote-gateway` | moduł hosta | brama klientów: sesje, `seq`, ACL, ramki protokołu | D |
| `device-pairing` | moduł hosta | parowanie QR, lista urządzeń, odwołanie, klucze | D |
| `remote-relay` | osobna usługa | przekaźnik szyfrogramu; do postawienia samodzielnie | D |
| `host-headless` | binarka | host bez okna | D |
| `web-panel` | klient | PWA z interfejsem Svelte | E |
| `offline-client` | biblioteka klienta | rozmowa przez API bez hosta, kolejka do scalenia | F |
| `platform-android` | implementacje portów | magazyn kluczy, powiadomienia, biometria | F |
| `platform-linux` | implementacje portów | pliki, procesy, schowek, skróty, zasobnik, sekrety, audio | G |

Szkice SPEC: [`../modules/README.md`](../modules/README.md), sekcja „Planowane (plan v6)”.

## Mapa gotowości kodu

**Gotowe na wiele platform**

- Dyscyplina kontrakt / implementacja / atrapa i wstrzykiwanie portów przez `AppOptions`.
- Rdzeń logiki — magistrala, rejestr, konfiguracja, sesje, wyszukiwanie, pamięć, transfer, router, dostawcy
  chmurowi, licznik kosztów, persony, harmonogram, wyzwalacze, środowisko agentek, klasyfikator ryzyka,
  środowisko wtyczek, `lib-*` — CI buduje i testuje go także na Ubuntu.
- Jedna powierzchnia poleceń i zdarzeń; interfejs z pełną atrapą działa w zwykłej przeglądarce.
- Format `.alfa` (ścieżki względne), katalog dostawców, zarządzanie procesami llama.cpp i whisper.cpp, lokalny
  kanał MCP (ma już gałąź dla gniazda Unix), ścieżki danych (`AppPaths` ma gałąź XDG).

**Potrzebny szew**

- Fabryka platform, protokół zdalny (pięć braków wyżej), generator typów TS.
- Magazyn sekretów i kluczy dla Linuksa i Androida.
- Semantyka ścieżek, powłok i tożsamości w kontraktach Brokera.
- Nowy rodzaj dowodu zgody (obszar pod przeglądem człowieka).
- Wymiar system / architektura w katalogu silników i modeli (`app-models/src/data.rs` zna tylko Windows x64).
- Zmienne środowiskowe wpisane na sztywno w binarkach Jądra, watchdoga i aktualizatora.
- Układ interfejsu na szerokość telefonu (dziś minimum okna to 400 × 500 px).

**Tylko Windows**

- Siedem crate'ów `platform-windows-*-impl`, `voice-audio-impl` (WASAPI) i z nim cały głos na żywo.
- Jądro poza procesem: usługa, potoki z DACL, okno Brokera na podwyższonej integralności, dowód fizycznego
  wejścia, skrót watchdoga.
- Launcher, aktualizator, instalator NSIS, proces wydania.
- Narzędzia sterowania komputerem: okna, UIA, wejście, ekran, Office, rejestr, OCR, terminal.

Drobiazg znaleziony przy przeglądzie: `crates/app-bridges/src/app.rs:227` wybiera `cmd` dla systemów innych
niż Windows — do poprawienia w etapie B.
