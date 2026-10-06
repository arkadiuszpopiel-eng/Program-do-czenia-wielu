// Test dymny PRAWDZIWEJ aplikacji (PLAN §4.4: „E2E — Playwright przez CDP do WebView2”): powłoka
// Tauri z cechą `e2e`, rdzeń Rust (`app-core`), Broker w procesie (build debug). Kolejność testów
// odpowiada pierwszemu uruchomieniu: okno → wprowadzenie → rozmowa bez modelu → Broker → Modele
// i silniki → eksport `.alfa` → zatrzymanie z UI → okna poboczne → konsola → CSP/Trusted Types.
// Testy nie zależą od siebie: każdy sam dochodzi do rozmowy (`toChat`), więc porażka jednego
// nie zasłania reszty. Uruchamia `e2e-app/run.ps1` (workflow `rehearsal.yml`, job `app-e2e`).
import { mkdirSync, readFileSync } from 'node:fs';
import { basename, join } from 'node:path';
import type { Locator, Page } from '@playwright/test';
import {
  AXE_PHASE,
  OUT,
  PROBE_PHASE,
  describeFinding,
  driveSaveDialog,
  expect,
  expectAccessible,
  readFindings,
  shot,
  test,
} from './alfa';

/**
 * Szum środowiska runnera — NIE błędy Alfy. Każdy wpis z uzasadnieniem; reszta błędów konsoli
 * i wyjątków oblewa test. Nowy wpis tylko po sprawdzeniu, że to środowisko, nie regresja.
 */
const RUNNER_NOISE: readonly { pattern: RegExp; why: string }[] = [
  {
    pattern: /Test mikrofonu/,
    why: 'runner GitHub nie ma urządzeń audio; krok „Mikrofon” wprowadzenia startuje test mikrofonu',
  },
];

const ONBOARDING = 'Witaj w Alfie';
const MESSAGE = 'Cześć, Alfa! To test dymny E2E.';

test.beforeEach(({ alfa }, info) => {
  alfa.log.phase = info.title;
});

/** Rozmowa, wprowadzenie albo Ustawienia (to, co pokazał rdzeń albo zostawił poprzedni test). */
async function startView(page: Page): Promise<void> {
  const ready = page
    .locator('#alfa-composer')
    .or(page.getByRole('heading', { name: ONBOARDING }))
    .or(page.getByRole('button', { name: 'Wróć do rozmowy' }));
  await expect(ready.first()).toBeVisible({ timeout: 60_000 });
}

async function firstVisible(candidates: Locator[]): Promise<Locator> {
  for (const candidate of candidates) {
    if (await candidate.isVisible()) return candidate;
  }
  throw new Error('Wprowadzenie: brak przycisku przejścia dalej');
}

/** Pomija kroki wprowadzenia („Pomiń — dodam później”, „Później”, „Pomiń”, inaczej „Dalej”). */
async function skipOnboarding(page: Page): Promise<string[]> {
  const steps: string[] = [];
  const composer = page.locator('#alfa-composer');
  const counter = page.getByText(/^Krok \d+ z \d+$/);
  for (let i = 0; i < 12 && !(await composer.isVisible()); i++) {
    const before = (await counter.textContent({ timeout: 10_000 })) ?? '?';
    const button = await firstVisible([
      page.getByRole('button', { name: 'Pomiń — dodam później', exact: true }),
      page.getByRole('button', { name: 'Później', exact: true }),
      page.getByRole('button', { name: 'Pomiń', exact: true }),
      page.getByRole('button', { name: 'Zaczynamy', exact: true }),
      page.getByRole('button', { name: 'Dalej', exact: true }),
    ]);
    steps.push(`${before}: ${(await button.textContent())?.trim() ?? '?'}`);
    await button.click();
    await expect(async () => {
      // Krótki limit: w chwili przełączania widoku licznika może nie być (toPass ponowi).
      const now = (await composer.isVisible())
        ? 'rozmowa'
        : await counter.textContent({ timeout: 1_000 });
      expect(now).not.toBe(before);
    }).toPass({ timeout: 30_000 });
  }
  return steps;
}

/** Widok rozmowy (z Ustawień wraca, wprowadzenie pomija). */
async function toChat(page: Page): Promise<void> {
  await startView(page);
  const back = page.getByRole('button', { name: 'Wróć do rozmowy' });
  if (await back.isVisible()) await back.click();
  if (await page.getByRole('heading', { name: ONBOARDING }).isVisible()) await skipOnboarding(page);
  await expect(page.locator('#alfa-composer')).toBeVisible({ timeout: 30_000 });
}

async function openSettings(page: Page, section: string): Promise<void> {
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await page.keyboard.press('Control+,');
  await nav.getByRole('button', { name: section, exact: true }).click();
  await expect(page.getByRole('heading', { name: section, level: 2 })).toBeVisible();
}

test('okno główne startuje na prawdziwym rdzeniu (IPC Tauri, nie atrapa)', async ({ alfa }) => {
  const { main } = alfa;
  await expect(main).toHaveTitle('Alfa');
  const ipc = await main.evaluate(() => '__TAURI_INTERNALS__' in window);
  expect(ipc, 'okno ma IPC Tauri (TauriAlfaClient), nie atrapę przeglądarkową').toBe(true);
  await startView(main);
  // Pełna droga IPC pod CSP wydania (`connect-src ipc:`): komenda rdzenia i odpowiedź.
  const boot = await main.evaluate(() => {
    const ipcWindow = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string) => Promise<{ app_version: string }> };
    };
    return ipcWindow.__TAURI_INTERNALS__.invoke('app_bootstrap');
  });
  expect(boot.app_version).toMatch(/^\d+\.\d+\.\d+/);
  test.info().annotations.push({ type: 'wersja', description: boot.app_version });
  await shot(alfa, '01-start');
});

test('wprowadzenie: pominięcie kroków prowadzi do rozmowy', async ({ alfa }) => {
  const { main } = alfa;
  await startView(main);
  const onboarding = main.getByRole('heading', { name: ONBOARDING });
  test.skip(!(await onboarding.isVisible()), 'wprowadzenie już ukończone (profil nie jest świeży)');
  await shot(alfa, '02-wprowadzenie');
  await expectAccessible(alfa, 'wprowadzenie — krok 1');
  const steps = await skipOnboarding(main);
  test.info().annotations.push({ type: 'kroki', description: steps.join(' → ') });
  await expect(main.locator('#alfa-composer')).toBeVisible();
  await shot(alfa, '03-rozmowa-pusta');
});

test('rozmowa bez modelu: czytelny błąd „brak mózgu”', async ({ alfa }) => {
  const { main } = alfa;
  await toChat(main);
  const composer = main.locator('#alfa-composer');
  await composer.fill(MESSAGE);
  await composer.press('Enter');
  const conversation = main.getByRole('region', { name: 'Rozmowa' });
  await expect(conversation.getByText(MESSAGE, { exact: true }).first()).toBeVisible();
  // Rdzeń zwraca `no_keys` z komunikatem „Brak mózgu: …”; UI pokazuje tekst dla kodu błędu.
  const error = conversation
    .getByRole('alert')
    .filter({ hasText: /Brak dostępnego modelu|Brak mózgu/ })
    .last();
  await expect(error).toBeVisible({ timeout: 60_000 });
  await expect(error.getByRole('button', { name: 'Ponów' })).toBeVisible();
  await expect(composer).toHaveValue('');
  await shot(alfa, '04-brak-mozgu');
  await expectAccessible(alfa, 'rozmowa z błędem „brak mózgu”');
});

test('Broker w procesie (build debug): baner i stan w Ustawieniach', async ({ alfa }) => {
  const { main } = alfa;
  await toChat(main);
  await expect(main.getByRole('region', { name: 'Stan Brokera' })).toContainText(
    'Tryb deweloperski: Broker w procesie aplikacji, bez okna zatwierdzeń.',
  );
  await openSettings(main, 'Uprawnienia i bezpieczeństwo');
  await expect(main.getByText('Broker w procesie aplikacji (tryb deweloperski)')).toBeVisible();
  await expect(main.getByText(/awaryjnie w aplikacji — watchdog nie działa/)).toBeVisible();
  await expect(main.getByText(/Izolacja: słabsza/)).toBeVisible();
  await shot(alfa, '05-broker');
});

test('Ustawienia → Modele i silniki: lista z katalogu', async ({ alfa }) => {
  const { main } = alfa;
  await toChat(main);
  await openSettings(main, 'Modele i silniki');
  await expect(main.getByRole('heading', { name: 'Pozycje katalogu' })).toBeVisible({
    timeout: 30_000,
  });
  const counter = main.getByText(/^\d+ pozycj[aei]$/);
  await expect(counter).toBeVisible();
  const count = Number.parseInt((await counter.textContent()) ?? '0', 10);
  expect(count, 'pozycje katalogu app-models').toBeGreaterThanOrEqual(3);
  const rows = main.getByRole('listitem').filter({ has: main.getByRole('heading', { level: 4 }) });
  await expect.soft(rows).toHaveCount(count);
  await expect(main.getByRole('heading', { level: 4, name: /llama-server/ }).first()).toBeVisible();
  await expect(main.getByRole('heading', { level: 4, name: /Silero VAD/ }).first()).toBeVisible();
  test.info().annotations.push({ type: 'katalog', description: `${count} pozycji` });
  await shot(alfa, '06-modele-i-silniki');
  await expectAccessible(alfa, 'Ustawienia → Modele i silniki');
});

test('eksport .alfa do katalogu tymczasowego (natywne okno „Zapisz jako”)', async ({ alfa }) => {
  test.skip(process.platform !== 'win32', 'okno „Zapisz jako” obsługuje UI Automation Windows');
  const { main } = alfa;
  await toChat(main);
  await openSettings(main, 'Import i eksport');
  const dir = process.env['ALFA_E2E_EXPORT_DIR'] ?? join(OUT, 'export');
  mkdirSync(dir, { recursive: true });
  const target = join(dir, `alfa-e2e-${Date.now()}.alfa`);
  const dialog = driveSaveDialog(target);
  await main.getByRole('button', { name: 'Eksportuj…', exact: true }).click();
  const result = await dialog;
  await test
    .info()
    .attach('okno-zapisz-jako.log', { body: result.output, contentType: 'text/plain' });
  try {
    expect(result.code, `UI Automation okna „Zapisz jako”:\n${result.output}`).toBe(0);
    const saved = main.getByRole('status').filter({ hasText: /^Zapisano / });
    await expect(saved).toBeVisible({ timeout: 60_000 });
    await expect(saved).toContainText(basename(target));
    const head = readFileSync(target).subarray(0, 2).toString('latin1');
    expect(head, 'paczka .alfa to archiwum ZIP').toBe('PK');
  } finally {
    // Okno, którego nie udało się obsłużyć, nie może zostać otwarte dla kolejnych testów.
    if (result.code !== 0) await driveSaveDialog(null, 5);
  }
  await shot(alfa, '07-eksport');
});

test('zatrzymanie z UI (bez skrótu globalnego): „Zatrzymaj sterowanie” → „Sterujesz Ty”', async ({
  alfa,
}) => {
  // STOP WSZYSTKIEGO nie ma komendy IPC (skrót globalny / watchdog / zasobnik) — z okna jest
  // „Zatrzymaj sterowanie” (`gui_stop`): anuluje akcje GUI i tury, wstrzymuje narzędzia ekranu.
  const { main } = alfa;
  await toChat(main);
  await openSettings(main, 'Komputer');
  await main.getByRole('button', { name: 'Zatrzymaj sterowanie' }).click();
  await expect(
    main.getByText('Sterujesz Ty — narzędzia ekranu agentek są wstrzymane.').first(),
  ).toBeVisible();
  await shot(alfa, '08-zatrzymano');
  await main.getByRole('button', { name: 'Oddaj sterowanie' }).first().click();
  await expect(main.getByText('Żadna agentka nie steruje teraz ekranem.').first()).toBeVisible();
});

test('okna poboczne (Szybkie pytanie, pigułka) ładują się w WebView2', async ({ alfa }) => {
  const { quick, pill } = alfa.windows;
  expect(quick, 'okno „quick” wśród stron CDP').toBeTruthy();
  expect(pill, 'okno „pill” wśród stron CDP').toBeTruthy();
  if (quick) {
    await expect(quick).toHaveTitle('Alfa — Szybkie pytanie');
    await expect(quick.locator('#app main')).toBeAttached({ timeout: 30_000 });
  }
  if (pill) {
    await expect(pill).toHaveTitle('Alfa — pigułka głosowa');
    await expect(pill.locator('#pill [role="status"]')).toBeAttached({ timeout: 30_000 });
  }
});

test('konsola wszystkich okien: zero błędów JS i nieobsłużonych wyjątków', async () => {
  const errors = readFindings().filter(
    (f) =>
      f.phase !== PROBE_PHASE &&
      (f.kind === 'pageerror' || (f.kind === 'console' && f.level === 'error')),
  );
  const noise = errors.filter((f) => RUNNER_NOISE.some((n) => n.pattern.test(f.text)));
  for (const f of noise) {
    const why = RUNNER_NOISE.find((n) => n.pattern.test(f.text))?.why ?? '';
    test
      .info()
      .annotations.push({ type: 'szum runnera', description: `${describeFinding(f)} — ${why}` });
  }
  const real = errors.filter((f) => !noise.includes(f)).map(describeFinding);
  expect(real, 'błędy konsoli / nieobsłużone wyjątki (pełna lista: out/console.jsonl)').toEqual([]);
});

test('CSP z Trusted Types: zero naruszeń we wszystkich oknach, polityka wymuszana', async ({
  alfa,
}) => {
  const csp = readFindings().filter((f) => f.kind === 'csp' && f.phase !== PROBE_PHASE);
  // Naruszenie z fazy axe bez źródła w zasobach aplikacji = samo narzędzie (raport, nie porażka).
  const tooling = csp.filter((f) => f.phase.startsWith(AXE_PHASE) && !/\/assets\//.test(f.source));
  for (const f of tooling) {
    test.info().annotations.push({ type: 'naruszenie narzędzia', description: describeFinding(f) });
  }
  const real = csp.filter((f) => !tooling.includes(f)).map(describeFinding);
  expect(real, 'naruszenia CSP / Trusted Types (pełna lista: out/console.jsonl)').toEqual([]);

  // Sonda: CSP z tauri.conf.json naprawdę dotarła do WebView2 i Trusted Types są wymuszane.
  alfa.log.phase = PROBE_PHASE;
  const probe = await alfa.main.evaluate(() => {
    try {
      document.createElement('div').innerHTML = '<b>sonda</b>';
      return 'przypisanie przeszło — Trusted Types NIE są wymuszane';
    } catch (error) {
      return error instanceof TypeError ? 'zablokowane' : String(error);
    }
  });
  expect(probe, 'surowe innerHTML pod CSP wydania').toBe('zablokowane');
  const policy = await alfa.main.evaluate(() => {
    const tt = (globalThis as { trustedTypes?: { defaultPolicy: { name: string } | null } })
      .trustedTypes;
    return tt?.defaultPolicy?.name ?? null;
  });
  expect(policy, 'polityka `default` (tylko adresy skryptów z tego pochodzenia)').toBe('default');
});
