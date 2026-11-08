# Pamięć

Agentki pamiętają to, co ważne — ale zawsze możesz sprawdzić, **co** pamiętają i **dlaczego**, poprawić to albo kazać
zapomnieć.

## Zakresy i warstwy

| Zakres           | Kto korzysta                        |
| ---------------- | ----------------------------------- |
| Sesja (domyślny) | Tylko ta rozmowa                    |
| Projekt          | Sesje tego samego projektu          |
| Agentka          | Jedna agentka we wszystkich sesjach |
| Globalna         | Wszystkie sesje                     |

Warstwy: **robocza** (przypięte wpisy, zawsze w kontekście), **epizodyczna** (co się wydarzyło), **semantyczna**
(fakty), **proceduralna** (jak coś zrobić). Każdy zakres poza sesją ma własną zaszyfrowaną bazę.

## Jak coś zapamiętać

- Przy wiadomości: **Zapamiętaj** → w tej sesji, w projekcie, globalnie albo u agentki.
- Agentka sama może zaproponować zapis. Wpis spoza bieżącej sesji czeka na Twoje zatwierdzenie („Do zatwierdzenia”).
- Treść z zewnątrz (strona, plik, mail, schowek) jest **niezaufana**: zostaje tylko w zakresie sesji, nigdy nie jest
  zapisywana automatycznie i nie da się jej awansować do szerszego zakresu.

## Naucz Alfę

**Naucz Alfę** (paleta `Ctrl+K` albo przycisk w panelu **Pamięć**) dodaje do pamięci wiedzę, do której Alfa i agentki
sięgną później. Wybierz źródło:

- **Tekst** — wklej notatkę, fragment instrukcji, listę zasad;
- **Plik** — `.txt`, `.md`, `.pdf` albo `.docx` (do 25 MB; chronionych katalogów, danych Alfy i katalogów
  z poświadczeniami nie czytam). Przed zapisem widzisz nazwę pliku i liczbę znaków;
- **Ta rozmowa** — bieżąca gałąź rozmowy (bez ukrytych wiadomości i wyników narzędzi): Twoje wiadomości, a przy
  zakresie „Ta rozmowa” także odpowiedzi agentek.
- Plik wybierasz jeden naraz — zaznaczenie kilku plików kończy się prośbą o wybór jednego.

Treść dzielę na krótkie fakty (akapity, potem zdania; do 800 znaków na fakt, najwyżej 128 faktów i 100 000 znaków
treści — dłuższa zostaje ucięta, powtórzenia pomijam). Okno pokazuje, ile faktów zapiszę. Opcjonalny **tytuł** trafia
na początek każdego faktu (np. „[Wyjazd · 1/3] …”), a przy faktach zostaje **źródło**: nazwa pliku albo „wklejony tekst”
ze skrótem treści (SHA-256) albo identyfikator rozmowy. Wpisy są dodane przez Ciebie, więc od razu aktywne.

**Tekst i plik to treść z zewnątrz** — traktuję je jak załączniki: jako dane, nigdy jako polecenia. Zapisuję je tylko
w pamięci tej rozmowy z oznaczeniem „niezaufane” (agentki widzą to oznaczenie), bez awansu do szerszego zakresu.
**Rozmowę** możesz zapisać w tej rozmowie, w projekcie albo globalnie — z wyjątkiem rozmowy prywatnej, która nie
zasila szerszych zakresów. Odpowiedzi agentek często powtarzają treść z zewnątrz (strony, wyniki narzędzi,
załączniki), więc zapisuję je tylko w pamięci tej rozmowy i jako „niezaufane”; do projektu i pamięci globalnej trafiają
wyłącznie Twoje wiadomości. Tekst dłuższy niż 4 MB odrzucam od razu — podziel go na części.

Po zapisie okno pokazuje wynik (liczba faktów, zakres, źródło) i ma **Cofnij naukę** — zapisane fakty znikają
kaskadowo, a formularz wraca pusty. Jeśli części faktów nie udało się usunąć, wynik zostaje na ekranie i ponowne
**Cofnij naukę** próbuje usunąć pozostałe. Pojedyncze fakty usuniesz też później w Inspektorze (**Zapomnij**), a usunięcie rozmowy zabiera wiedzę
z niej zapisaną.

## Inspektor pamięci (`Alt+4`)

Panel **Pamięć** to Inspektor:

- wyszukiwarka i filtry: zakres, warstwa, zaufanie (zaufane / niezaufane), stan (aktywne, do zatwierdzenia,
  zastąpione, wygasłe), przypięte;
- **Dlaczego to pamiętam** — skąd wpis się wziął (Ty, agentka, treść niezaufana, import), źródła, historia wersji,
  co z niego wywiedziono, pewność, data wygaśnięcia;
- **Edytuj** — zapis tworzy nową wersję, stara zostaje w historii jako „zastąpiona”;
- **Przypnij** / **Odepnij**, **Zatwierdź** (propozycje), **Awansuj** (kopia do szerszego zakresu, za Twoją zgodą);
- **Dziennik zmian** zakresu z przyciskiem **Cofnij** przy zmianach (Twoich i Strażniczki).

## Zapominanie

**Zapomnij** najpierw pokazuje, **co zniknie**: wpis, jego wersje i wszystko, co z niego wywiedziono (np. streszczenia),
oraz co wróci do aktywnych (starsza wersja). Po „Zapomnij na zawsze” wpis znika ze wszystkich miejsc: z indeksu
wyszukiwania, z wektorów, z dziennika i z pamięci podręcznej. **Zapomnij cały zakres** niszczy kryptograficznie bazę
tego zakresu. Usunięcie sesji najpierw usuwa jej kopie w szerszych zakresach.

## Porządkowanie (Strażniczka pamięci)

**Ustawienia → Pamięć**:

- **Porządkowanie nocne** (domyślnie włączone, okno 02:00–05:00) — Strażniczka łączy duplikaty, streszcza, wygasza
  stare wpisy i proponuje fakty. Nie startuje na baterii ani w trybie gry. Każdą zmianę widać w dzienniku z „Cofnij”.
- **Porządkuj teraz** — uruchomienie od razu.
- **Fakty z rozmów**: „Proponuj do zatwierdzenia” (domyślnie), „Zapisuj sama” albo „Wyłączone”.
- Lista zakresów z liczbą wpisów i przyciskiem „Zapomnij cały zakres”.

Bez modelu lokalnego porządkowanie działa tylko na regułach (bez propozycji nowych faktów).

## Ograniczenie tej wersji

Wyszukiwanie w pamięci opiera się na słowach (z obsługą polskich końcówek), a nie na znaczeniu — model rozumiejący
znaczenie (embedder semantyczny) jest **jeszcze niedostępny**. Pytaj więc słowami, które padły w zapamiętanej treści.
