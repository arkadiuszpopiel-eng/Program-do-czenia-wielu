# voice-readaloud-contract

Kontrakt czytania na głos (docs/modules/voice-readaloud/SPEC.md): traity `TextSource` (zaznaczenie / dokument okna na
pierwszym planie), `SelectionReader` (port zaznaczenia UIA — do dodania w `platform-contract`), `ReadAloud` (start,
krok, sterowanie, udostępnienie modelowi za zgodą); `segment` (zdania PL z offsetami: skróty, liczby, inicjały,
cudzysłowy, długie zdania dzielone), `ReadAloudMachine` (pauza, wznów, dalej, wstecz, szybciej/wolniej 0,5–2,0, od
początku, stop czyści treść), `UntrustedText` (bez `Display`; do modelu tylko `for_model(Some(zgoda))` w bloku
niezaufanym). Zdarzenia `voice.readaloud.*` bez treści.
