# Archiwum regulaminów (`docs/compliance/archive/`)

Kopie źródeł, na których opierają się wpisy w `../compliance-registry.json` (PLAN.md §1.3 pkt 5, §18 „pobrać i zarchiwizować"). Na dziś katalog jest **pusty** poza tym README.

## Co archiwizujemy
- ToS / AUP / strona planu / polityka prywatności każdego dostawcy i każdej trasy z rejestru.
- Wpisy pomocy, dyskusje i posty użyte jako źródło (także wtórne [W] — z adnotacją, że nie są źródłem pierwotnym).
- Każdą kolejną wersję po zmianie treści (stara kopia zostaje; diff jest dowodem zmiany).
- Nie archiwizujemy: treści rozmów, tokenów, danych logowania, stron zalogowanych sesji.

## Układ i nazwy
```
archive/
  <provider>/<slug>/<YYYY-MM-DD>/
    page.html        surowa odpowiedź HTTP (bez modyfikacji)
    page.txt         tekst wyekstrahowany (do cytatów i diffów)
    meta.json        metadane (poniżej)
```
`<provider>` = `id` dostawcy z rejestru (np. `anthropic`), `<slug>` = krótka nazwa dokumentu (np. `legal-and-compliance`, `tos-privacy`).

## `meta.json`
```json
{
  "url": "https://...",
  "retrieved_at": "2026-09-29T14:05:00Z",
  "sha256_html": "<hex>",
  "sha256_txt": "<hex>",
  "http_status": 200,
  "content_type": "text/html; charset=utf-8",
  "source_kind": "primary | secondary",
  "registry_refs": ["routes/claude-code-cli", "providers/anthropic"],
  "retrieved_by": "manual | compliance-module",
  "notes": ""
}
```

## Jak archiwizować
1. Pobrać stronę z oficjalnej domeny (bez logowania, bez ciasteczek); zapisać `page.html` bajt w bajt.
2. Wyekstrahować tekst do `page.txt`; policzyć SHA-256 obu plików (`sha256sum`).
3. Wypełnić `meta.json`; w rejestrze wpisać `url`, `retrieved_at`, dosłowny `quote` i `confidence` (`V` dla źródła pierwotnego).
4. Przy ponownej weryfikacji porównać hash z ostatnią kopią: inny hash → nowa kopia w nowym katalogu z datą + ponowna ocena trasy (`../subscription-routes.md` §6–7).
5. Strony niedostępne dla pobrania (blokady) zostają w rejestrze jako `url: "TODO"` / `confidence: "W"` lub `"?"` — trasa pozostaje szara.

Kopie są dokumentacją; nie są redystrybuowane poza repo (użytek osobisty).
