# lib-quick

Wspólna biblioteka (kategoria `lib-*`, bez logiki modułu): **polecenia wykonywane bez modelu**
(plan v6, P5). Alfa odpowiada na nie od razu, bez dostawcy, kosztu i tokenów — także offline.

| Polecenie      | Przykłady                                                                 |
| -------------- | ------------------------------------------------------------------------- |
| Rachunki       | „Ile to jest 7 razy 8?”, „oblicz (2+3)*4”, „20% z 150”, „pierwiastek z 81” |
| Literowanie    | „Przeliteruj rower”, „jak się literuje słowo żółw”                        |
| Losowanie      | „Rzuć kostką”, „orzeł czy reszka?”, „wylosuj liczbę od 1 do 10”           |
| Data i godzina | „Która godzina?”, „Jaki dziś dzień?”                                      |
| Quiz           | „Przepytaj mnie z tabliczki mnożenia przez 7” — 10 pytań, „koniec” przerywa |

## Zasady

- **Całe polecenie musi pasować.** Wzorce są zakotwiczone na początku i końcu wiadomości (po
  normalizacji: małe litery, bez polskich znaków, bez „hej Alfa,” i „proszę”); w rachunkach każde
  słowo musi być liczbą, działaniem albo wypełniaczem („ile”, „to”, „jest”, „oblicz”). Dlatego
  „ile kosztuje 5 jabłek”, „która godzina w Tokio” czy „jak się pisze podanie” idą do modelu.
- **Bez `eval`.** Działania liczy parser zstępujący na `f64`; wykładnik ≤ 32, wynik ≤ 10¹⁵,
  dzielenie przez zero i pierwiastek z liczby ujemnej mają własne odpowiedzi.
- **Bez akcji.** Biblioteka tylko liczy tekst odpowiedzi; zegar i źródło losowości podaje
  wywołujący (testy podstawiają stałe).
- **Quiz** (`quiz`) ma stan, który trzyma wywołujący — w Alfie jeden quiz na sesję, w pamięci;
  wiadomość, która nie jest odpowiedzią ani „koniec”, kończy quiz bez słowa i idzie dalej.
- Katalog `CATALOG` ma przykłady każdego polecenia — test sprawdza, że każdy trafia we własne
  polecenie, a lista zwykłych pytań nie trafia w żadne.

## Użycie w Alfie

`app-chat` (`src/quick.rs`) sprawdza ostatnią wiadomość użytkownika (tekst albo głos, bez
załączników) przed delegacją do modelu; odpowiedź jest zwykłą turą agentki bez `usage`, na osi czasu
„Bez modelu: <polecenie>”. Offline taka wiadomość nie trafia do kolejki. Wyłącza ustawienie
`sessions.local_commands` (Ustawienia → Sesje i okna → „Polecenia bez modelu”).
