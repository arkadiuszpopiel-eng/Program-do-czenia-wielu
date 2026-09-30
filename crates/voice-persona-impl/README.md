# voice-persona-impl

Implementacja `voice-persona`:
- **Normalizator PL** (`normalize`, `PlNormalizer`): liczebniki główne 0–999 999 999 999 z rodzajem
  (dwie minuty, jedno zadanie) i dopełniaczem po przyimkach (od pięciu, około dwóch), większe liczby i zera
  wiodące cyfra po cyfrze; daty („1 października 2026” → „pierwszego października dwa tysiące dwudziestego
  szóstego roku”, `1.10.2026`, `2026-10-01`, zakresy dni), lata z „r./roku/rok” i po „w” (miejscownik),
  godziny z przypadkiem wg przyimka (o czternastej, przed dwunastą), zakresy godzin; waluty zł/PLN, $, USD,
  €, EUR, £ z groszami/centami i odmianą (1 złoty / 2 złote / 5 złotych), mnożniki tys./mln/mld; procenty;
  jednostki (km, m, cm, mm, kg, g, l, GB, MB, TB, kB, GHz, MHz, W, kW, ms, s, min, h, px, pkt, °C, km/h);
  zakresy liczb („pięć do dziesięciu minut”); wersje i IP („trzy kropka dwanaście”); telefony; skróty (np.,
  itd., itp., m.in., tzn., tj., tzw., wg, dr, prof., mgr, inż., ul., al., godz., nr, tel., pkt, zob., ds., ww.,
  jw., cdn., ew., św., br., ang., etc., p.n.e., n.e., ok. przed liczbą) z odmianą wg przyimka; URL-e („link do
  github kropka com”), e-maile („… małpa firma kropka pl”), domeny i nazwy plików; kod → „(kod na ekranie)”.
  Mieszany PL/EN zostaje bez zmian. W wyniku nigdy nie ma cyfr ASCII.
- **Słownik wymowy** — pierwszeństwo przed regułami (dopasowanie całych słów/fraz, najdłuższy klucz wygrywa);
  słownik wbudowany (`builtin_lexicon`) ze skrótowcami i nazwami własnymi.
- **Chunker** (`SentenceChunker`) — strumieniowo: koniec zdania, średnik, linia, blok kodu w całości;
  długie zdania na przecinku (> `max_chars`, pierwszy fragment krócej), bez przecinka na spacji; nie tnie
  po skrótach („np. to”), inicjałach ani w liczbach dziesiętnych; wynik niezależny od podziału strumienia.
- **Planista stylu** (`TablePlanner`, `table_for`) — biblia + znaczniki `[emocja:…] [tempo:…] [energia:…]`
  → tempo/wysokość/głośność/intensywność/znacznik emocji wg tabeli silnika (Pocket TTS, Piper, ElevenLabs,
  Azure, Cartesia, Generic); brak możliwości = pominięcie i wpis w `unsupported`, nigdy tekst tagu w mowie.
- **Plan** (`PersonaService::plan`) — markdown zdjęty, kod i tabele na ekran, zdania znormalizowane ze stylem.

Testy: kontrakt współdzielony, ≥ 120 przypadków tabelarycznych normalizatora, property-based (brak cyfr,
idempotencja, identyczność tekstu bez wzorców, niezmienność chunkera względem podziału strumienia).
