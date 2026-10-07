# voice-dictation-impl

Implementacja `voice-dictation` (docs/modules/voice-dictation/SPEC.md):

- **`DictationService`** (porty `DesktopPort` + `UiaPort` + `InputPort` z `platform-contract`): cel = okno na pierwszym
  planie w chwili startu; odmowa dla okien Alfy/Brokera (`TargetGuard`), administratora (UIPI) i pól haseł — UIA:
  element z fokusem `IsPassword`, a gdy fokusu nie da się ustalić — jakiekolwiek pole hasła w oknie albo błąd UIA =
  odmowa (**fail-closed**). Tekst idzie przez `InputPort::send` w porcjach ≤ 8 jednostek UTF-16 — każda porcja to jedna
  atomowa paczka `SendInput` (strażnik celów, fizyczne wejście i fokus sprawdzane przed każdą paczką), więc po
  przerwaniu wiadomo dokładnie, co wpisano; reszta czeka na powrót do okna. Wejście użytkownika → pauza i wznowienie po
  1,5 s. „cofnij to” → Backspace (tylko ostatnia fraza, ≤ 30 s, bez zmiany okna). Anulowanie: `cancel()` (kill-switch).
- **`DictationRunner`**: mikrofon jako zasób wyłączny `scheduler-lite` (`lease_now`, `Holder::User`), resampling do
  16 kHz, PTT (jedna wypowiedź na przytrzymanie) albo przełącznik (VAD + pre-roll 300 ms), `Stt` → `on_final`;
  odebranie dzierżawy (kill-switch) kończy sesję.
- Zdarzenia `voice.dictation.*` bez treści; `VoiceDictationModule` publikuje je na magistralę.

F5-10 (5 aplikacji: Notatnik, WordPad, przeglądarka, VS Code, Word; ≥ 95% zgodności) — self-hosted runner na
Windows (`platform-windows-gui-impl`); CI: normalizacja (`evals/F5/voice/dictation-cases.json`) i property „0 wpisów
poza oknem celu / do okien chronionych” na `platform-fake`.
