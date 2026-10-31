// Skróty globalne na PRAWDZIWEJ instancji testowej (docs/plan/45 §6): rejestruje je powłoka w systemie
// (`src-tauri/src/shortcuts.rs`). Test nie wstrzykuje klawiszy w sesję Windows — właściciel może w tym
// czasie pisać w innym programie. Polecenie budowy testowej `e2e_global_shortcut` uruchamia tę samą
// obsługę naciśnięcia co system i mówi, czy skrót jest zarejestrowany przez tę instancję. Dyktowanie
// i czytanie zaznaczenia działają na oknie na pierwszym planie: polecenie najpierw daje fokus oknu
// instancji testowej i odmawia bez niego, więc nic nie sięga do innych programów. Profil testowy nie
// ma modeli mowy — dyktowanie i czytanie odmawiają komunikatem w oknie; gdyby jednak ruszyły, test je
// zatrzymuje. STOP WSZYSTKIEGO na końcu. Plik biegnie po `z-terminal` i przed `zz-mode`.
import type { Page } from '@playwright/test';
import { expect, test } from './alfa';
import { call, raw } from './ipc-call';

interface BootReport {
  lines: { level: string; text: string }[];
}

interface VoiceFeatures {
  dictation: { state: string };
  read: { state: string };
}

/** Teksty komunikatów w oknie (obszar „Powiadomienia”). */
async function toasts(page: Page): Promise<string[]> {
  return page.locator('.toasts > *').allInnerTexts();
}

/** Komunikat, którego nie było przed naciśnięciem (komunikaty znikają same — odpytywanie co 100 ms). */
async function newToast(page: Page, before: string[], what: string): Promise<string> {
  let seen = '';
  await expect
    .poll(
      async () => {
        seen = (await toasts(page)).find((t) => !before.includes(t)) ?? '';
        return seen;
      },
      { message: `${what}: komunikat po naciśnięciu`, timeout: 15_000, intervals: [100] },
    )
    .not.toBe('');
  return seen;
}

/**
 * Naciśnięcie skrótu `keys` obsługą powłoki. Skrót niezarejestrowany jest błędem, chyba że dziennik
 * rozruchu zgłasza go jako zajęty przez inny program (wtedy adnotacja, obsługa i tak sprawdzona).
 */
async function press(page: Page, keys: string): Promise<void> {
  const reply = await raw(page, 'e2e_global_shortcut', { keys });
  expect(reply.ok, `e2e_global_shortcut ${keys}: ${JSON.stringify(reply)}`).toBe(true);
  if (reply.ok && reply.value === true) return;
  // `boot_report` należy do powłoki (poza schematem łącza rdzenia) — odczyt bez sprawdzania schematu.
  const boot = await raw(page, 'boot_report', {});
  const lines = (boot.ok ? (boot.value as BootReport).lines : []).map((l) => l.text);
  const taken = lines.find(
    (l) => l.includes('rejestracja skrótu globalnego') && l.includes(`skrot=${keys} `),
  );
  expect(taken, `${keys} niezarejestrowany bez wpisu w dzienniku rozruchu`).toBeDefined();
  test.info().annotations.push({ type: 'skrót zajęty w systemie', description: taken ?? '' });
}

/**
 * Widok rozmowy (obszar powiadomień jest w widoku programu): z Ustawień, w których zostawia okno
 * poprzedni plik testów, powrót przyciskiem; przy samodzielnym biegu tego pliku (świeży profil)
 * wprowadzenie kończone poleceniem rdzenia i okno wczytane ponownie.
 */
async function programView(page: Page): Promise<void> {
  const composer = page.locator('#alfa-composer');
  const onboarding = page.getByRole('heading', { name: 'Witaj w Alfie' });
  const back = page.getByRole('button', { name: 'Wróć do rozmowy' });
  await expect(composer.or(onboarding).or(back).first()).toBeVisible({ timeout: 30_000 });
  if (await back.isVisible()) await back.click();
  if (await onboarding.isVisible()) {
    await call(page, 'app_complete_onboarding');
    await page.reload();
  }
  await expect(page.locator('.toasts')).toBeAttached({ timeout: 30_000 });
}

test.describe.configure({ mode: 'serial' });

test.beforeEach(async ({ alfa }, info) => {
  alfa.log.phase = info.title;
  await programView(alfa.main);
});

test('Ctrl+Alt+D — dyktowanie (voice.dictation) przez skrót globalny', async ({ alfa }) => {
  const before = await toasts(alfa.main);
  await press(alfa.main, 'Ctrl+Alt+D');
  const features = await call<VoiceFeatures>(alfa.main, 'voice_features');
  if (features.dictation.state === 'active') {
    // Profil z modelami mowy: dyktowanie ruszyło — zatrzymane od razu (pisze do okna testowego).
    await call(alfa.main, 'voice_dictation', { action: { kind: 'stop' } });
    return;
  }
  const message = await newToast(alfa.main, before, 'dyktowanie');
  // Ta sama odmowa co z łącza: skrót wywołał dyktowanie, a nie coś innego.
  const reply = await raw(alfa.main, 'voice_dictation', { action: { kind: 'start' } });
  expect(reply.ok, 'dyktowanie bez modeli mowy odmawia').toBe(false);
  if (!reply.ok) expect(message).toContain(reply.error.message ?? '');
});

test('Ctrl+Alt+R — czytanie zaznaczenia (voice.readSelection) przez skrót globalny', async ({
  alfa,
}) => {
  const before = await toasts(alfa.main);
  await press(alfa.main, 'Ctrl+Alt+R');
  const features = await call<VoiceFeatures>(alfa.main, 'voice_features');
  if (features.read.state === 'speaking') {
    await call(alfa.main, 'voice_read', { action: { kind: 'control', control: 'stop' } });
    return;
  }
  const message = await newToast(alfa.main, before, 'czytanie zaznaczenia');
  expect(message.length).toBeGreaterThan(3);
});

test('Ctrl+Shift+F12 — STOP WSZYSTKIEGO (system.killSwitch) przez skrót globalny', async ({
  alfa,
}) => {
  const before = await toasts(alfa.main);
  await press(alfa.main, 'Ctrl+Shift+F12');
  const message = await newToast(alfa.main, before, 'STOP WSZYSTKIEGO');
  expect(message).toContain('STOP WSZYSTKIEGO');
  // Po zatrzymaniu program dalej odpowiada (rdzeń nie zawisł na zatrzymywaniu).
  await call(alfa.main, 'voice_features');
});

test('nieznany skrót — odmowa bez skutku', async ({ alfa }) => {
  const reply = await raw(alfa.main, 'e2e_global_shortcut', { keys: 'Ctrl+Alt+Q' });
  expect(reply.ok).toBe(false);
});
