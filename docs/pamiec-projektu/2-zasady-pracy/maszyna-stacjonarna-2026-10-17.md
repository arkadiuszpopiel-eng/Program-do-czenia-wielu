---
name: maszyna-stacjonarna-2026-10-17
description: "Projekt Alfa działa na zmianę na dwóch komputerach (dysk E: przenoszony): stacjonarny PC (Ryzen 5700X3D, RX 9070 XT, 32 GB) od 17.10 i laptop (i7-13620H, RTX 4050 + Intel UHD, 16 GB) znów od 18.10 — różnice i skutki"
metadata:
  node_type: memory
  type: project
  originSessionId: 8c1892ed-2987-4f4f-91ab-0abe9bb3c4b8
  modified: 2026-10-22T09:23:38.764Z
---

Od 2026-10-17 dysk E: (ADATA SX8200 na USB) z projektem jest podpięty do stacjonarnego PC DESKTOP-S02O9OH
(AMD Ryzen 7 5700X3D, AMD Radeon RX 9070 XT, 32 GB RAM, konto Spopielony). Wcześniej: laptop i7-13620H, RTX 4050.

Skutki stwierdzone przy analizie 2026-10-17:
- Pliki należą do SID ze starej instalacji → git wymaga `-c safe.directory=*` (globalnej konfiguracji nie zmieniano).
- 1447 plików w starym `node_modules` miało EPERM; naprawione 2026-10-17 (`pnpm install` z lockfile do nowego katalogu). Resztki w `Alfa AI Program/stare-node_modules-acl-2026-10-17` (1447 plików starego SID) usunie tylko właściciel z prawami administratora.
- `cargo check --workspace --offline --all-targets` z `portable-env.ps1` (Rust 1.94.1) zielony w 2 min 15 s.
- Brak `%LOCALAPPDATA%\Alfa` (modele, silniki, profil), dysk F: (Kopie Plików, Realtek RTL9210B) podłączany okresowo, brak `gh`, brak Python313 z launch.json.
- GPU AMD: silniki pobrane na laptopie były w wariancie CUDA (whisper) — tu nie zadziałają bez innego wariantu.

2026-10-17 polecenie właściciela: Alfa ma działać na NVIDIA, AMD, Intel i iGPU — zadanie C6.6 w roadmapie
(commit 6ff0c311 + seria 0.0.63: Qwen3-ASR przez Vulkan na AMD/Intel, DirectML dla Supertonic, ADR 0028 proponowany).
node_modules zainstalowane z lockfile (właściciel zgodził się na pobranie brakujących pakietów).
`cargo fmt --all` pada na Windows (os error 206) — używać `rustfmt --edition 2024 <pliki>`.

Pamięć asystenta na tym PC była pusta — zasady pracy z laptopa (alfa-*.md) przywrócone 2026-10-18 z kopii
w repozytorium (`alfa/docs/pamiec-projektu/2-zasady-pracy`), bo `memory-sync.sh` jest lustrem i skasowałby je z repo.

2026-10-18 po południu dysk E: wrócił do laptopa DESKTOP-BHEJQ2D (i7-13620H, 16 GB, RTX 4050 6 GB + Intel UHD
Graphics). Na laptopie: git działa bez `safe.directory`, są `jq`, `gh`, Edge i `%LOCALAPPDATA%\Alfa` (modele, silniki
CUDA), cargo z `-j 3` (16 GB RAM). Dysk F: na laptopie to teraz INNY dysk („Dane”, 1,9 TB, zapełniony plikami
testu pojemności CAPACITYTESTER) — kopie `F:\Alfa\kopia-…` z PC stacjonarnego są na tamtejszym F: („Kopie Plików”)
i stąd nieosiągalne; `scripts/backup.sh` z domyślnym celem `/f/Alfa` NIE uruchamiać, dopóki właściciel nie wskaże
dysku na kopie. Laptop z iGPU + NVIDIA to dokładnie przypadek C6.6 niesprawdzony na PC (raport 28).

2026-10-19 dysk E: znów na PC stacjonarnym (przekazanie: `PRZEKAZANIE-NA-PC-2026-10-19.md`). Na PC: git tylko
z `safe.directory` (także skrypty node — `GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=safe.directory GIT_CONFIG_VALUE_0=*`),
brak Edge (testy w przeglądarce bez `PW_CHANNEL`, na Chromium Playwrighta), brak pobranego ONNX Runtime w
`%LOCALAPPDATA%\Alfa` (verify-app bierze bibliotekę z przypiętą sumą z kopii właściciela), F: = „Kopie Plików” z
`F:\Alfa\kopia-…`. 2026-10-21 `memory-sync.sh` nadpisał w repozytorium ten plik i zasadę -j6/-j3 starszymi
wersjami z pamięci asystenta na PC — przywrócone; skrypt od 21.10 wstrzymuje taką kopię (kod 3).

**Why:** pamięć i raporty były pisane raz na laptopie, raz na PC — reguły jednej maszyny (np. `-j 6`, brak `jq`,
WSL jako `bash`) na drugiej są błędne albo groźne (16 GB RAM).
**How to apply:** na początku sesji sprawdź maszynę (procesor, karty: `Get-CimInstance Win32_Processor,
Win32_VideoController`) i stosuj jej reguły; przed `verify sprzet` ustal z właścicielem, czy ta maszyna jest
docelowa; nie porównuj czasów między maszynami.

**Mosty CLI na PC (2026-10-22):** aplikacje okienkowe Claude i OpenAI Codex (Sklep, MSIX) noszą własne `claude.exe`/`codex.exe`
poza PATH (`%APPDATA%\Claude\claude-code\<wersja>\…`, `WindowsApps\OpenAI.Codex_…\app\resources`), więc Alfa (szuka tylko
w PATH + PATHEXT) pokazywała „nie zainstalowane”. Na prośbę właściciela zainstalowane oficjalne CLI
`@anthropic-ai/claude-code@2.1.308` i `@openai/codex@0.163.0` → `%APPDATA%\npm\claude.cmd`, `codex.cmd` (w PATH użytkownika).
Antigravity odinstalowane (zostały `.antigravity`, pusty `D:\Antigravity`).

**PUŁAPKA — sesja Claude Code w aplikacji Claude (MSIX) ma zwirtualizowane `%APPDATA%`/`%LOCALAPPDATA%`:** pierwszy
`npm install -g` z tej sesji trafił do prywatnej kopii pakietu (`%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\
Roaming\npm`) — widoczny tylko wewnątrz aplikacji Claude; Alfa ze skrótu (i Eksplorator) go nie widziała, a emulacja z tej
sesji błędnie mówiła „działa”. Instalacje i zapisy do AppData dla właściciela uruchamiać POZA kontenerem:
`Start-Process explorer.exe -ArgumentList '"<skrypt>.cmd"'` (Eksplorator uruchamia skrypt w zwykłym środowisku); tak samo
sprawdzać, co widzi Alfa ze skrótu (instancja testowa przez `.cmd` z Eksploratora). Pliki poza AppData (E:, F:, `~\.claude`)
nie są wirtualizowane. Druga instalacja (11:20, przez Eksplorator) — Alfa uruchomiona przez Eksplorator wykrywa oba CLI
i otwiera okna logowania (sprawdzone przez CDP na instancji testowej).

**Why:** przez wirtualizację właściciel dostał „zainstalowane, działa”, a w Alfie dalej było „nie zainstalowane”.
**How to apply:** każda zmiana w `%APPDATA%`/`%LOCALAPPDATA%` dla programów właściciela i każda weryfikacja „co widzi Alfa”
— przez proces uruchomiony z Eksploratora, nie bezpośrednio z tej sesji.
