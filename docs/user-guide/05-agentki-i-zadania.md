# Agentki, obsady, zadania i Marszałek

## Cztery agentki i ich role

| Agentka       | Charakter                         | Rola w obsadzie „Standard”                                |
| ------------- | --------------------------------- | --------------------------------------------------------- |
| **Alfa** (α)  | ciepła, spokojna, konkretna       | Dyrygentka i Mówczyni — prowadzi rozmowę, rozdziela pracę |
| **Beta** (β)  | pogodna, uporządkowana, troskliwa | Strażniczka pamięci i Pisarka                             |
| **Gama** (γ)  | rzeczowa, dociekliwa, wolniejsza  | Badaczka, Krytyczka i Myślicielka                         |
| **Delta** (δ) | energiczna, zwięzła, praktyczna   | Wykonawczyni (działa na komputerze) i Koderka             |

Role są niezależne od agentek i możesz je zmieniać w każdej sesji. Gotowe obsady: **Standard**, **Solo** (jedna
agentka do wszystkiego), **Kodowanie**, **Badania**.

- **Panel Agentki** (`Alt+1`): wybierz szablon obsady i kliknij „Zastosuj” albo przy agentce „Zmień role”. Zmiana
  działa od razu i trafia do dziennika. Panel pokazuje też, czym agentka zajmuje się teraz.
- Kliknięcie awatara w pasku tytułu otwiera szczegóły agentki; komenda `/obsada` — zmianę obsady.
- `@Delta` w wiadomości albo „Delta, …” głosem kieruje prośbę do wybranej agentki.

## Agentka, która działa na plikach

Żeby agentka mogła porządkować pliki czy uruchamiać polecenia, sesja potrzebuje **katalogu roboczego** — karta
„Katalog roboczy agentek” w panelu Agentki: **Wybierz katalog…**, **Katalog sesji** (`%USERPROFILE%\Alfa\Sesje\…`)
albo **Wyłącz narzędzia**. Bez katalogu agentki tylko odpowiadają. Alfa nie pozwoli wskazać katalogów z danymi Alfy
ani folderów z hasłami i kluczami.

W trakcie pracy:

- **kapsuła aktywności** nad polem wiadomości pokazuje, kto pracuje i na którym kroku („krok 3/7”), z przyciskiem
  **Stop**;
- wiadomość wysłana w trakcie trafia do agentki — uwzględni ją w następnym kroku;
- każda zmiana pliku przechodzi przez Brokera i dziennik cofania: po działaniu pojawia się toast **Cofnij** (8 s),
  a pod odpowiedzią karta „N kroków można cofnąć”;
- polecenie, którego Alfa sama nie uruchomi, dostaje kartę **Uruchom w terminalu**: „Kopiuj polecenie” i „Otwórz
  terminal” — uruchamiasz je Ty;
- karta **„czeka na zatwierdzenie”**: w tej wersji okna Brokera nie ma, więc po upływie czasu agentka dostaje
  odmowę i szuka innej drogi.

Limity zadania ustawisz w **Ustawienia → Agentki**: liczba kroków (domyślnie 40), czas (15 min), koszt, czas
czekania na zatwierdzenie, liczba zadań naraz (4) i „Gotowe” dopiero po samoweryfikacji.

## Replay — co agentka zrobiła

**Oś czasu** (`Alt+2`) → przełącznik **Replay**: lista przebiegów agentek, a w każdym kroki — narzędzie, wejście
i wynik, stan, czas. Przyciski: Od początku, Poprzedni, Następny, Wszystkie kroki oraz **Cofnij krok** i **Otwórz
terminal**. Wynik z treścią z zewnątrz (strona, plik, mail) jest oznaczony — agentka traktuje go jako dane, nie
polecenia. Pracę pomocniczą (zlecenie innej agentce, sprawdzenie przez Krytyczkę, umiejętność) widać jako podprzebiegi.

## Zadania (`Alt+7`)

Panel **Zadania** pokazuje drzewo zadań: podzadania, kolejność („Po: …”), stan, wynik, postęp, pochodzenie (od Ciebie,
od agentki, z wyzwalacza…) i koszt. Przy zadaniu: **Wiadomość dla agentki**, **Wstrzymaj** / **Wznów**, **Anuluj**
(razem z podzadaniami), **Ponów**. Na górze: **Nowe zadanie** (cel, opcjonalnie „po zadaniu …”). Zadanie zlecone
mostowi CLI ma dopisek „wynik niezweryfikowany przez Alfę”. `Ctrl+Shift+F12` zatrzymuje wszystkie zadania.

## Wyzwalacze

**Ustawienia → Zadania w tle i wyzwalacze** — zadania uruchamiane same:

- **Harmonogram** (wyrażenie cron, podgląd 5 najbliższych uruchomień w czasie polskim, ze zmianą czasu),
- **Co pewien czas** (co N minut),
- **Nowy plik w katalogu** (np. `*.pdf`; jeśli obserwacja katalogów nie jest dostępna, Alfa to napisze),
- **Ręcznie** („Uruchom teraz”).

Dla każdego wpisujesz, co ma zrobić agentka, i czy szanować „nie przeszkadzać”. Lista ma włącznik, „Uruchom teraz”,
„Usuń” i dziennik uruchomień. Wyzwalacz **nigdy nie uruchomi mostu CLI** bez Twojej zgody na karcie mostu.

## Marszałek — reguły, które tylko ograniczają

**Ustawienia → Reguły Marszałka.** Opisz słowami, czego agentkom nie wolno, np. „po 22:00 bez poleceń w terminalu”.
Marszałek zamienia to w reguły i pokazuje **podgląd zawężenia** (kiedy obowiązuje, co ogranicza, konflikty, odrzucone
szkice). Nic nie wchodzi w życie bez Twojego **Zatwierdź**. Reguła może tylko zawęzić uprawnienia — nigdy ich
rozszerzyć. Obowiązujące reguły cofniesz przyciskiem „Cofnij regułę”. Gdy model Marszałka jest niedostępny, reguły
wpiszesz w edytorze (JSON). Codziennie Marszałek przygotowuje **raport dnia** (powiadomienie).

## Kreator agentek

**Ustawienia → Kreator agentek.** Opisz nową agentkę słowami albo wypełnij formularz: imię (odmianę Alfa liczy sama),
charakter, kolor, głos (wysokość, tempo, odsłuch), rola i instrukcja, rodzaj pracy, narzędzia, „tylko odczyt”,
autonomia (najwyżej do bieżącego sufitu, nigdy L4), gdzie wolno zapisywać pliki, jak długo pamiętać. Potem **Pokaż
podgląd** (odmiana imienia w 7 przypadkach, instrukcja systemowa) → **Test na sucho** (co będzie dozwolone, o co
zapyta, co zablokowane) → **Zapisz agentkę** (dostępne tylko po zaliczonym teście tego samego podglądu). Gotowe
agentki są w bibliotece „Agentki z Kreatora”.

## Umiejętności

**Ustawienia → Umiejętności** — opisane przepisy na zadania. Propozycje (od agentek, z Twojej paczki albo z zewnątrz)
czekają w „Do przejrzenia”; umiejętności z zewnątrz trafiają do **kwarantanny** z uwagami skanera. Po **Przejrzyj**
widzisz zmiany, hash i wymagane narzędzia; instalujesz dokładnie tę wersję, którą przejrzałeś. Zainstalowaną
uruchomisz przyciskiem **Uruchom** — powstaje zadanie w bieżącej sesji. Są też **Eksportuj paczkę** /
**Importuj paczkę** i **Własna umiejętność**.
