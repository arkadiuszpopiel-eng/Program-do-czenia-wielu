// Test dymny PRAWDZIWEJ aplikacji (PLAN §4.4: „E2E — Playwright przez CDP do WebView2”): powłoka
// Tauri z cechą `e2e`, rdzeń Rust (`app-core`), Broker w procesie (build debug). Kolejność testów
// odpowiada pierwszemu uruchomieniu: okno → wprowadzenie → rozmowa bez modelu → Broker → Modele
// i silniki → eksport `.alfa` → zatrzymanie z UI → okna poboczne → konsola → CSP/Trusted Types.
// Testy nie zależą od siebie: każdy sam dochodzi do rozmowy (`toChat`), więc porażka jednego
// nie zasłania reszty. Uruchamia `e2e-app/run.ps1` (workflow `rehearsal.yml`, job `app-e2e`).
import { mkdirSync, readFileSync, rmSync } from 'node:fs';
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

// Skala interfejsu to CSS `zoom`: strona nie może być wyższa niż okno (pole pisania znikało pod
// dolną krawędzią przy każdej skali powyżej 100 % — docs/STATUS.md, przegląd 2026-10-08).
test('powiększenie Ctrl+=: strona ma wysokość okna, pole pisania zostaje widoczne', async ({
  alfa,
}) => {
  const { main } = alfa;
  await toChat(main);
  const zoom = () => main.evaluate(() => Number(document.documentElement.style.zoom || 1));
  await main.locator('#alfa-composer').focus();
  await main.keyboard.press('Control+=');
  await main.keyboard.press('Control+=');
  await expect.poll(zoom, { message: 'skala po Ctrl+=' }).toBeGreaterThan(1);
  const m = await main.evaluate(() => {
    const composer = document.querySelector('#alfa-composer');
    return {
      inner: window.innerHeight,
      scroll: document.documentElement.scrollHeight,
      bottom: composer ? composer.getBoundingClientRect().bottom : -1,
    };
  });
  await shot(alfa, '04a-powiekszenie');
  await main.keyboard.press('Control+0');
  await expect.poll(zoom, { message: 'skala po Ctrl+0' }).toBe(1);
  expect(m.scroll, 'brak przewijania całej strony').toBeLessThanOrEqual(m.inner);
  expect(m.bottom, 'dół pola pisania').toBeGreaterThan(0);
  expect(m.bottom, 'dół pola pisania').toBeLessThanOrEqual(m.inner);
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

test('Ustawienia → Modele lokalne i silniki: lista z katalogu', async ({ alfa }) => {
  const { main } = alfa;
  await toChat(main);
  await openSettings(main, 'Modele lokalne i silniki');
  await expect(main.getByRole('heading', { name: 'Pozycje katalogu' })).toBeVisible({
    timeout: 30_000,
  });
  const counter = main.getByText(/^\d+ pozycj[aei]$/);
  await expect(counter).toBeVisible();
  const count = Number.parseInt((await counter.textContent()) ?? '0', 10);
  expect(count, 'pozycje katalogu app-models').toBeGreaterThanOrEqual(3);
  const catalog = main.locator('section[aria-labelledby="mm-list"]');
  const rows = catalog
    .getByRole('listitem')
    .filter({ has: main.getByRole('heading', { level: 4 }) });
  await expect.soft(rows).toHaveCount(count);
  await expect(main.getByRole('heading', { level: 4, name: /llama-server/ }).first()).toBeVisible();
  await expect(main.getByRole('heading', { level: 4, name: /Silero VAD/ }).first()).toBeVisible();
  test.info().annotations.push({ type: 'katalog', description: `${count} pozycji` });
  // Pakiety 1–6 na prawdziwym rdzeniu (profil urządzenia runnera, stan z katalogu): sześć kart
  // od „Wzorcowy” do „Minimalny”, bez komunikatu „Nie udało się wczytać”.
  const bundles = main.locator('section[aria-labelledby="mm-bundles"]');
  const failed = bundles.getByRole('alert');
  const loaded = bundles.getByRole('img', { name: /^Ocena \d z 6$/ });
  await expect(loaded.or(failed).first()).toBeVisible({ timeout: 30_000 });
  if (await failed.count()) {
    throw new Error(`pakiety nie wczytały się: ${await failed.first().innerText()}`);
  }
  await expect(loaded).toHaveCount(6);
  await expect(bundles.getByText('Zalecany dla tego komputera', { exact: true })).toHaveCount(1);
  const machine = bundles.getByText(/^Ten komputer:/);
  await expect.soft(machine, 'profil urządzenia w opisie pakietów').toBeVisible();
  if (await machine.count()) {
    test.info().annotations.push({ type: 'pakiety', description: await machine.innerText() });
  }
  await shot(alfa, '06-modele-i-silniki');
  await expectAccessible(alfa, 'Ustawienia → Modele lokalne i silniki');
});

// Błąd z wersji 0.0.3: lista wyboru była systemowa i w stylu „Szkło” nieczytelna. Sprawdzamy ją
// otwartą w prawdziwym WebView2, w każdym stylu, ze zrzutem do przejrzenia.
test('Ustawienia → Wygląd: lista wyboru Alfy otwarta w każdym stylu', async ({ alfa }) => {
  const { main } = alfa;
  await toChat(main);
  await openSettings(main, 'Wygląd');
  await expect(main.locator('select')).toHaveCount(0);
  const style = main.getByRole('combobox', { name: 'Styl', exact: true });
  try {
    for (const skin of ['glass', 'next', 'classic']) {
      await style.click();
      const list = main.getByRole('listbox', { name: 'Styl' });
      await expect(list.getByRole('option')).toHaveCount(3);
      await list.locator(`[role="option"][data-value="${skin}"]`).click();
      await expect(style).toHaveAttribute('data-value', skin);
      await style.click();
      const look = await list.evaluate((el) => {
        const probe = document.createElement('canvas').getContext('2d');
        const rgba = (color: string) => {
          if (!probe) return [0, 0, 0, 255];
          probe.clearRect(0, 0, 1, 1);
          probe.fillStyle = color;
          probe.fillRect(0, 0, 1, 1);
          return [...probe.getImageData(0, 0, 1, 1).data];
        };
        const lum = ([r = 0, g = 0, b = 0]: number[]) => {
          const ch = (v: number) => {
            const c = v / 255;
            return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
          };
          return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b);
        };
        const bg = rgba(getComputedStyle(el).backgroundColor);
        const option = el.querySelector('[role="option"]:not(.active)') ?? el;
        const fg = rgba(getComputedStyle(option).color);
        const [hi, lo] = lum(bg) > lum(fg) ? [lum(bg), lum(fg)] : [lum(fg), lum(bg)];
        return { alpha: bg[3] ?? 0, contrast: (hi + 0.05) / (lo + 0.05) };
      });
      expect(look.alpha, `tło listy kryjące (${skin})`).toBe(255);
      expect(look.contrast, `kontrast opcji (${skin})`).toBeGreaterThanOrEqual(4.5);
      await shot(alfa, `06a-lista-wyboru-${skin}`);
      await main.keyboard.press('Escape');
      await expect(list).toHaveCount(0);
    }
  } finally {
    // Jedna aplikacja obsługuje wszystkie testy — styl nie może zostać po nieudanej asercji.
    // Escape tylko przy otwartej liście: przy zamkniętej zamyka Ustawienia i pole stylu znika
    // (wcześniej test przechodził dzięki wyścigowi z zamykaniem widoku).
    if ((await main.getByRole('listbox', { name: 'Styl' }).count()) > 0)
      await main.keyboard.press('Escape');
    if ((await style.getAttribute('data-value')) !== 'classic') {
      await style.click();
      await main.locator('[role="option"][data-value="classic"]').click();
    }
  }
  await expect(main.locator('html')).not.toHaveAttribute('data-skin', /.+/);
});

// Zgłoszenie właściciela (2026-10-09): prawy przycisk otwierał systemowe menu WebView2. W prawdziwym
// WebView2 sprawdzamy własne menu Alfy i to, że „Wklej” naprawdę czyta schowek systemu.
test('prawy przycisk w polu pisania: menu Alfy, wklejanie ze schowka systemu', async ({ alfa }) => {
  const { main } = alfa;
  await toChat(main);
  const field = main.locator('#alfa-composer');
  await field.fill('Ala ma kota');
  await field.selectText();
  await field.click({ button: 'right' });
  const menu = main.getByRole('menu', { name: 'Menu edycji' });
  await expect(menu).toBeVisible();
  await expect(menu.getByRole('menuitem')).toHaveCount(5);
  await shot(alfa, '06b-menu-kontekstowe');
  await menu.getByRole('menuitem', { name: /^Wytnij/ }).click();
  await expect(field).toHaveValue('');
  await field.click({ button: 'right' });
  await menu.getByRole('menuitem', { name: /^Wklej/ }).click();
  await expect(field).toHaveValue('Ala ma kota');
  await field.fill('');
  // Poza polem i bez zaznaczenia prawy przycisk nie otwiera niczego.
  await main.locator('header').first().click({ button: 'right' });
  await expect(menu).toHaveCount(0);
});

// Zgłoszenie właściciela (2026-10-09): górna belka ma być belką Alfy z paskiem LED. W prawdziwym
// oknie sprawdzamy, że belki systemowej nie ma, a przyciski okna naprawdę zmieniają jego rozmiar.
test('belka okna: bez belki systemowej, pasek LED, maksymalizacja i przywrócenie', async ({
  alfa,
}) => {
  const { main } = alfa;
  await toChat(main);
  const frame = await main.evaluate(() => window.outerHeight - window.innerHeight);
  expect(frame, 'wysokość ramki systemowej nad stroną').toBeLessThan(12);
  await expect(main.locator('header.titlebar .led')).toBeVisible();
  const controls = main.getByRole('group', { name: 'Okno' });
  const width = () => main.evaluate(() => window.outerWidth);
  const before = await width();
  await controls.getByRole('button', { name: 'Maksymalizuj' }).click();
  await expect(controls.getByRole('button', { name: 'Przywróć rozmiar' })).toBeVisible();
  await expect.poll(width).toBeGreaterThan(before);
  // Także zmaksymalizowane okno nie ma belki systemowej, a znak Alfy jest na naszej belce.
  expect(await main.evaluate(() => window.outerHeight - window.innerHeight)).toBeLessThan(20);
  await expect(main.locator('header.titlebar .brand svg.mark')).toBeVisible();
  await shot(alfa, '06c-belka-zmaksymalizowana');
  await controls.getByRole('button', { name: 'Przywróć rozmiar' }).click();
  await expect(controls.getByRole('button', { name: 'Maksymalizuj' })).toBeVisible();
  await expect.poll(width).toBe(before);
  await shot(alfa, '06d-belka');
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
    // Ścieżka z komunikatu rdzenia („Zapisano <ścieżka> · …”) — ta, którą zwróciło okno systemowe.
    const status = (await saved.textContent()) ?? '';
    const written = /^Zapisano (.+?\.alfa)/.exec(status)?.[1] ?? '';
    expect(written, `komunikat: ${status}`).not.toBe('');
    if (basename(written) !== basename(target)) {
      // Okno przyjęło inną nazwę niż wpisana (np. inne wydanie Windows) — eksport i tak sprawdzamy.
      test.info().annotations.push({
        type: 'warning',
        description: `okno zapisu zwróciło ${written} zamiast ${target}`,
      });
    }
    const head = readFileSync(written).subarray(0, 2).toString('latin1');
    expect(head, 'paczka .alfa to archiwum ZIP').toBe('PK');
    if (written !== target) rmSync(written, { force: true });
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

// Zlecenie właściciela (2026-10-09): system kamer. W prawdziwym oknie: kamery są domyślnie
// wyłączone; po włączeniu podgląd prawdziwej kamery pokazuje obraz, a belka — znak kamery; po
// wyłączeniu wszystko gaśnie. Na maszynie bez kamery (CI) sprawdzamy pusty stan.
test('Ustawienia → Kamery: wyłączone domyślnie, podgląd prawdziwej kamery, wskaźnik w belce', async ({
  alfa,
}) => {
  const { main } = alfa;
  await toChat(main);
  await openSettings(main, 'Kamery');
  const master = main.getByRole('switch', { name: 'Kamery włączone' });
  await expect(master).toHaveAttribute('aria-checked', 'false');
  const indicator = main.locator('header.titlebar button.cam');
  await expect(indicator).toHaveCount(0);
  const cameras = main.locator('li.camera');
  await expect(main.getByRole('heading', { name: 'Twoje kamery' })).toBeVisible();
  // Lista albo pusty stan — zależnie od sprzętu.
  await expect(cameras.first().or(main.getByText('Nie widzę żadnej kamery'))).toBeVisible();
  const count = await cameras.count();
  test.info().annotations.push({ type: 'kamery', description: String(count) });
  if (count === 0) return;

  const first = cameras.first();
  await expect(first.getByRole('button', { name: 'Podgląd', exact: true })).toBeDisabled();
  await master.click();
  try {
    const preview = first.getByRole('button', { name: 'Podgląd', exact: true });
    await expect(preview).toBeEnabled();
    await preview.click();
    // Prawdziwa klatka: obraz o rozsądnym rozmiarze, stan „działa”, znak kamery w belce.
    const image = first.locator('img');
    await expect(image).toBeVisible({ timeout: 20_000 });
    const size = await image.evaluate((el: HTMLImageElement) => [
      el.naturalWidth,
      el.naturalHeight,
    ]);
    expect(size[0], 'szerokość klatki').toBeGreaterThanOrEqual(64);
    expect(size[1], 'wysokość klatki').toBeGreaterThanOrEqual(36);
    await expect(first.locator('.state')).toHaveText('działa');
    await expect(indicator).toBeVisible();
    // Zrzut tylko belki — obrazu z kamery nie zapisujemy do artefaktów testu.
    await first.getByRole('button', { name: 'Zakończ podgląd' }).click();
    await expect(image).toHaveCount(0);
  } finally {
    // Zawsze wyłącz kamery po teście.
    if ((await master.getAttribute('aria-checked')) === 'true') await master.click();
  }
  await expect(indicator).toHaveCount(0, { timeout: 15_000 });
  await main.keyboard.press('Escape');
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
