# artifacts — SPEC (szkic v0)

## Cel
Oddawanie plików przez agentki i wejście plików do sesji: karty plików (nazwa, ścieżka, rozmiar, podgląd/diff, wersje), akcje Otwórz · Pokaż w Eksploratorze · Kopiuj jako plik · Zapisz jako · Spakuj · Wyślij (MCP) · Przekaż do innego czatu, przeciąganie do Eksploratora; wejście: drag&drop, wklejanie, zrzut, montowanie folderu, „Otwórz w Alfie" (PLAN §11).

## Fala i priorytet
F1 (panel Artefakty, karty, podglądy podstawowe). P0. Podgląd Office tekstowy — P1; Wyślij (MCP) — F4.

## Kontrakt (szkic Rust)
```rust
// artifacts-contract — SZKIC
pub struct Artifact { pub id: ArtifactId, pub session: SessionId, pub path: PathBuf, pub name: String,
    pub bytes: u64, pub mime: String, pub versions: Vec<ArtifactVersion>, pub origin: Origin /* Agent(PersonaId) | User | Import */,
    pub preview: PreviewKind /* Image | Pdf | Text { lang } | Audio | Video | Markdown | Html { sandboxed } | None */ }
pub trait Artifacts: Send + Sync {
    fn register(&self, session: SessionId, path: &Path, origin: Origin) -> Result<Artifact>;
    fn attach_input(&self, session: SessionId, input: InputFile) -> Result<Artifact>;
    fn new_version(&self, id: ArtifactId, path: &Path) -> Result<ArtifactVersion>;
    fn diff(&self, id: ArtifactId, a: VersionId, b: VersionId) -> Result<Diff>;
    fn export(&self, id: ArtifactId, action: ExportAction) -> Result<()>;   // SaveAs | Zip | CopyAsFile | RevealInExplorer | HandoffTo(SessionId)
}
```
Zdarzenia: `artifact.registered`, `artifact.version.added`, `artifact.exported`, `artifact.handoff`, `artifact.preview.blocked` (deny-lista / za duży).

## Zależności
`core-bus/config/log-contract`, `sessions-contract`, `platform-windows-contract` (Eksplorator, schowek plików, ścieżki), `undo-journal-contract` (F3).

## Niezmienniki
- Domyślny katalog wyjściowy `%USERPROFILE%\Alfa\Sesje\<nazwa>\out`; artefakt poza katalogiem sesji wymaga zakresu `fs.write` z tokenu agentki.
- Duże pliki nigdy przez IPC do UI — podgląd przez protokół zasobów/ścieżkę (PLAN §14.7).
- Aktywne artefakty HTML/SVG renderowane wyłącznie w sandboxowanym iframe bez IPC, jeden na podgląd (PLAN §8.2); Markdown renderowany w Rust z sanitizacją.
- Wersje artefaktów są niezmienne; „nowa wersja" = nowy plik/hash.
- Przekazanie do innego czatu = kopia + jawna tura `Handoff` w sesji docelowej.

## Zdolności / uprawnienia
`fs.read/write` w katalogu sesji (jądro); akcje użytkownika (Zapisz jako, Otwórz) wykonywane jako Ty, nie jako agentka.

## Izolacja
`inproc`, `lazy` (panel ładowany leniwie).

## Budżet zasobów
RAM ≤ 6 MB + miniatury (cache ≤ 50 MB na dysku); podgląd tekstu 1 MB ≤ 100 ms; miniatury obrazów generowane leniwie.

## Konfiguracja (klucze TOML)
`[artifacts] out_dir = "%USERPROFILE%\\Alfa\\Sesje\\{session}\\out"`, `preview_max_mb = 20`, `thumb_cache_mb = 50`, `keep_versions = 10`.

## Wkład do UI
Panel Pliki/Artefakty (karty, podgląd, diff, wersje, akcje), podglądy plików (obrazy, PDF, tekst/kod, audio/wideo, Markdown), linki do plików w wiadomościach, blok kodu „zapisz jako plik", makieta 8.

## Testy akceptacyjne
- `ACC-F1-artifacts-01`: test kontraktowy register/version/diff/export na `-fake`/`-impl`.
- `ACC-F1-artifacts-02`: E2E — agentka (fake LLM) oddaje plik → karta w panelu → Zapisz jako / Pokaż w Eksploratorze (runner Windows).
- `ACC-F1-artifacts-03`: artefakt HTML wykonujący skrypt nie ma dostępu do IPC (test bezpieczeństwa iframe).

## Fake
`artifacts-fake`: rejestr w pamięci na wirtualnym FS z `platform-windows-fake`, podglądy z fixture'ów, brak Eksploratora.

## Otwarte pytania
- Silnik podglądu PDF w WebView2 (wbudowany vs pdf.js) — do ustalenia w SPEC v1 z budżetem JS.
- Kopiowanie „jako plik" do schowka (CF_HDROP) — potwierdzić w spike (j).

## Zmiany po implementacji (F1, `artifacts-contract/-impl/-fake`, 2026-09-30)
- Kontrakt synchroniczny: `out_dir(dir_name)` (`<root>\Sesje\<nazwa>\out`, `root` wstrzykiwany, `dir_name` =
  nazwa katalogu roboczego sesji), `register(session, path, origin, source_turn)`, `add_version(session, id,
  path, source_turn)`, `get`, `list`, `preview(session, id, version, max_bytes)`, `diff(session, id, from,
  to)`, `intent(session, id, version, action) -> ArtifactIntent` zamiast `export(...) -> ()` — **akcje UI
  (Open, Reveal, CopyAsFile, SaveAs, Zip, SendToSession) to intencje**; wykonuje je `platform-windows` jako
  użytkownik (intencja niesie SHA-256 wersji do sprawdzenia). `attach_input` (wejście plików) — później.
- Wersje niezmienne (wyzwalacze w bazie); ta sama ścieżka → nowa wersja istniejącego artefaktu; identyczny
  hash → bez nowej wersji. Migawka treści wersji ≤ 1 MiB jako BLOB w **szyfrowanej bazie sesji** (podgląd i
  diff działają po nadpisaniu pliku; usunięcie sesji kasuje rejestr i migawki). Większe pliki: tylko najnowsza
  wersja z dysku, gdy rozmiar się nie zmienił (`ContentUnavailable` w pozostałych przypadkach).
- `Preview::{Text { text, truncated }, Binary { mime, bytes }}` (NUL lub błędny UTF-8 w pierwszych 8 KiB →
  binarny); rodzaj podglądu (obraz, PDF, audio…) UI wyprowadza z MIME. `TextDiff` — linie z numerami +
  format ujednolicony (crate `similar`).
- Atrapa czyta prawdziwe pliki (katalog tymczasowy w testach) zamiast wirtualnego FS `platform-fake`;
  zamiast Eksploratora rejestruje intencje.
- Zdarzenia: `artifact.registered`, `artifact.version.added`, `artifact.exported`, `artifact.handoff`.
- Pomiar: podgląd 1 MiB tekstu z pliku 2,3 MB — ~3 ms (budżet 100 ms).
