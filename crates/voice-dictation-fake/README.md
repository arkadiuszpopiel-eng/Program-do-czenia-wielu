# voice-dictation-fake

Atrapa `voice-dictation`: `DictationMachine` z kontraktu na wirtualnych oknach w pamięci (`FakeWindows`: obraz procesu,
administrator, pole hasła, tekst, fokus) — te same reguły odmowy (`TargetGuard`, UIPI, pole hasła) i pauzy przy zmianie
okna co `-impl`, bez `platform-*`. Przechodzi testy kontraktowe (`FakeWindows` implementuje `DesktopDriver`).
