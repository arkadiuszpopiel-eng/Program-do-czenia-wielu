# voice-readaloud-impl

Implementacja `voice-readaloud` (docs/modules/voice-readaloud/SPEC.md):

- **`UiaTextSource`** — okno na pierwszym planie (`DesktopPort`); okna Alfy/Brokera i nieznanych procesów → odmowa
  (`TargetGuard`). Dokument: element z fokusem z `TextPattern` albo pierwszy dokument/pole edycji, `UiaPort::read_text`
  (port nie oddaje pól haseł). Zaznaczenie: `SelectionReader` (UIA `TextPattern.GetSelection` — port do dodania
  w `platform-contract`), a bez niego zapas **Ctrl+C**: schowek zapamiętany i wyczyszczony, `SendInput` Ctrl+C do okna celu
  (strażnik celów portu), odczyt, **przywrócenie poprzedniej zawartości zawsze**; zapas tylko gdy pole hasła jest
  wykluczone (fokus znany i nie-hasło albo w oknie brak pól haseł; błąd UIA = odmowa).
- **`ReadAloudService`** — `ReadAloudMachine` z kontraktu → `voice-tts` głosem wybranej agentki, `PrivacyTag::Private`
  (treść okna nie idzie do chmurowego TTS; `allow_cloud_tts` w konfiguracji), fragmenty do `OutputStream`, koniec
  zdania = `PlaybackFinished`; głośnik jako zasób `scheduler-lite` (`Holder::Persona`, `Priority::Interactive`) —
  zajęty → odmowa, odebrany (mowa użytkownika, kill-switch) → stop. Stop usuwa treść z pamięci.
- Treść niezaufana: zdarzenia bez tekstu; `share_with_model` tylko z `ShareConsent` (blok `<<<NIEZAUFANE`).

F5-11 (5 aplikacji: Notatnik, WordPad, przeglądarka, VS Code, Word) — self-hosted runner na Windows; CI: kontrakt na
`platform-fake` + `voice-tts-fake` + `voice-audio-fake` (wirtualny zegar).
