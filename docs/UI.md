# UI / UX programu Alfa

> Dokument pochodny od `docs/PLAN.md` (§14, §15, §1.2, §8.2, §8.6). Nie wprowadza nowych decyzji ani nowych skrótów; wartości tokenów i budżetów są startowe — strojone na makietach i mierzone w F0. Stos: Tauri 2.x + WebView2, Svelte 5 (runes) + Bits UI, czysty Vite, bez SSR; tokeny i komponenty w `packages/ui-kit` (Storybook).

## 1. Kierunek wizualny

**„Spokojny, jasny, szybki.”** Nowoczesny styl Windows 11 (Fluent 2), ale ciszej:

| Zasada | Jak |
|---|---|
| Dużo powietrza | jedna kolumna tekstu, rytm odstępów z siatki 4 px |
| Cienkie podziały zamiast ramek | 1 px linie, elewacja bez ciężkich cieni |
| **Jeden akcent koloru na raz** | kolor mówiącej / pracującej agentki; reszta neutralna |
| Zero dekoracji w treści | bez ciężkich gradientów i efektów; ładne przez typografię, rytm odstępów i płynny, krótki ruch |
| Spokój na wierzchu, moc pod spodem | powierzchnia minimalna, panele chowane, pełne ustawienia w osobnym widoku |

## 2. Trzy warstwy

| Warstwa | Zawartość | Widoczność |
|---|---|---|
| 1. Powierzchnia | rozmowa, jedno pole wejścia (composer), przycisk mikrofonu, kapsuła aktywności | zawsze |
| 2. Panele | lewy — **Sesje** (lista, wyszukiwanie, projekty); prawy — jeden panel naraz w kartach: Agentki · Oś czasu/Logi · Pliki/Artefakty · Pamięć · Ekran · Głos; każdy panel można **odłączyć do osobnego okna** | chowane, pamiętane per sesja |
| 3. Ustawienia i konfiguracja | osobny widok z drzewem i wyszukiwarką (§12) | na żądanie (`Ctrl+,`) |

**Tryb skupienia** (`F11` lub `Ctrl+Shift+Enter`) chowa wszystko poza rozmową.

## 3. Układ i szkic

```
┌─ Alfa ──────────────────────────────────────────────── ─ □ × ┐  pasek tytułu 32–40 px (stały, Mica)
│ ☰  Projekt X ▸ Raport Q3 ▾      (A)(B)(Γ)(Δ)    ● Hybryda · L3 · 2,14 zł  ⚙ │
├────────────┬──────────────────────────────────────┬──────────────────┤
│ SESJE  ⌕   │                                      │ Agentki│Oś│Pliki │  panel prawy 300–520 px
│ ● Raport Q3│   rozmowa — kolumna ~72 znaki        │                  │  (zmienna szerokość,
│   Kod: API │   (≈560 px; wariant szeroki 760 px), │  (zawartość      │   przeciągana krawędź)
│   Zakupy   │   wyśrodkowana, dużo oddechu         │   panelu)        │
│ + Nowa     │  ╭ Δ Delta · edytuję raport.docx ─╮  │                  │
│            │  │ krok 3/7 ▰▰▰▱▱▱▱  0:42   ■ Stop │  │                  │
│ 240 px     │  ╰────────────────────────────────╯  │                  │
├────────────┴──────────────────────────────────────┴──────────────────┤
│  📎  Napisz… (@agentka, /komenda)        [Alfa ▾] [Hybryda ▾]   🎙  ➤ │  composer: 1–12 linii
└──────────────────────────────────────────────────────────────────────┘
```

### 3.1 Pasek górny

Stały, cienki; auto-ukrywany **tylko** w trybie skupienia (inaczej psuje przeciąganie okna i Snap Layouts). Własny pasek tytułu z natywnymi przyciskami okna i obsługą **Snap Layouts** Windows 11 (do sprawdzenia w F0 (j) dla Tauri).

| Element | Zachowanie |
|---|---|
| Przełącznik lewego panelu (☰) | `Ctrl+B` |
| Ścieżka projekt ▸ sesja | klik = zmień nazwę (`F2`) |
| Obsada agentek | 4 małe awatary: kółko z greckim glifem α β γ δ i kolorowym pierścieniem; **świeci ta, która mówi lub pracuje**; klik = szczegóły i zmiana roli |
| Stan | profil głosu/modelu, poziom autonomii (L0–L4), koszt sesji w PLN — klik otwiera szczegóły |
| Ustawienia (⚙) | `Ctrl+,` |

### 3.2 Kolumna rozmowy i panele

- Kolumna czytania ~72 znaki (≈560 px), wariant szeroki 760 px (wąska/szeroka do wyboru), wyśrodkowana.
- Lewy panel 240–320 px, zwijany do paska ikon 48 px; prawy 300–520 px, przeciągana krawędź.
- Szerokości paneli, pozycja okna i monitor **zapamiętywane per maszyna** (nakładka `config/machine/<id>.toml`); **które panele są otwarte — per sesja**.
- Kapsuła aktywności w wątku: agentka · czynność · krok n/m · pasek postępu · czas · **Stop**.

### 3.3 Responsywność (laptop 1080p ze skalowaniem 125–150 %)

| Szerokość okna (px efektywne) | Zachowanie |
|---|---|
| ≥ 1440 | lewy i prawy panel zadokowane jednocześnie |
| 1100–1440 | jeden panel zadokowany, drugi jako wysuwana szuflada |
| 720–1100 | panele jako szuflady nad treścią; kolumna rozmowy pełna |
| < 720 (min. okna 400 × 500) | tryb kompaktowy: tylko rozmowa + composer; panele jako arkusze pełnoekranowe |

Makiety są sprawdzane w szerokości laptopa (1280 px efektywnie).

### 3.4 Okno główne, zasobnik, wiele okien

| Temat | Zasada |
|---|---|
| Zamknięcie okna | ukrywa okno (do zasobnika; konfigurowalne); WebView żyje przez N minut (domyślnie 10), potem jest niszczony dla oszczędności RAM |
| Otwarcie z zasobnika | ciepłe ≤ 150 ms, zimne ≤ 1 s |
| Zimny start aplikacji | do zasobnika ≤ 1 s, do okna ≤ 1,5 s (PLAN §3.4) |
| Wiele okien | odłączone panele, Szybkie pytanie, pigułka głosowa — wspólne środowisko WebView2 (jeden proces przeglądarki); koszt „+MB na okno" mierzony w F0 (f); limit odłączonych okien z ostrzeżeniem |
| Stały folder danych WebView2 | poza katalogiem wersji (integracja z Windows przy wersjach side-by-side) |
| Przeładowanie WebView | `F5` / `Ctrl+R` wyłączone |

## 4. Tokeny designu (`packages/ui-kit`, jedno źródło prawdy)

| Token | Wartość startowa |
|---|---|
| **Fonty** | systemowe (zero pobierania): **Segoe UI Variable** (tekst), **Cascadia Mono** (kod); fallback `system-ui` |
| **Skala typografii** | 12 · 13 · **14 (baza)** · 16 · 20 · 24 · 32 px; interlinia 1,5 (tekst), 1,25 (nagłówki); grubości 400 / 600 |
| **Siatka odstępów** | 4 px: 4 · 8 · 12 · 16 · 24 · 32 · 48 |
| **Promienie** | 6 (kontrolki) · 10 (karty, wiadomości) · 16 (nakładki, pigułka głosowa) · pełny (awatary, chipy) |
| **Elewacja** | 3 poziomy; w jasnym: delikatny cień + 1 px obramowanie; w ciemnym: tylko jaśniejsze tło + obramowanie (bez cieni) |
| **Neutralne** | chłodna szarość (≈ slate), tło jasne ~#FAFAFB / ciemne ~#131417; tekst główny kontrast ≥ 7:1, pomocniczy ≥ 4,5:1 |
| **Kolory agentek** (akcenty, nie tekst) | **Alfa** — ciepły koral · **Beta** — miętowa zieleń · **Gama** — indygo · **Delta** — lazur/turkus (propozycja; ostateczne odcienie z makiet, sprawdzane skryptem kontrastu i symulacją deuteranopii; odrębne od kolorów błąd/ostrzeżenie/sukces); każdy w wariancie jasnym i ciemnym, kontrast ≥ 3:1 dla elementów UI (WCAG 1.4.11) i ≥ 4,5:1, gdy kolor niesie tekst; zawsze w parze z glifem α/β/γ/δ i imieniem (nie tylko kolor). Propozycje hex: `docs/PERSONAS.md` |
| **Semantyczne** | sukces / ostrzeżenie / błąd / informacja + **kolor ryzyka** w kartach zatwierdzeń (niskie / średnie / wysokie) |
| **Ruch** | 120 ms (hover, naciśnięcie) · 180 ms (panele, szuflady) · 240 ms (orb, tryb głosowy); ease-out; **tylko `transform` / `opacity`**; `prefers-reduced-motion` i ustawienie „bez animacji" wyłączają ruch |
| **Ikony** | jeden zestaw liniowy (np. Lucide) jako SVG inline, tylko użyte ikony (tree-shaking), 16 / 20 px, linia 1,5 px |
| **Tła okna** | **natywna Mica** (liczona przez system — tania) dla paska i paneli; treść na jednolitym, nieprzezroczystym tle; CSS `backdrop-filter` zabroniony poza jedną nakładką (paleta `Ctrl+K`); fallback na jednolity kolor przy: wyłączonej przezroczystości w Windows, trybie baterii, trybie wysokiego kontrastu |
| **Motywy** | jasny / ciemny / auto (za Windows), wysoki kontrast (`forced-colors`), akcent: kolor agentki albo kolor akcentu Windows; gęstość: komfortowa / kompaktowa; zoom 80–200 % (`Ctrl+=` / `Ctrl+-` / `Ctrl+0`) |

Bezpieczeństwo renderowania (PLAN §8.2): markdown z LLM renderowany do HTML w Rust (pulldown-cmark + `ammonia`, bez surowego HTML i skryptów) → wirtualizacja, zaznaczanie, `Ctrl+F` i czytniki ekranu działają; sandboxowany iframe bez IPC tylko dla aktywnych artefaktów HTML/SVG (jeden na podgląd); ścisłe CSP bez inline, Trusted Types, minimalne capabilities Tauri per okno; port CDP tylko w buildzie testowym.

## 5. Dostępność (od F1)

| Wymaganie | Szczegół |
|---|---|
| Klawiatura-first | każda akcja osiągalna bez myszy; widoczny fokus 2 px |
| WCAG 2.2 AA | kontrasty z §4; cele kliknięcia ≥ 24 px |
| Strumień odpowiedzi | `aria-live` z throttlingiem — ogłaszanie na końcu zdania |
| Karta potwierdzenia | `alertdialog` z pułapką fokusu |
| Mowa | napisy do mowy (tryb głosowy, §10) |
| Kolor | nigdy sam: glif + imię (agentki), ikona + tekst (stany mikrofonu) |
| CI | Storybook + regresja wizualna + axe (0 naruszeń critical/serious — kryterium F1); testy klawiatury |
| Języki | PL / EN, i18n od dnia 0 |

## 6. Stany systemowe

Każdy stan ma baner lub kartę z wyjaśnieniem i akcją (ekran makiety nr 18).

| Stan | Zachowanie UI |
|---|---|
| Offline | fallback lokalny + baner + kolejka wiadomości |
| 429 / wyczerpane okno planu | informacja, kiedy się odnowi; przełączenie trasy |
| Brak zgody mikrofonu | link `ms-settings:privacy-microphone` |
| GPU OOM | komunikat, przełączenie STT/LLM na CPU lub chmurę |
| Przerwane pobieranie (model, moduł) | wznawianie |
| Brak miejsca na dysku | ostrzeżenie, limity logów |
| Crash-loop modułu | stan z watchdoga, safe-mode |
| Pętla agentki | detektor pętli, Stop w kapsule |
| Zablokowany ekran | zadania GUI wstrzymane, informacja |
| Zmiana urządzenia audio | baner + wybór urządzenia jednym kliknięciem |
| Brak kluczy / dostawca nieskonfigurowany | „dodaj klucz, aby odblokować X" → Hub kont |
| Okno administratora na pierwszym planie | PTT / dyktowanie / hooki nie działają bez helpera `uiAccess` — komunikat wprost |

## 7. Onboarding (w MVP, ekran makiety nr 14)

| Krok | Treść | Od fali |
|---|---|---|
| 1 | Test mikrofonu | F1 (minimalny) / F2 |
| 2 | Wybór profilu głosu (A–D, wg `device-profile`) | F2 |
| 3 | Pomiar sprzętu (`device-profile`, „Kreator sprzętu" pokazuje kompromis i pozwala nadpisać) | F1 |
| 4 | Konta i klucze: **„dodaj teraz" albo „pomiń — dodam później"**; bez kluczy startuje profil lokalny (model 3–4,5B pobierany tutaj) | F1 |
| 5 | Tryb próbny i prosty opis poziomów autonomii (start L3, L4 „Maks" jednym przełącznikiem) | F3 |
| 6 | Opcjonalne nagranie korpusu własnego | F2 |
| 7 | Opcjonalny import paczki `.alfa` z innej maszyny | F1 |
| 8 | **Mosty CLI:** wykrycie Claude Code / Codex, zależności, logowanie w wbudowanym terminalu ConPTY (logujesz się sam; tokeny nie są przechwytywane) | **F4** — wcześniej krok niewidoczny |

## 8. Koszty i zatwierdzenia

| Temat | Zasada |
|---|---|
| Limit miesięczny | w PLN, ustawiany w aplikacji, **z możliwością całkowitego wyłączenia** (zostaje wskaźnik zużycia i opcjonalne alerty) |
| Kurs | ceny dostawców w USD → kurs NBP (tabela A, raz dziennie, kurs zapasowy w konfiguracji) |
| Szacunek | przed długim zadaniem i przy dużych załącznikach (tokeny / koszt w composerze) |
| Budżet tła | osobny, domyślnie tylko modele lokalne |
| Okno planu (mosty) | wskaźnik zużycia best effort |
| Licznik kontekstu | pasek zużycia okna kontekstu sesji z oznaczeniem kompaktowania; koszt sesji i dnia; ostrzeżenie przed limitem (jeśli włączony) |
| Zmęczenie zatwierdzeniami | **„plan do zatwierdzenia" zamiast 40 pytań**; szablony uprawnień; metryka pytań/godz.; „zawsze zezwalaj w tym zakresie" nie eskaluje do L4 |
| Gdzie się zatwierdza | **tylko w oknie Brokera** (Broker-UI, osobny natywny proces na wyższym poziomie integralności); w wątku jest karta „czeka na zatwierdzenie" z przyciskiem przenoszącym do Brokera; **nigdy w toaście**; wejście musi być fizyczne (klik / klawisz), opcjonalnie Windows Hello |

## 9. Budżety wydajności UI (CI: ślad CDP/Playwright na szkielecie, pod limitami baseline)

| Wskaźnik | Cel |
|---|---|
| Pisanie w composerze (klawisz → widoczny znak) | ≤ 16 ms p95 |
| Strumień odpowiedzi 100 tokenów/s | płynne 60 kl./s; **aktualizacja DOM najwyżej raz na klatkę** (bufor + `requestAnimationFrame`); zadania na głównym wątku ≤ 50 ms |
| Otwarcie panelu / szuflady | pierwsza klatka ≤ 100 ms |
| Przełączenie sesji (1000 wiadomości) | ≤ 150 ms (lista wirtualizowana — renderowane tylko widoczne wiadomości ± 1 ekran) |
| Paleta poleceń (`Ctrl+K`) | otwarcie ≤ 50 ms, wyniki ≤ 16 ms na znak |
| Paczka JS przy starcie | ≤ 150 KB gzip; panele, ustawienia, Voice Lab, edytor surowy jako leniwie ładowane moduły |
| CSS | ≤ 30 KB gzip; zero fontów webowych; bez CSS `backdrop-filter` w treści (najwyżej 1 mała nakładka) |
| Bezczynność z otwartym oknem | ~0 % CPU; animacja orba zatrzymana, gdy okno ukryte / zminimalizowane; wskaźniki głośności ≤ 30 kl./s |
| Markdown i kod | przyrostowe parsowanie (zamknięte bloki się nie przerenderowują); **podświetlanie składni leniwie i w Web Workerze** po zakończeniu bloku; długie wyniki zwinięte; obrazy jako leniwe miniatury |
| IPC rdzeń → UI | zdarzenia grupowane (batch co klatkę), bez przesyłania dużych danych (pliki / obrazy przez ścieżki i protokół zasobów) |
| Przyrost pamięci | Private WS ≤ 5 % po 1 h / 500 wiadomościach (kryterium F1) |

## 10. Tryb głosowy — szczegóły UI

### 10.1 Pełny tryb głosowy (ekran nr 3)

- Duży **orb** (Canvas 2D; reaguje na głośność; kolor mówiącej agentki; 30 kl./s; pauza w tle).
- **Napisy na żywo** pod orbem: Twoja wypowiedź najpierw szara (tekst częściowy), po zatwierdzeniu pełna; odpowiedź agentki z podświetleniem już wypowiedzianych słów („usłyszany prefiks", `docs/VOICE.md` §7) i znacznikiem **„przerwano tutaj"**.
- Przyciski: Stop mowy · Wycisz · Przełącz agentkę · Przejdź do tekstu; wybór urządzenia audio w jednym kliknięciu.

### 10.2 Stany mikrofonu (kolor + ikona + tekst, nigdy sam kolor)

| Stan | Znaczenie |
|---|---|
| wyłączony | mikrofon zamknięty |
| słucha | otwarty, czeka na mowę (PTT / wake / „zawsze słucham") |
| słyszy Cię | VAD wykrył mowę |
| przetwarza | koniec tury → STT → model |
| agentka mówi | TTS gra; barge-in aktywny |
| wyciszony | ręcznie wyciszony |
| nie przeszkadzać | także automatycznie przy pełnym ekranie / grach |

Wskaźnik prywatności w zasobniku, gdy mikrofon jest otwarty.

### 10.3 Sterowanie głosem z klawiatury

`Ctrl+Shift+M` mikrofon wł./wył. · przytrzymanie `Spacji` (poza polem tekstowym) = mów (PTT; hook `WH_KEYBOARD_LL`) · `Esc` = stop mowy (po zamknięciu menu/dialogu) · `Ctrl+Shift+F12` STOP WSZYSTKIEGO.

## 11. Drobne funkcje (wszystko także z klawiatury i palety poleceń)

### 11.1 Composer

- Wiele linii (auto-wzrost do ~40 % wysokości, 1–12 linii); `Enter` wyślij / `Shift+Enter` nowa linia (odwracalne w ustawieniach).
- **Szkic zapisywany per sesja**; wklejanie obrazów i plików, przeciąganie, zrzut ekranu z composera.
- `@agentka` (adresowanie), `/komendy` (np. `/obsada`, `/model`, `/pamięć`, `/eksport`); wybór agentki i profilu modelu jako chipy.
- Szacunek kosztu / tokenów przy dużych załącznikach.
- **Sprawdzanie pisowni PL** (wbudowane w WebView2 + słownik użytkownika; do weryfikacji w F0 (j)).
- Historia wysłanych (`Ctrl+↑/↓`); `↑` w pustym composerze = edytuj ostatnią wiadomość; „wklej jako zwykły tekst" (`Ctrl+Shift+V`); cofnięcie wysłania przez 3 s (opcjonalnie).
- **Dyktowanie do czatu:** podgląd transkryptu z możliwością poprawienia przed wysłaniem (opcjonalny tryb „sprawdź przed wysłaniem").

### 11.2 Wiadomość — pasek akcji (po najechaniu, fokusie lub prawym przyciskiem)

| Akcja | Uwagi |
|---|---|
| Kopiuj | Markdown / tekst |
| Przeczytaj na głos | głosem tej agentki |
| Ponów | opcjonalnie innym modelem; tworzy wariant obok (`‹ 1/3 ›`) |
| Edytuj i wyślij ponownie | tworzy odgałęzienie od tego miejsca |
| Kontynuuj | uciętą odpowiedź |
| Przekaż agentce… | |
| Rozgałęź do nowej sesji | |
| Cytuj / odpowiedz | |
| Zapamiętaj | do pamięci, z wyborem zakresu (sesja / projekt / globalna / agentka) |
| Ocena 👍/👎 | z notatką (sygnał dla Ulepszacza) |
| Szczegóły | model, tokeny, koszt, opóźnienie, użyte narzędzia → skok do Osi czasu |
| Ukryj z widoku | audyt zostaje |
| Eksportuj | |

Dziennik zdarzeń jest append-only, a widoczna rozmowa to projekcja drzewa gałęzi — każda gałąź jest nieedytowaną historią, więc bloki myślenia dostawców pozostają ważne. Znaczniki czasu względne z pełną datą w podpowiedzi.

### 11.3 Wiadomość agentki

- Awatar z pierścieniem koloru + imię + chip roli (np. „Delta · Wykonawczyni").
- Kroki narzędzi zwinięte do jednej linii statusu (ikona, opis, czas) z rozwinięciem.
- Bloki kodu: kopiuj, zapisz jako plik, **uruchom w terminalu** (przez Broker), zawijanie.
- Linki do plików otwierają podgląd.
- **Karta „czeka na zatwierdzenie"** z przyciskiem przenoszącym do okna Brokera (samo zatwierdzenie tylko tam).

### 11.4 Sesje (panel lewy)

Wyszukiwanie pełnotekstowe (`Ctrl+Shift+F`), projekty / foldery, tagi, przypięte, **kropka aktywności** przy sesjach pracujących w tle, znacznik nieprzeczytanych, sortowanie, zmiana nazwy (`F2`; automatyczny tytuł z pierwszych wiadomości), duplikuj jako szablon, archiwizuj, usuń z **cofnięciem przez 10 s**, eksport pojedynczej sesji do `.alfa`. Szablony sesji: Kodowanie, Research, Asystent głosowy, Administracja PC, Pusty. Karty + widok dzielony + odłączane okna; fork; przypinanie; **przekazanie kontekstu** między czatami jako jawna wiadomość.

### 11.5 Zasobnik systemowy (tray)

- Ikona ze stanem: spoczynek / słucha / mówi / pracuje / błąd / mikrofon otwarty.
- Menu: pokaż · nowa rozmowa · **Szybkie pytanie** · głos wł./wył. · nie przeszkadzać · **STOP WSZYSTKIEGO** · wyjście.
- Zamknięcie okna = do zasobnika (konfigurowalne).

### 11.6 Szybkie pytanie (Quick Ask)

Globalny skrót (domyślnie `Ctrl+Alt+Space`, zmienialny, wykrywanie konfliktów np. z PowerToys) → małe okno 640 px na środku ekranu z polem tekstowym; odpowiedź rozwija się pod spodem; `Enter` „otwórz w pełnym oknie"; `Esc` zamyka. Moduł `ui-quick` (F1).

### 11.7 Pigułka głosowa (mini-nakładka, F5)

Małe, przesuwane okno „zawsze na wierzchu" (ok. 220 × 48 px) z awatarem mówiącej agentki, falą głośności, stanem mikrofonu i przyciskami Stop / Wycisz; pokazuje się, gdy rozmawiasz głosem przy ukrytym oknie głównym. Minimalna strona bez frameworka (każde dodatkowe okno Tauri kosztuje RAM — mierzone w F0).

### 11.8 Powiadomienia i dźwięki

- Natywne Windows (toast) gdy okno ukryte — zakończenie zadania, prośba o zatwierdzenie (sam toast, nie zatwierdzenie), błąd; toasty z AUMID przez launcher (F0 (j)).
- W aplikacji — małe toasty w prawym dolnym rogu.
- Tryb **nie przeszkadzać** (także automatycznie przy pełnym ekranie / grach).
- **Earcony:** krótkie, ciche: start/stop słuchania, zadanie ukończone, błąd, prośba o zatwierdzenie; osobna głośność, wyłączalne.
- **Pasek zadań Windows:** postęp długiego zadania na ikonie i plakietka „czeka na zatwierdzenie".

### 11.9 Podglądy plików i artefakty

Obrazy, PDF, tekst / kod (z podświetleniem), audio / wideo, Markdown; dokumenty Office jako podgląd tekstowy (P1). Panel Artefakty: karta pliku (nazwa, ścieżka, rozmiar, podgląd / diff, wersje) + Otwórz · Pokaż w Eksploratorze · Kopiuj (jako plik do schowka) · Zapisz jako… · Spakuj · Wyślij (MCP) · Przekaż do innego czatu + przeciągnięcie do Eksploratora. Domyślnie `%USERPROFILE%\Alfa\Sesje\<nazwa>\out`. Wejście: drag&drop, wklejanie, zrzut ekranu, montowanie folderu, „Otwórz w Alfie" z Eksploratora.

### 11.10 Skróty (wszystkie zmienialne, z wykrywaniem konfliktów i regułą AltGr)

| Skrót | Akcja |
|---|---|
| `Ctrl+N` | nowa rozmowa |
| `Ctrl+W` | zamknij kartę |
| `Ctrl+Shift+T` | przywróć zamkniętą |
| `Ctrl+Tab` / `Ctrl+1…9` | karty |
| `Ctrl+P` | szybkie przełączanie sesji |
| `Ctrl+B` | panel sesji |
| `Ctrl+\` | panel prawy |
| `Alt+1…6` | panele (Agentki, Oś czasu, Pliki, Pamięć, Ekran, Głos) |
| `Ctrl+K` | paleta poleceń |
| `Ctrl+F` | szukaj w rozmowie |
| `Ctrl+Shift+F` | szukaj wszędzie |
| `Ctrl+,` | ustawienia |
| `Ctrl+/` | ściągawka skrótów |
| `Ctrl+Shift+M` | mikrofon wł./wył. |
| przytrzymanie `Spacji` (poza polem tekstowym) | mów (PTT) |
| `Esc` | kolejno: zamknij menu/dialog → stop mowy → stop generowania |
| `F11` / `Ctrl+Shift+Enter` | tryb skupienia |
| `↑` (pusty composer) | edytuj ostatnią |
| `Ctrl+↑/↓` | historia wysłanych |
| `F2` | zmień nazwę |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | zoom |
| `Ctrl+Shift+V` | wklej jako zwykły tekst |
| `Ctrl+Alt+Space` | szybkie pytanie (globalny) |
| **`Ctrl+Shift+F12`** | **STOP WSZYSTKIEGO** (globalny; obsługuje watchdog/broker, < 200 ms) |

Reguła AltGr (PLAN §8.6): na polskiej klawiaturze `Ctrl+Alt` = AltGr, więc **żaden skrót globalny `Ctrl+Alt(+Shift)` z literami a, c, e, l, n, o, s, x, z** — test w CI. `F5` / `Ctrl+R` (przeładowanie WebView) wyłączone.

### 11.11 Przewijanie i czytanie

Automatyczne przewijanie przy strumieniu wyłącza się, gdy przewiniesz w górę; przycisk „↓ nowe (3)"; pozycja zachowana przy zmianie wariantu / gałęzi; separatory dni; blok „myślenie" zwinięty z czasem trwania; tabele z przewijaniem poziomym; KaTeX / Mermaid ładowane dopiero przy użyciu; błąd na poziomie wiadomości z akcją „Ponów / inny model"; szkielety ładowania zamiast spinnerów dłuższych niż 300 ms.

### 11.12 Cofanie jednym kliknięciem

Po każdej cofalnej akcji agentki (`fs.*`) toast „Cofnij" przez 8 s — skrót do dziennika cofania; karty kroków narzędzi mają przycisk „Cofnij" (np. „Delta: przeniesiono 14 plików · 2,1 s · Cofnij").

### 11.13 Język i formaty

Poprawne liczby mnogie PL („1 plik, 2 pliki, 5 plików"), żeńskie formy czasowników agentek, daty / liczby / waluta w formacie pl-PL; przełącznik PL/EN per sesja; „Co nowego" po aktualizacji; eksport sesji do MD / HTML / PDF.

### 11.14 Historia schowka w UI

Sekcja w palecie poleceń (wklej do composera), z wykluczeniem haseł i aplikacji z deny-listy oraz retencją.

## 12. Ustawienia — drzewo (§15)

Pliki `%APPDATA%\Alfa\config\*.toml` z JSON Schema; przeładowanie na żywo; profile widoku **Prosty / Zaawansowany / Ekspert**; historia w git z diffem i rollbackiem; **zmiany polityk Jądra tylko przez Broker** (potwierdzenie w jego oknie, opcjonalnie Hello). Każdy moduł dostarcza własną stronę ustawień z manifestu. Każde ustawienie ma: opis, wartość domyślną, zakres (globalny / sesja / agentka), reset, wyszukiwanie.

| Gałąź | Zawartość |
|---|---|
| Ogólne | start z systemem, zachowanie zamknięcia okna, profil widoku |
| Modele i dostawcy | konta, klucze (Hub kont, §5.6: dodawanie, test połączenia, wykrycie modeli, przypisanie do klas zadań / ról / głosu, limity, usuwanie), lokalne (llama.cpp), mosty CLI + karty zgodności |
| Router i reguły | klasy zadań, fallbacki, tagi prywatności i jurysdykcji, tryb „Rada" (P2) |
| **Głos** | urządzenia, STT, TTS, tury i barge-in, słowa wywoławcze, weryfikacja mówcy, dyktowanie, czytanie, słownik wymowy, Voice Lab |
| **Agentki** | Alfa / Beta / Gama / Delta (biblie głosu, charakter, kolor), obsada ról (szablony), Kreator |
| Uprawnienia i bezpieczeństwo | poziomy autonomii L0–L4 (przełącznik L4, globalnie / per sesja / per agentka / na czas), Windows Hello, szablony uprawnień, deny-listy |
| Komputer | trasy (API > UIA > wizja > wejście), aplikacje, przeglądarka, helper `uiAccess` |
| Pamięć | zakresy, konsolidacja nocna, Inspektor, `forget` |
| Sesje i okna | szablony sesji, karty, odłączone okna, limit okien |
| Pliki | katalog wyjściowy, podglądy, „Otwórz w Alfie" |
| Logi i prywatność | poziomy, retencja (GUI 7 dni), redakcja, „co poszło do chmury", paczka diagnostyczna |
| Samonaprawa i ulepszanie | pierścienie R0–R2, kolejka propozycji, Zdrowie systemu |
| **Moduły** | lista, włącz / wyłącz, budżety RAM/CPU, zwalnianie po bezczynności |
| **Urządzenia** | profil sprzętu per maszyna, profil głosu A–D, tryb baterii, tryb gry / pełnego ekranu |
| **Import i eksport** | paczki `.alfa`, kopie zapasowe z harmonogramem |
| Wygląd | motyw, akcent, gęstość, zoom, „bez animacji", szerokość kolumny |
| Skróty | wszystkie z §11.10, wykrywanie konfliktów, reguła AltGr |
| Powiadomienia | toasty, earcony, nie przeszkadzać, pasek zadań |
| Język | PL / EN, formaty pl-PL |
| Aktualizacje | kanał, „Co nowego", rollback do poprzedniej wersji |
| Zaawansowane | edytor surowy TOML, flagi, port CDP (tylko build testowy) |

## 13. Import / eksport `.alfa` w UI (§15.1)

Moduł `transfer`: F1 = P0-lite (konfiguracja + sesje), F7 = pełny (pamięć, umiejętności, kopie zapasowe). Bez automatycznej synchronizacji między maszynami.

### 13.1 Eksport (ekran nr 13)

| Element | Zachowanie |
|---|---|
| Format | jeden plik `.alfa` (archiwum + manifest z wersją schematu i sumami kontrolnymi) |
| Wybór zakresu (lista z przełącznikami) | konfiguracja wspólna · agentki i biblie głosów · obsady ról · reguły · umiejętności / agenci z Kreatora · **wybrane sesje** · **wybrane zakresy pamięci** · artefakty (opcjonalnie) · logi (domyślnie nie) · nakładka maszyny (domyślnie nie) |
| Sekrety | **klucze API i sekrety nie wchodzą do eksportu**; osobna, jawna opcja „eksport sekretów" szyfrowany hasłem |
| Szyfrowanie | opcjonalne, całej paczki, hasłem |
| Dostęp | Ustawienia → Import i eksport, paleta poleceń, `/eksport`, panel Sesje (pojedyncza sesja), głosem („Beta, wyeksportuj sesję X") |

### 13.2 Import

| Krok | Zachowanie |
|---|---|
| 1. Podgląd | zawartość paczki i różnice względem stanu lokalnego (dry-run) |
| 2. Tryb | *dodaj / scal / zastąp* |
| 3. Kolizje | rozwiązywanie per element (id sesji, wpisy pamięci) |
| 4. Migracja | upcastery schematu, komunikat o wersji |
| 5. Snapshot | **automatyczny snapshot przed importem** i jednoklikowy rollback |
| Dostęp | jak eksport + krok onboardingu (§7) |

### 13.3 Kopie zapasowe

Zaplanowany eksport (ten sam format i kod) do wskazanego katalogu, z rotacją; test przywracania w CI; wersjonowanie IR i schematu zdarzeń.

## 14. Ekrany do makiet (bramka ludzka #4)

Makiety powstają jako **klikalne strony w Storybooku na prawdziwych komponentach** (nie grafiki) — od razu mierzą budżety z §9. Każdy ekran w wariancie **jasnym i ciemnym** oraz w szerokości laptopa (1280 px efektywnie). Makiety 1–3 akceptujesz w F0; pozostałe przed falą, która je implementuje.

| # | Ekran | Zawartość | Fala |
|---|---|---|---|
| 1 | Start / pusty stan | pusta rozmowa z 3 sugestiami, composer, obsada w pasku, brak kluczy → podpowiedź „dodaj klucz" | F0 |
| 2 | Rozmowa | strumień odpowiedzi, kroki narzędzi zwinięte, karta „czeka na zatwierdzenie", warianty odpowiedzi `‹ 1/3 ›`, kapsuła aktywności, pasek akcji wiadomości | F0 |
| 3 | Pełny tryb głosowy | orb, napisy na żywo (szary partial → pełny), podświetlony prefiks i „przerwano tutaj", stany mikrofonu, przyciski Stop / Wycisz / Przełącz / Do tekstu | F0 |
| 4 | Pigułka głosowa | okno 220 × 48 px: awatar, fala głośności, stan mikrofonu, Stop / Wycisz | F5 |
| 5 | Szybkie pytanie | okno 640 px, pole tekstowe, rozwijana odpowiedź, `Enter` → pełne okno | F1 |
| 6 | Panel Agentki / obsada ról | cztery karty person (glif, kolor, rola, stan), szablony obsad, zmiana roli, przekazanie | F2 |
| 7 | Oś czasu i Replay | lista zdarzeń sesji, drzewo Plan → kroki → status (%, ETA, koszt), odtwarzanie krok po kroku, filtr strumieni | F1 (v0) / F3 |
| 8 | Artefakty i podgląd pliku | karty plików, podgląd / diff / wersje, akcje (Otwórz, Pokaż w Eksploratorze, Kopiuj, Zapisz jako, Spakuj, Wyślij, Przekaż) | F1 |
| 9 | Inspektor pamięci | warstwy i zakresy, wpisy z proweniencją / pewnością / TTL, `remember` / `forget`, awans za zgodą | F1 (v0) / F7 |
| 10 | Ekran (computer use) | podgląd na żywo z nakładką (ramki UIA, ścieżka kursora, opis kroku), Przejmij / Pauza / Stop | F6 |
| 11 | Ustawienia | drzewo z §12, wyszukiwarka, profil Prosty / Zaawansowany / Ekspert, strona modułu z manifestu | F1 |
| 12 | Hub kont i kluczy + kreator | lista dostawców ze stanem, kreator: wybór → klucz → test → wykrycie modeli → przypisanie → limit; mosty CLI z terminalem ConPTY | F1 (F4: mosty) |
| 13 | Import / eksport `.alfa` | wybór zakresu, sekrety osobno, hasło; import: dry-run, tryb, kolizje, snapshot / rollback | F1 |
| 14 | Onboarding | każdy krok z §7 jako osobna strona, w tym „pomiń — dodam później" | F1–F4 |
| 15 | Okno Brokera | karta zatwierdzenia (opis akcji, kolor ryzyka, zakres, Zezwól / Odmów / Zawsze w tym zakresie), przełącznik poziomów L0–L4, opcjonalne Hello; natywne okno, nie WebView | F3 |
| 16 | Voice Lab i casting głosów | tabela kandydatów, wyniki p50/p95 / WER / prefiks, ślepe A/B, odsłuch 4 głosów, edytor słownika wymowy | F2 |
| 17 | Zdrowie systemu | moduły i budżety, watchdog, kolejka propozycji Diagnosty / Ulepszacza (diff, ryzyko, plan cofnięcia), pulpit kosztów / opóźnień / limitów planów | F8 |
| 18 | Stany błędów / offline / brak kluczy | banery i karty z §6 | F1 |
| 19 | Kreator agentek | opis słowami → manifest (persona, rola, narzędzia, uprawnienia, pamięć, wyzwalacze, budżet) → test w piaskownicy → biblioteka | F5 |
| 20 | Menu zasobnika | ikona stanów, menu z §11.5 | F1 |

## 15. Powiązania

- Głos (automat dialogu, prefiks, profile): `docs/VOICE.md`.
- Persony, kolory i obsady: `docs/PERSONAS.md`.
- Broker-UI, poziomy autonomii, kill-switch: PLAN §8.
- Format paczki: `docs/formats/alfa-package.md`.
