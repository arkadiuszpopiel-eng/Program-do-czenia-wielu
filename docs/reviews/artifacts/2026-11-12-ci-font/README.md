# CI190, font systemowy i rzeczywisty Core — 12.11.2026

[Raport](../../2026-11-12-execution-records-personas.md) · [dane](raw-evidence.tar.gz) ·
[manifest](manifest.json) · [CI190](ci190-summary.json).

Archiwum65plików, SHA256 `88ef2b026fec86e33f48876256838ce3942f329cc0fad68a8d1e346a8eff230d`.
Lokalne logi/JSON/archiwa zachowują oryginalne bajty. GitHub zwrócił logi zdekodowane:
są jawnie nazwane *decoded.json*, bez deklaracji zgodności z oryginalnym ZIP logów.
Oryginalny ZIP paczkiUI i ZIP raportu silników są zachowane i mają zgodneSHA z API.
Brak wag modeli, kluczy, tokenówCLI lub prywatnych sesji; wszystkie konteksty są syntetyczne.

CI19045df:8/10zadań, Linux3630/0/27,Windows3652/0/48,UI960/1,Tauri5+4,Vitest593.
Nie anulowano testów. Windows1144/4,WebView263/0/5; pełna lista pominięć jest w logu.
Raport silników zawiera rzeczywisteCPU, nagranieSTT i wygenerowanyTTS, bezGPU/mikrofonu.

approval-font-red:0/2 na dokładnej paczceCI, DejaVuSans, przepełnienie przycisku.
approval-wrap-focus:48/48 po zawinięciu etykiety. approval-title-red:0/2 dla ściśniętego
nagłówka; approval-final-focus:48/48 po zmianie układu. Wszystkie pierwotne wyniki
są osobno; nie sumować nakładających się serii jako nowych testów.

records-live-compile zachowuje E0277 w nowym teście; records-live-tests ma1/0/3pominięte,
records-live-clippy0. To przygotowanie adaptera diagnostycznego, bez lokalnej próby modelu.
Realny Core30 naWindows ma dopiero zostać wykonany; wynik transportu nie jest oceną semantyki.

[PL po zmianie](approval-pl.png) · [EN po zmianie](approval-en.png).

Końcowa walidacja YAML, generatorów, Rustfmt, lintu, wersji, IPC i diffu: kod0.
[Osobne komendy i logi](validation-manifest.json); zapisano po archiwum, bez zmiany jego hasha.
