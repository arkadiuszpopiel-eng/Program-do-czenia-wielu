# Decyzje właściciela — 12.11.2026

Zapis otrzymania odpowiedzi: **2026-11-12T03:52:57.502784+00:00**. To zapis zgody projektowej
z rozmowy, nie kryptograficzny podpis urządzenia lub odbiór wdrożenia.

## D.0 — podpis ADR0019–0023

Na pytanie o zatwierdzenie konkretnego projektu jako doprecyzowania i podpisu
ADR0019–0023 właściciel odpowiedział: **„Zatwierdzam opisany projekt”**.

Zatwierdzony dokument: [pakiet projektu z niezmienionej bazy372bfcd5](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/blob/372bfcd550f1a4f37a48580d050bc20de28ca270/docs/reviews/2026-11-11-remote-design-review.md),
SHA256 `e2dec7f9ce0fa2cde5b62abe5f5f886931aa3bf00fcafcfbbf9e115cab6b07d6`. To ten sam plik, który był podlinkowany w pytaniu właścicielowi.

Zakres: własny przekaźnik WSS z E2E, proponowany NoiseXX25519ChaChaPolySHA256 dla
klienta natywnego, pierwsze parowanie przy komputerze, osobny dowód każdej zgody
weryfikowany przez Brokera, panel WWW bez przechowywania klucza API.
ADR0019–0023 otrzymują status zatwierdzonego projektu z tym doprecyzowaniem.
D.0 jest spełnione, można przygotować niezależne kontrakty/fake/testy i dalszy kod.

**Konkretne poprawki Brokera nadal osobno do przeglądu.** Nie zmieniono AGENTS.md,
chronionych modułów, polityk lub zamrożonych evals. Podpis nie ustala nowych progów
ramki/bufora/opóźnienia/odwołania i nie wdraża bramy, parowania lub usługi relay.
Nie obejmuje publikacji panelu, kosztów hostingu lub automatycznego odczytu kluczy.
Magazyn i ewentualne przekazanie klucza na Android wymagają odrębnej zgody/polityki.
Przed włączeniem pozostają przegląd implementacji i odbiór prawdziwego telefonu.

## #78 — pierwsza ścieżka WWW

Na pytanie o pierwszą drogę wyszukiwania właściciel wybrał:
**„Wyszukiwanie w przeglądarce bez klucza API”**.

Pierwszy przyrost ma więc obsłużyć interaktywną nawigację/wyszukanie i źródła w
przeglądarce. Nie wymaga konfiguracji konta API. Rzeczywisty adapter SearchPort jest
osobnym późniejszym zakresem, a nie warunkiem tej ścieżki. ADR0031 nadal opisuje
projekt WebView2; ten wybór nie jest dowodem gotowego embeddingu lub pomiaru pamięci.
Nie pobrano kluczy/tokenów CLI i nie wysłano prywatnych zapytań do przypadkowych usług.
