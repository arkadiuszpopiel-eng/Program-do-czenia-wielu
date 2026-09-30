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
