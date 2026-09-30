# ADR 0010 — Reguła skrótów globalnych (polski AltGr) i kill-switch `Ctrl+Shift+F12`

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §6.5 (pkt 4–5), §7.3, §8.6, §14.8 (Skróty, Szybkie pytanie), §16.2 (F1, F3), §17 |

## Kontekst

Na polskiej klawiaturze programisty `Ctrl+Alt` = **AltGr**, a `AltGr + a/c/e/l/n/o/s/x/z` wpisuje polskie znaki (ą ć ę ł ń ó ś ź ż), z `Shift` — wielkie litery. Globalny skrót typu `Ctrl+Alt+Shift+S` przechwyciłby wpisywanie „Ś" w każdej aplikacji. Klawisz `Pause` nie występuje na laptopach. Kill-switch musi działać w < 200 ms poza UI, także gdy UI zawiesiło się lub gdy okno administratora jest na pierwszym planie (z ograniczeniami UIPI).

## Decyzja

1. **Reguła zakazu:** żaden skrót globalny nie używa `Ctrl+Alt` ani `Ctrl+Alt+Shift` z literami **a, c, e, l, n, o, s, x, z** (na układzie PL). Test w CI sprawdza tabelę domyślnych skrótów i schemat konfiguracji skrótów; UI wykrywa konflikty przy zmianie przez użytkownika.
2. **Kill-switch „STOP WSZYSTKIEGO":** domyślnie **`Ctrl+Shift+F12`** (konfigurowalny). Obsługuje go **Watchdog/Broker, nie UI**; od klawisza do ciszy audio i zabicia wszystkich Job Objects **< 200 ms p95 z 50 prób pod obciążeniem UI** (kryterium F3). Dostępny też z kapsuły, zasobnika i głosem („stop").
3. **Dwa zakresy:** *Stop mowy* (`Esc` / „stop") ≠ *Stop wszystkiego* (kill-switch). `Esc` działa kolejno: zamknij menu/dialog → stop mowy → stop generowania.
4. **Szybkie pytanie:** domyślnie `Ctrl+Alt+Space` (spacja nie jest literą z listy zakazu) z wykrywaniem konfliktów (np. PowerToys).
5. **Push-to-talk globalny** wymaga hooka `WH_KEYBOARD_LL` (zwykły skrót nie zgłasza puszczenia klawisza); przytrzymanie `Spacji` poza polem tekstowym = mów. Hooki, PTT i dyktowanie nie działają, gdy na pierwszym planie jest okno administratora bez helpera `uiAccess` — UI to komunikuje.
6. `F5`/`Ctrl+R` (przeładowanie WebView) wyłączone.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| `Pause`/`Break` jako kill-switch | brak na większości laptopów (w tym testowym) |
| `Ctrl+Alt+Shift+S` („Stop") | to „Ś" na układzie PL |
| Kill-switch obsługiwany w UI (Tauri global shortcut) | zawieszone UI = brak stopu; wymóg < 200 ms poza UI |
| Rejestrowanie skrótów tylko przez `RegisterHotKey` | nie zgłasza puszczenia klawisza → PTT niemożliwe |
| Brak reguły, tylko wykrywanie konfliktów w runtime | modele AI dopisujące skróty nie znają układu PL; potrzebny test w CI |

## Konsekwencje

- Tabela domyślnych skrótów (§14.8 planu) przechodzi test reguły; przykładowo `Ctrl+Shift+M`, `Ctrl+K`, `Alt+1…6`, `Ctrl+\` są dozwolone.
- `platform-windows` v1 (F1) dostarcza skróty globalne i hook klawiatury dla PTT; kill-switch przenosi się do Watchdoga/Brokera w F3.
- Skróty są zmienialne, z wykrywaniem konfliktów i regułą AltGr egzekwowaną także dla wartości użytkownika.
- Dokumentacja użytkownika: ściągawka `Ctrl+/` wyjaśnia ograniczenia przy oknach administratora.

## Jak cofnąć

- Zmiana domyślnego kill-switcha to zmiana konfiguracji, nie architektury; reguła AltGr zostaje.
- Rozszerzenie reguły na inne układy (np. niemiecki AltGr+q = @) wymaga tabeli układów w teście CI — dodatek, nie cofnięcie.
