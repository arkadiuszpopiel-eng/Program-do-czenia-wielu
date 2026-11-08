# ADR 0009 — Markdown z LLM renderowany w Rust; sandboxowany iframe tylko dla artefaktów HTML/SVG

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §8.0, §8.2, §14.7, §14.8, §17 |

## Kontekst

Treść z modeli jest niezaufana (prompt injection ze stron, plików, maili). WebView2 z Tauri ma dostęp do komend IPC — XSS w oknie głównym może stać się RCE lub samozatwierdzeniem. Jednocześnie UI ma być szybkie (strumień 100 tok/s przy 60 kl./s, wirtualizacja 1000 wiadomości, `Ctrl+F`, czytniki ekranu) i pozwalać na podgląd artefaktów HTML/SVG tworzonych przez agentki.

## Decyzja

1. **Markdown z LLM renderowany do HTML w Rust**: `pulldown-cmark` + sanitizacja `ammonia`; bez surowego HTML, bez skryptów, bez inline event handlers. Wynik wstawiany do dokumentu jako zwykłe węzły DOM.
2. **Sandboxowany `<iframe>` bez IPC wyłącznie dla aktywnych artefaktów HTML/SVG** — jeden na podgląd, bez dostępu do `window.__TAURI__`, bez `allow-same-origin`.
3. **Ścisłe CSP** bez inline i Trusted Types w oknie głównym; minimalne capabilities Tauri per okno.
4. Port debugowania WebView2 (CDP) istnieje **wyłącznie w buildzie testowym**; test CI sprawdza, że w produkcji jest zamknięty.
5. Wydajność: przyrostowe parsowanie (zamknięte bloki się nie przerenderowują), podświetlanie składni leniwie w Web Workerze po zakończeniu bloku, aktualizacja DOM najwyżej raz na klatkę (bufor + `requestAnimationFrame`), KaTeX/Mermaid ładowane dopiero przy użyciu.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Renderowanie Markdown w JS (marked/markdown-it) w oknie głównym | sanitizacja po stronie niezaufanego kontekstu; większa paczka JS (budżet ≤ 150 KB gzip); parser w głównym wątku psuje budżet 50 ms |
| Każda wiadomość w osobnym iframe | zabija wirtualizację, zaznaczanie tekstu, `Ctrl+F`, `aria-live` i czytniki ekranu; koszt RAM |
| Surowy HTML z modelu w treści | wprost sprzeczne z modelem zagrożeń (XSS → RCE) |
| Artefakty HTML otwierane w zewnętrznej przeglądarce | traci podgląd w panelu Artefakty; nadal możliwe jako akcja „Otwórz" |
| Ograniczenie CSP bez Trusted Types | Trusted Types domykają `innerHTML` z kodu UI generowanego przez AI |

## Konsekwencje

- Moduł `ui-shell` otrzymuje z rdzenia gotowy, bezpieczny HTML per blok (przez IPC grupowane co klatkę); duże dane (obrazy, pliki) przez ścieżki i protokół zasobów, nie przez IPC.
- Testy: zestaw złośliwych Markdownów (skrypty, `javascript:`, SVG z handlerami, data-URI) w CI; test zamkniętego CDP w buildzie produkcyjnym.
- Konsekwencja dla a11y: treść jest zwykłym DOM → `aria-live` z throttlingiem, fokus, zoom działają.
- Iframe artefaktów nie ma IPC — interaktywne artefakty nie mogą wywoływać narzędzi Alfy (świadome ograniczenie).
- Kryterium F3 (red-team injection ≥ 100 przypadków: 0 eskalacji) obejmuje wektor Markdown/HTML.

## Jak cofnąć

- Wymiana `pulldown-cmark`/`ammonia` na inny parser w Rust nie zmienia kontraktu (`ui-shell` dostaje HTML).
- Dopuszczenie renderowania w JS wymagałoby rewizji `THREAT_MODEL.md` i nowego zestawu testów XSS; nie przewidujemy.
- Zdjęcie sandboxu z iframe artefaktów to zmiana polityki bezpieczeństwa — tylko przez właściciela (Jądro/Broker).
