// Dziennik rozruchu (pl — źródło kluczy): sygnał po starcie i sekcja w Zdrowiu systemu.
export const plBoot = {
  'boot.toast.ok': 'Rozruch OK · {s} s',
  'boot.toast.warnings': 'Rozruch z uwagami · ostrzeżenia: {w} · {s} s',
  'boot.toast.errors': 'Rozruch z problemem · błędy: {e}, ostrzeżenia: {w}',
  'boot.showLog': 'Pokaż dziennik',
  'boot.title': 'Dziennik rozruchu',
  'boot.intro':
    'Wpisy dziennika od startu programu do końca rozruchu (procesy Jądra, Broker, watchdog, moduły, głos). Treści rozmów tu nie ma, sekrety są zamazane. Pełny zapis jest w pliku dziennika.',
  'boot.verdict.ok': 'Rozruch OK',
  'boot.verdict.warnings': 'Rozruch z uwagami',
  'boot.verdict.errors': 'Rozruch z problemem',
  'boot.summary': 'czas {s} s · ostrzeżenia: {w} · błędy: {e}',
  'boot.running': 'Rozruch trwa — wpisy jeszcze dochodzą.',
  'boot.onlyProblems': 'Tylko ostrzeżenia i błędy',
  'boot.copy': 'Kopiuj dziennik',
  'boot.copied': 'Dziennik rozruchu skopiowany.',
  'boot.refresh': 'Odśwież',
  'boot.file': 'Plik dziennika: {path}',
  'boot.dropped': 'Pominięto {n} dalszych wpisów (limit 500) — są w pliku dziennika.',
  'boot.empty': 'Brak wpisów.',
  'boot.loadFailed': 'Nie udało się odczytać dziennika rozruchu: {error}',
} as const;
