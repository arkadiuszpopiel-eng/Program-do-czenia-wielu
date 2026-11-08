# voice-readaloud-fake

Atrapa `voice-readaloud`: `ReadAloudMachine` z kontraktu + `FakeReadWorld` (okna w pamięci z dokumentem,
zaznaczeniem, polem hasła; wirtualny zegar) jako `TextSource` i `ReadDriver`; „mówienie” zdania trwa
60 ms/znak ÷ tempo. Te same odmowy (okna chronione — `TargetGuard`, pola haseł) co `-impl`.
