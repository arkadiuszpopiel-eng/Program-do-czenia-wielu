# platform-linux — SPEC (szkic v0, planowany — plan v6)

## Cel
Implementacje portów systemowych dla Linuksa — drugiego systemu dla hosta i interfejsu: pliki z koszem,
procesy, schowek, sekrety, skróty globalne, zasobnik, okna, audio. Kod specyficzny dla Linuksa żyje tylko
w crate'ach `platform-linux-*`. Moduł **nie** zmienia kontraktów Jądra (to ADR 0018) i w v0 **nie** dostarcza
sterowania komputerem, Office ani rejestru. Stan: propozycja, crate'y nie istnieją.

## Fala i priorytet
Plan v6, **etap G** (zadania G.1, G.5, G.6; powiązane G.2–G.4, G.7–G.9), po B.4 i po podpisie ADR 0017–0018.
Priorytet: do ustalenia przez właściciela. v0 (G.1): pliki z koszem, procesy, schowek, sekrety (Secret Service).
v1: audio PipeWire i głos (G.5), skróty, zasobnik, okna przez portale XDG (G.6).
Badawcze, poza zobowiązaniem: okno zgód i dowód fizycznego wejścia (G.4), sterowanie komputerem (G.9).

## Kontrakt (szkic Rust)
```rust
// szkic — porty są już w platform-contract; ten moduł daje tylko implementacje
pub fn platform_linux() -> PlatformPorts;            // wołane z platform-select (cfg(target_os = "linux"))
pub struct LinuxFs;        // impl FsPort: write_atomic = tmp + rename, usuwanie do kosza systemowego
pub struct LinuxProcesses; // impl ProcessPort / ExecPort: drzewo procesów zabijane w całości
pub struct LinuxClipboard; // impl ClipboardPort
pub struct LinuxSecrets;   // impl SecretStore: Secret Service
pub struct LinuxHotkeys;   // impl HotkeyPort  (v1, portale XDG)
pub struct LinuxTray;      // impl TrayPort    (v1)
pub struct PipewireAudio;  // impl AudioIo     (v1)
// uia / input / screen: None w v0 → zdolność „sterowanie komputerem” wyłączona
```
Zdarzenia: te same nazwy `platform.*` co na Windows (publikuje `app-*`); nowych nie dodaje.

## Zależności
`platform-contract` oraz kontrakty z `AudioIo`, `SecretStore`, `KeyVault`, `ShellPort`; `platform-select`.
Zewnętrzne (np. `zbus`) dozwolone tylko w `platform-linux-*` (reguła w `deny.toml`); wybór i wersje —
`docs/vendor/` przed kodem.

## Niezmienniki
- Poza `platform-linux-*` brak zależności systemowych Linuksa w workspace (test grafu zależności).
- Testy kontraktowe portów przechodzą tak samo na `platform-fake`, Windows i Linuksie.
- Usuwanie domyślnie do kosza; `write_atomic` = plik tymczasowy + zmiana nazwy.
- Lista zakazów ścieżek poświadczeń (`~/.claude`, `~/.codex`, profile przeglądarek, magazyn sekretów) jest
  sprawdzana także tutaj jako ostatnia linia, na postaci surowej i kanonicznej (dowiązania symboliczne).
- Ścieżki POSIX rozróżniają wielkość liter — żadnego sprowadzania do małych liter (`PathFlavor::Posix`, ADR 0018).
- Skrót kill-switch i reguła skrótów AltGr obowiązują bez zmian.
- Port nieobecny zwraca `Unsupported`; narzędzie powłoki jest wyłączone do czasu analizatora POSIX (G.2).
- Sekrety tylko w magazynie systemu; brak magazynu = odmowa, nie zapis do pliku.

## Zdolności / uprawnienia
Moduł sam nie żąda tokenów — jest używany przez moduły narzędzi pod kontrolą Brokera. Uprawnienia systemu:
dostęp do magistrali sesji i portali XDG; nic wymagającego uprawnień administratora w v0.

## Izolacja
`inproc` w procesie hosta, `always` (podstawa) i `lazy` (audio, zasobnik). Audio: wątek RT bez alokacji, jak
na Windows. Jądro poza procesem na Linuksie (usługa użytkownika, gniazdo Unix z kontrolą UID) — osobne zadanie G.3.

## Budżet zasobów
RAM i CPU portów, opóźnienie audio na PipeWire — do pomiaru na Ubuntu LTS z kartą NVIDIA; progi ustala właściciel.

## Konfiguracja (klucze TOML)
Ścieżki przez `AppPaths` (gałąź XDG już istnieje). Szkic kluczy per maszyna: `[machine.platform.linux]
secret_backend = "secret-service"`, `tray = true`. Wartości i nazwy — do ustalenia.

## Wkład do UI
Brak własnych ekranów. Interfejs ukrywa funkcje spoza mapy zdolności (Office, rejestr, sterowanie komputerem);
w Ustawieniach informacja, czego host na Linuksie nie potrafi.

## Testy akceptacyjne
- `ACC-V6-platform-linux-01` — testy kontraktowe portów v0 zielone na Ubuntu w CI.
- `ACC-V6-platform-linux-02` — cofalność losowych operacji plikowych jak na Windows (liczba prób i próg — ustala właściciel).
- `ACC-V6-platform-linux-03` — 0 odczytów ze ścieżek z listy zakazów, także przez dowiązania (N prób; N ustala właściciel).
- `ACC-V6-platform-linux-04` — zdolności niedostępne są ukryte, nie zepsute (przegląd interfejsu na hoście Linux).
- `ACC-V6-platform-linux-05` — scenariusz MVP 6/6 na Ubuntu LTS z kartą NVIDIA (wyjście etapu G; przebieg na sprzęcie).
- `ACC-V6-platform-linux-06` — opóźnienie głosu na PipeWire poniżej progu (v1; próg ustala właściciel).
- `ACC-V6-platform-linux-07` — interfejs na WebKitGTK z kartą NVIDIA renderuje się poprawnie w CI (G.8).

## Fake
Istniejący `platform-fake` — bez zmian; testy kontraktowe uruchamiane na atrapie i na implementacji Linux.
Dla audio: atrapa z wirtualnym zegarem jak dla WASAPI.

## Otwarte pytania
- ADR 0017 — kolejność: Linux jako czwarty; ADR 0018 — `PathFlavor`, `PathEnv`, analizator POSIX i tożsamość
  procesu (UID + `SO_PEERCRED`) muszą być rozstrzygnięte przed powłoką i Jądrem poza procesem.
- Okno zgód i dowód fizycznego wejścia: Wayland nie daje prostego odpowiednika (G.4, badawcze) — do tego czasu
  sposób zatwierdzania na hoście Linux jest nieustalony; ADR 0022 (dowód urządzenia) może być jedyną drogą.
- Podział na crate'y (`platform-linux-impl`, `-gui-impl`, `-audio-impl`…) pod limit 8 000 linii — do ustalenia.
- Zakres dystrybucji i środowisk graficznych; pakiety deb, rpm, potem Flatpak (G.8) a dostęp do portali.
- Silniki llama.cpp i whisper.cpp dla Linuksa (G.7) zależą od katalogu z wymiarem system / architektura (B.6).
