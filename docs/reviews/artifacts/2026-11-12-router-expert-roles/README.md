# Dowody poprawki doboru modeli — 12 listopada 2026

[Pełny raport](../../2026-11-12-router-expert-roles.md) · [Surowe archiwum](raw-evidence.tar.gz) · [Manifest](manifest.json).

Archiwum: **54 pliki**, SHA256 `0d741f12441f348990071ac38277c7fbdce2a25c51c8b0b595d0ca7b76b09565`.
Wszystkie wpisy sprawdzone hashami, źródła w manifest.json zgodne z testowaną wersją.

Baza a836bb57 / PR88. Receipty wskazują HEAD bazowy i stan niecommitowanych źródeł;
końcowe pliki są również w expert-roles-source/crates/… . Nie przenosimy wyników
między różnymi wersjami źródeł. Narzędzia: Rust1.94.1, przypięte środowisko z run_logged.py.

- RED: pierwotne6 regresji0/6; obejście wskazówką i research6/2; follow-up0/1.
- GREEN: router-contract/impl/fake80/0/0, w tym13 kontroli ról; app-modules50/0/1ignored;
  app-core routing/model_routing/pipeline_routing7/0/0. Łącznie137/0/1ignored.
- Ignored: istniejący builtin_catalog_has_azure_openai, oczekujący decyzji o rejestrze
  zgodności. Nie zmieniono polityki i nie zaliczono tego pominięcia.
- Clippy3pakiety/all-targets -D warnings0; fmt --check0; katalog/HTML/verify0,105stron.
- Zachowane: porażka linkera SIGBUS przy pełnym dysku, późniejsze8 niezgodnych
  starych oczekiwań, usunięcie nieużywanego uchwytu i końcowy brak ostrzeżeń.
- expert-roles-cache-retired: usunięty własny cache14B po zakończonej diagnozie i
  potwierdzeniu braku mapowań w procesach. Wcześniejszy hash zachowany, bez nowego
  pełnego hashowania. Pozostały wyniki/piny i cache8B. Zwolniono7321313664B.
- expert-roles-previous-ci199: zakończony poprzedni przebieg na a836bb57; nie jest
  zaliczeniem nowego commitu. Poprzedniego workflow nie przerwano i nie ponowiono.

Wyniki dotyczą Linux, deterministycznego routera, prawdziwej ścieżki AppCore oraz
kontrolowanych portów/HTTPstubów. Nie dowodzą inferencji27B, rzeczywistego API,
wyszukiwania WWW, Windows/WebView2, RTX2060 12GB, mikrofonu lub autonomicznego
kodowania modeli. Domyślnych modeli i uprawnień Brokera nie przełączono.
