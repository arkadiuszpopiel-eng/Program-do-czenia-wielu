# Zarys lądów kokpitu Jarvis

Źródło: Natural Earth 5.1.2, `ne_110m_land.geojson`, domena publiczna.
URL: https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_110m_land.geojson
Warunki: https://www.naturalearthdata.com/about/terms-of-use/
SHA256: `9e0729ee253ca7d7a5c4ae9395fb1902264c5377c52e224d13dd85010e2835d9` (138160 bajtów).

`node scripts/cockpit-map.mjs` generuje lokalny SVG; `--check` w lint sprawdza źródło
i wynik. Projekcja równodługościowa: x=2(lon+180), y=2(90-lat), WGS84;
uproszczenie Douglasa–Peuckera z tolerancją 0,3 stopnia. Nie porównujemy powierzchni.
Południk 0° trafia do x=360, równik do y=180; ±180° do brzegów obrazu.

Warstwa zawiera wyłącznie lądy, bez granic państw, punktów użytkownika, połączeń,
geolokalizacji lub danych bieżących. Jest tłem referencyjnym; stany aplikacji mają
osobne, opisane karty. Brak zależności od serwera map, konta lub klucza API.
Aktualizacja danych odbywa się przy zmianie tego przypiętego źródła w przeglądanym commicie.
