# Naprawy Alfy — 24.10.2026

Poprawki w lokalnej wersji 0.0.72 (build roboczy z oznaczeniem dirty).
Istniejące przed zadaniem zmiany w repozytorium zostały zachowane.

- Limit ustawienia `agents.max_steps`: 5–500, zabezpieczenie górnej granicy w rdzeniu.
- Jeden budżet kroków, czasu, tokenów i kosztu dla orkiestracji i jej potomków;
  usunięty ukryty limit 8 rund. Końcowe zużycie obejmuje również ostatnie narzędzie.
- Profile każdej persony: według ról, pełny, odczyt, własny, bez narzędzi.
  Alfa domyślnie pełny; profile obejmują czat, delegacje, Krytyczkę i nowe zadania
  w tle. Zmiana roli instrukcji etapu nie rozszerza narzędzi przy profilu według ról.
- Przekroczenie rozmiaru kopii powłoki jest błędem kopii, nie odmową Brokera.
  Agentka dostaje wskazanie narzędzia plikowego albo węższego katalogu.
- Pipeline: oddzielna lista etapów i edytor, zachowanie szkiców, ponowienie po
  błędzie, układ responsywny. Dostawca i model są dobierane dla każdego etapu.
- Wyśrodkowane ikony mają osobne menu z konfiguracją, rolami, głosem i adresowaniem.
  Kontrolki belki nie są objęte nadrzędnym obszarem przeciągania okna Tauri.
- Nawigacja ustawień po prawej stronie.

## Sprawdzenia

- Vitest: 491 testów; svelte-check: 0 błędów i ostrzeżeń.
- Testy przeglądarkowe: nowe kontrolki i regresja układów (27 testów), dodatkowo
  kontrola nawigacji i czytelności motywów. Osobna regresja obszaru przeciągania: 6/6.
- Testy Rust: narzędzia, profile, delegacje, zadania, Pipeline i routing etapów;
  round-trip DTO i sygnatury IPC. Clippy zmienianych modułów bez ostrzeżeń.
- Przegląd osobnym kontekstem: wykryte problemy filtrów dostępu, zadań w tle,
  końcowego zużycia i szkiców Pipeline poprawione oraz objęte regresjami.

## Kopia właściciela i weryfikacja natywna

Zbudowane i wgrane `alfa-desktop.exe` oraz `alfa_desktop_lib.dll` w
`../Alfa-uruchomiona`. Build bez portu debugowania WebView. Poprzednie pliki:
`target/repair-rollback-2026-10-24` (kopiowanie binariów, bez zmian bazy danych).

Po przerwaniu pierwszej próby klawiszem Esc kontrola została dokończona w kolejnej
turze: natywne okno pokazało wyśrodkowaną obsadę, przejście z Bety do jej profilu,
zakres 5–500, nawigację ustawień po prawej oraz przebudowany edytor Pipeline.
Nie wysyłano wiadomości do płatnego modelu. Odczytany po restarcie globalny
poziom Brokera: L3; profil narzędzi Alfy jest pełny, ale nie zastępuje decyzji Brokera.

Nie zmieniano reguł Brokera, deny-list, audytu ani zgód systemu Windows.
Wyniki na atrapach nie są pomiarem czasu odpowiedzi dostawców API.
