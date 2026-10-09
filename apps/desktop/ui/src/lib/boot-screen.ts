// Ekran ładowania z `index.html`: widać go od pierwszej klatki okna do chwili, gdy interfejs ma
// dane startowe. Tu tylko sterowanie: zapamiętany motyw, opis bieżącego kroku i schowanie.

const THEME_KEY = 'alfa.boot.theme';
/** Najkrótszy czas pokazania w oknie programu — krótsze mignięcie wygląda jak błąd rysowania. */
export const BOOT_MIN_MS = 700;
const FADE_MS = 280;

const started = typeof performance === 'undefined' ? 0 : performance.now();

/** Motyw z poprzedniego uruchomienia — żeby ekran ładowania nie mignął w złych kolorach. */
export function applyRememberedTheme(root: HTMLElement, storage: Pick<Storage, 'getItem'> | null) {
  try {
    const theme = storage?.getItem(THEME_KEY);
    if (theme === 'light' || theme === 'dark') root.setAttribute('data-theme', theme);
  } catch {
    // Pamięć niedostępna — zostaje motyw systemu.
  }
}

export function rememberTheme(storage: Pick<Storage, 'setItem'> | null, theme: string) {
  try {
    storage?.setItem(THEME_KEY, theme);
  } catch {
    // Jak wyżej.
  }
}

export function bootStep(text: string): void {
  const step = document.getElementById('alfa-boot-step');
  if (step) step.textContent = text;
}

/** Ile jeszcze trzymać ekran, żeby był widoczny co najmniej `minMs` od startu strony. */
export function remainingBootTime(now: number, startedAt: number, minMs: number): number {
  return Math.max(0, Math.round(minMs - (now - startedAt)));
}

/** Chowa ekran ładowania (z wygaszeniem) i usuwa go z dokumentu. */
export function finishBoot(minMs: number): void {
  const boot = document.getElementById('alfa-boot');
  if (!boot || boot.classList.contains('done')) return;
  window.setTimeout(
    () => {
      boot.classList.add('done');
      window.setTimeout(() => boot.remove(), FADE_MS);
    },
    remainingBootTime(performance.now(), started, minMs),
  );
}
