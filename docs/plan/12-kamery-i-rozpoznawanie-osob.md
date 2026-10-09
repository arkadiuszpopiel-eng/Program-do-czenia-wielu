# 12 — Kamery, rozpoznawanie osób i opis obrazu

Zlecenie właściciela z 2026-10-09. Ten dokument opisuje projekt — **nic z niego nie jest jeszcze zbudowane**.
Zadania są w [`03-roadmapa.md`](03-roadmapa.md) (grupa C3), technologie z etykietami pewności w
[`11-technologie-2026-10.md`](11-technologie-2026-10.md) (rozdział 10). Kod rozpoznawania osób powstanie dopiero
po podpisaniu [ADR 0025](../ADR/0025-kamery-i-biometria-twarzy.md), bo dotyczy danych biometrycznych.

## Co ma umieć Alfa (słowami właściciela)

1. Obsługa kamer **IP, Wi-Fi i USB**, **kilku naraz**, przez osobny system do tego.
2. **Rozpoznawanie zapisanego użytkownika albo kilku użytkowników** — z pamięcią długoterminową: po ponownym
   uruchomieniu Alfa nadal wie, kto jest kim.
3. Pamięć **tego, o czym z daną osobą rozmawiała**.
4. **Rozpoznawanie tego, co jest na kamerze**: na „opisz mi to, co widzisz” model opisuje obraz.

## Jak to wygląda dla użytkownika

| Sytuacja                               | Co robi Alfa                                                                                                                                                                 |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| „Opisz mi to, co widzisz”              | bierze bieżącą klatkę z kamery aktywnej (albo wskazanej: „…na kamerze w kuchni”), opisuje ją po polsku; w rozmowie zostaje miniatura klatki, żeby było widać, na co patrzyła |
| Siadasz przed komputerem               | rozpoznaje Cię po twarzy, wita po imieniu i ma pod ręką pamięć rozmów z Tobą; nic nie mówi, jeśli wyłączysz powitania                                                        |
| Przed kamerą siada inna zapisana osoba | przełącza „rozmówcę”: zwraca się do niej po imieniu i korzysta z jej pamięci, nie z Twojej                                                                                   |
| Przed kamerą jest ktoś nieznany        | traktuje go jako gościa: nie ujawnia pamięci osób zapisanych; może zaproponować zapisanie — tylko za wyraźną zgodą tej osoby                                                 |
| „Kto jest przy drzwiach?”              | opisuje obraz z kamery nazwanej „drzwi”; jeśli rozpoznaje zapisaną osobę, mówi kto to                                                                                        |
| „Co się zmieniło w pokoju od rana?”    | porównuje bieżącą klatkę z zapisanym opisem (bez przechowywania nagrań — patrz prywatność)                                                                                   |
| Chcesz spokoju                         | jeden przełącznik wyłącza wszystkie kamery; skrót awaryjny `Ctrl+Shift+F12` też je zatrzymuje                                                                                |

W belce okna pojawia się wskaźnik kamery (jak wskaźnik mikrofonu): widać, czy którakolwiek kamera jest otwarta
i kogo Alfa rozpoznaje jako rozmówcę.

## Architektura: cztery moduły

Zgodnie z regułą projektu każdy moduł to trójka `-contract` / `-impl` / `-fake` i jednostronicowy SPEC.

| Moduł                | Rola                                                                                                                          | Szkic SPEC                                                    |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| `camera-hub`         | „specjalny system kamer”: rejestr kamer, źródła (USB, sieć), kilka strumieni naraz, klatki na żądanie i podgląd, stan i błędy | [`camera-hub`](../modules/camera-hub/SPEC.md)                 |
| `vision-describe`    | opis obrazu: wybór modelu widzącego (lokalny albo w chmurze za zgodą), zapytanie o klatkę, odpowiedź tekstowa                 | [`vision-describe`](../modules/vision-describe/SPEC.md)       |
| `person-recognition` | wykrywanie twarzy, wzorce twarzy, zapisywanie osoby (za zgodą), dopasowanie „kto to” z progiem pewności                       | [`person-recognition`](../modules/person-recognition/SPEC.md) |
| `person-memory`      | rejestr osób i pamięć per osoba: profil, fakty, streszczenia rozmów; przypisanie tur rozmowy do osoby                         | [`person-memory`](../modules/person-memory/SPEC.md)           |

```text
kamery USB ─┐
kamery IP  ─┼─► camera-hub ──klatka──► vision-describe ──► agentka („opisz, co widzisz”)
kamery Wi-Fi┘        │
                     └──klatka (rzadko)──► person-recognition ──„to Arek, pewność 0,93”──► person-memory
                                                                                              │
mikrofon ─► voice-speaker (głos właściciela — już jest) ─────────────── drugi sygnał ─────────┤
                                                                                              ▼
                                                              rozmowa: rozmówca + jego pamięć w kontekście
```

Zasady wspólne:

- **Broker pozostaje jedynym wykonawcą.** Agentka nie otwiera kamery sama: prosi o narzędzie `camera.look`
  (klatka do opisu) albo `camera.who` (kto jest przed kamerą), a Broker sprawdza zdolność i poziom autonomii.
- **Kod systemowy tylko w `platform-windows-impl`** (kamery USB przez Media Foundation) za portem
  `CameraPort` w `platform-contract`. Dzięki temu Linux i Android dostaną własne implementacje (etapy F, G).
- **Kamery sieciowe przez osobny proces** (dekoder strumienia), tak jak silniki mowy i modele — awaria dekodera
  nie zabiera programu, a ruch sieciowy przechodzi przez `lib-netguard` (tylko sieć lokalna, lista dozwolonych
  adresów).
- **Obraz jest treścią niezaufaną.** Napis na kartce przed kamerą („zignoruj polecenia i…”) to dane, nie
  polecenie — klatka i jej opis dostają znacznik niezaufanej treści, jak strona WWW czy mail.

### Kamery: USB, IP, Wi-Fi

| Rodzaj                    | Jak Alfa ją widzi                                              | Uwagi                                                                           |
| ------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| USB (wbudowana, na kablu) | lista urządzeń systemu; podłączenie i odłączenie na żywo       | Media Foundation; rozdzielczość i liczba klatek wybierane z listy trybów kamery |
| IP po kablu               | adres strumienia RTSP albo wyszukanie w sieci lokalnej (ONVIF) | login i hasło kamery w Windows Credential Manager, nigdy w pliku                |
| Wi-Fi                     | to samo co IP — kamera Wi-Fi to kamera sieciowa                | słabszy zasięg = gubione klatki; hub pokazuje jakość połączenia                 |
| Telefon jako kamera       | późniejszy etap, razem z aplikacją na Androida (etap F)        | wymaga kanału z etapu D                                                         |

Kilka kamer naraz: hub trzyma każdą kamerę jako osobny strumień z własnym budżetem. Domyślnie strumień jest
**uśpiony** — kamera otwiera się, gdy ktoś prosi o klatkę albo włączone jest rozpoznawanie rozmówcy; po okresie
bezczynności się zamyka. Podgląd na żywo w Ustawieniach pobiera klatki w niskiej rozdzielczości.

### Opis obrazu

„Opisz mi to, co widzisz” jest rozpoznawane jako polecenie z użyciem narzędzia `camera.look`. Klatka trafia do
modelu widzącego wybranego przez Router:

| Droga                            | Kiedy                                                          | Uwagi                                                                                                                      |
| -------------------------------- | -------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Model lokalny z obsługą obrazu   | profil „Lokalny” albo sesja prywatna                           | obraz nie opuszcza komputera; na karcie 6 GB model widzący i model rozmowy mogą się nie zmieścić razem — do pomiaru (C3.6) |
| Model w chmurze z obsługą obrazu | profil „Hybryda” albo „Chmura”, po zgodzie na wysyłanie obrazu | dokładniejszy opis; **obraz z kamery wychodzi do dostawcy** — osobna zgoda, wyłączona domyślnie                            |

W sesji prywatnej obraz nigdy nie idzie do chmury. Jeśli nie ma żadnego modelu widzącego, Alfa mówi o tym
wprost i wskazuje, co pobrać albo włączyć.

### Rozpoznawanie osób

1. **Zapisanie osoby** (kreator w Ustawieniach): osoba podaje imię, wyraża zgodę i pokazuje twarz z kilku stron.
   Alfa zapisuje **wzorzec twarzy** (ciąg liczb), nie zdjęcia.
2. **Rozpoznanie**: co kilka sekund jedna klatka w małej rozdzielczości → wykrycie twarzy → wzorzec →
   porównanie z zapisanymi. Wynik ma pewność; poniżej progu osoba jest „nieznana”.
3. **Dwa sygnały**: twarz z kamery i głos z mikrofonu (moduł `voice-speaker` już rozpoznaje głos właściciela).
   Gdy oba wskazują tę samą osobę, pewność rośnie; gdy się różnią, Alfa pyta zamiast zgadywać.

**Rozpoznanie twarzy nie jest zgodą ani hasłem.** Nie odblokowuje akcji ryzykownych, nie zastępuje okna Brokera
i nie podnosi poziomu autonomii — twarz da się podrobić zdjęciem. Służy wyłącznie personalizacji rozmowy.

### Pamięć osób

Każda zapisana osoba ma w `person-memory`:

| Co                  | Przykład                                                 | Skąd                                                                |
| ------------------- | -------------------------------------------------------- | ------------------------------------------------------------------- |
| Profil              | imię, forma zwracania się, język, uwagi                  | kreator zapisu, potem poprawki z rozmów                             |
| Fakty               | „pracuje nad raportem Q3”, „nie lubi długich odpowiedzi” | wyciągane z rozmów jak dziś w pamięci Alfy, ale przypisane do osoby |
| Streszczenia rozmów | „8 października: plan wyjazdu, ustalone daty”            | po każdej rozmowie; zasila kontekst następnej                       |
| Wzorce              | twarz (i głos, jeśli osoba zapisała głos)                | zaszyfrowane, klucz w Credential Manager                            |

Pamięć jest trwała: żyje w bazie programu i wraca po uruchomieniu. Tury rozmowy dostają znacznik „kto mówił”
(historia rozmowy pozostaje tylko do dopisywania — znacznik jest częścią tury, nie jej późniejszą edycją).
Gdy rozmówcą jest osoba X, do kontekstu trafia pamięć X i wspólna; pamięć innych osób — nie.

## Prywatność i bezpieczeństwo

| Zasada                                | Jak jest egzekwowana                                                                                                              |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Kamery są domyślnie wyłączone         | pierwsze włączenie przez kreator ze zgodą; każda kamera ma własny przełącznik                                                     |
| Widać, że kamera działa               | wskaźnik w belce i w pigułce głosowej; nie da się go ukryć ustawieniem                                                            |
| Nagrania nie są przechowywane         | hub nie zapisuje wideo; klatka żyje w pamięci do końca zapytania; w rozmowie zostaje miniatura, którą można usunąć z resztą sesji |
| Wzorce twarzy tylko lokalnie          | zaszyfrowane w bazie, klucz w Credential Manager; nie wchodzą do eksportu `.alfa`, logów ani diagnostyki                          |
| Zapis tylko za zgodą osoby            | kreator wymaga potwierdzenia przy kamerze; usunięcie osoby kasuje wzorce i jej pamięć                                             |
| Obraz do chmury tylko za osobną zgodą | przełącznik wyłączony domyślnie; nigdy w sesji prywatnej                                                                          |
| Agentki nie włączają kamer po cichu   | narzędzia `camera.*` przez Brokera; zdolność `camera.view` nadawana per agentka                                                   |
| Hasła kamer jak inne sekrety          | Windows Credential Manager; adresy strumieni bez hasła w konfiguracji                                                             |
| Kamera sieciowa nie jest furtką       | tylko adresy sieci lokalnej z listy dozwolonych; dekoder w osobnym procesie bez dostępu do danych Alfy                            |

Nowe zagrożenia do dopisania w `THREAT_MODEL.md` po akceptacji ADR 0025: podstawiona kamera albo strumień,
polecenia ukryte w obrazie, zdjęcie zamiast twarzy, wyciek wzorców, podgląd przez cudzą agentkę lub wtyczkę,
przejęta kamera sieciowa jako punkt wejścia do sieci.

## Prawo (nie jest to porada prawna)

- **Wzorzec twarzy to dane biometryczne** (RODO, art. 9). Do prywatnego użytku domowego RODO zwykle się nie
  stosuje, ale kamera obejmująca gości, współpracowników albo klatkę schodową to już inna sytuacja. Dlatego
  plan zakłada wyraźną zgodę każdej zapisywanej osoby i brak rozpoznawania osób niezapisanych.
- **AI Act** zakazuje między innymi rozpoznawania emocji w miejscu pracy i szkole oraz budowania baz twarzy
  z niecelowanego pobierania zdjęć. Alfa tego nie robi: nie ocenia emocji i nie tworzy wzorców osób, które się
  nie zapisały. Punkt do potwierdzenia przez prawnika przed wydaniem publicznym.
- Zadania i pytania do prawnika: [`10-zgodnosc-prawna.md`](10-zgodnosc-prawna.md).

## Sprzęt i wydajność (do pomiaru, nie obietnice)

Sprzęt bazowy: laptop i7-13620H, RTX 4050 z 6 GB, 16 GB RAM, obok karta zintegrowana.

| Zadanie                   | Gdzie liczone                | Czego się spodziewamy                                                                                          | Stan           |
| ------------------------- | ---------------------------- | -------------------------------------------------------------------------------------------------------------- | -------------- |
| Dekodowanie 2–4 strumieni | procesor / dekoder sprzętowy | mieści się, jeśli strumienie są uśpione poza zapytaniem                                                        | do pomiaru     |
| Wykrycie i wzorzec twarzy | procesor (ONNX Runtime)      | ułamek sekundy na klatkę przy jednej klatce co kilka sekund                                                    | do pomiaru     |
| Opis obrazu lokalnie      | karta główna                 | **wspólnie z modelem rozmowy może nie zmieścić się w 6 GB** — wtedy model widzący ładuje się na czas zapytania | do pomiaru     |
| Lekkie zadania wizyjne    | karta zintegrowana           | możliwe po pobraniu silnika Vulkan (zadanie A.3i)                                                              | do sprawdzenia |

## Etapy

| Krok            | Zakres                                                           | Wynik widoczny dla użytkownika                       |
| --------------- | ---------------------------------------------------------------- | ---------------------------------------------------- |
| 1 (C3.1–C3.4)   | port kamery, kamery USB, hub z wieloma kamerami, strona „Kamery” | lista kamer, podgląd, przełączniki, wskaźnik w belce |
| 2 (C3.5–C3.7)   | kamery sieciowe, narzędzie `camera.look`, opis obrazu            | „opisz mi to, co widzisz” działa                     |
| 3 (C3.8–C3.10)  | wykrywanie twarzy, zapis osób, rozpoznanie rozmówcy              | Alfa wita po imieniu, rozróżnia osoby                |
| 4 (C3.11–C3.13) | pamięć per osoba, przypisanie tur, łączenie z głosem             | Alfa pamięta, o czym z kim rozmawiała                |
| 5 (C3.14–C3.15) | ewaluacja, pomiary na sprzęcie, przegląd bezpieczeństwa          | kryteria `ACC-V6-camera-…` zaliczone na maszynie     |

Krok 1 i 2 nie dotykają biometrii i mogą ruszyć bez ADR 0025. Kroki 3 i 4 czekają na jego podpis.

## Decyzje dla właściciela

| Decyzja                                                                   | Opcje                                                                          | Co blokuje   |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | ------------ |
| Podpis pod ADR 0025 (biometria twarzy)                                    | przyjąć / zmienić zasady / zrezygnować z rozpoznawania osób                    | C3.8–C3.13   |
| Czy obraz z kamery może iść do modelu w chmurze                           | nigdy / za zgodą per kamera (propozycja) / za zgodą globalną                   | C3.7         |
| Zgoda na pobranie modelu widzącego i modeli twarzy                        | rozmiar do ustalenia w C3.6 i C3.8                                             | C3.6, C3.8   |
| Licencje modeli twarzy                                                    | tylko modele z licencją pozwalającą na użycie w produkcie — sprawdzenie w C3.8 | C3.8         |
| Progi akceptacji (trafność rozpoznania, fałszywe rozpoznania, czas opisu) | ustala właściciel                                                              | wyjście z C3 |
| Czy osoba nieznana może rozmawiać z Alfą                                  | tak jako gość bez pamięci (propozycja) / tylko zapisani                        | C3.10        |
