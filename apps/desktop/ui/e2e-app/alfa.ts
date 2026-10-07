// E2E prawdziwej aplikacji (PLAN §4.4): Playwright łączy się przez CDP z WebView2 powłoki Tauri
// zbudowanej z cechą `e2e` (port 9222 — `src-tauri/src/cdp.rs`), odnajduje okna `main`/`quick`/
// `pill` i zbiera wszystko, co strona zgłasza: błędy i ostrzeżenia konsoli, nieobsłużone wyjątki,
// naruszenia CSP / Trusted Types (zdarzenie `securitypolicyviolation` w stronie + problemy CDP
// `Audits`, które obejmują też okres przed podłączeniem). Zapis na bieżąco do `out/console.jsonl`,
// więc ocena na końcu przebiegu widzi także wpisy z procesów roboczych zrestartowanych po porażce.
import AxeBuilder from '@axe-core/playwright';
import { spawn } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, expect, test as base, type Browser, type Page } from '@playwright/test';

/** Adres CDP WebView2 (`--remote-debugging-port=9222` z `cdp::browser_args`). */
export const CDP_URL = process.env['ALFA_CDP_URL'] ?? 'http://127.0.0.1:9222';
/** Katalog artefaktów (zrzuty, dziennik konsoli, raporty) — job `app-e2e` w `rehearsal.yml`. */
export const OUT = process.env['ALFA_E2E_OUT'] ?? fileURLToPath(new URL('./out', import.meta.url));
const FINDINGS = join(OUT, 'console.jsonl');

/** Faza sondy Trusted Types (celowe naruszenie — wyłączone z oceny). */
export const PROBE_PHASE = 'sonda Trusted Types';
/** Przedrostek faz pomiaru axe (osobno w raporcie, gdyby narzędzie samo coś naruszyło). */
export const AXE_PHASE = 'axe';

export type WindowName = 'main' | 'quick' | 'pill';
export type FindingKind = 'csp' | 'pageerror' | 'console' | 'audit';

export interface Finding {
  kind: FindingKind;
  level: string;
  window: WindowName;
  phase: string;
  text: string;
  source: string;
  at: string;
}

/** Komunikaty konsoli o CSP i Trusted Types (Chromium/WebView2 zgłasza je po angielsku). */
const CSP_TEXT =
  /Content[- ]Security[- ]Policy|Trusted ?Types?|TrustedHTML|TrustedScript|require-trusted-types-for/i;

/** Faza wpisów odtworzonych przez `Audits.enable` (problemy sprzed podłączenia CDP). */
export const HISTORY_PHASE = 'przed podłączeniem (Audits)';

/** Dziennik ustaleń (plik JSON Lines dopisywany na bieżąco). */
export class Findings {
  phase = 'start';
  private readonly seen = new Set<string>();

  constructor() {
    mkdirSync(OUT, { recursive: true });
    // Po restarcie procesu roboczego `Audits.enable` odtwarza to, co już zapisano — bez duplikatów.
    for (const f of readFindings()) this.seen.add(keyOf(f.kind, f.window, f.text, f.source));
  }

  add(
    kind: FindingKind,
    level: string,
    window: WindowName,
    text: string,
    source = '',
    phase = this.phase,
  ): void {
    const key = keyOf(kind, window, text, source);
    if (this.seen.has(key)) return;
    this.seen.add(key);
    const finding: Finding = {
      kind,
      level,
      window,
      phase,
      text,
      source,
      at: new Date().toISOString(),
    };
    appendFileSync(FINDINGS, `${JSON.stringify(finding)}\n`, 'utf8');
  }
}

function keyOf(kind: FindingKind, window: WindowName, text: string, source: string): string {
  return `${kind}|${window}|${text}|${source}`;
}

/** Wszystkie ustalenia z bieżącego przebiegu (wszystkie procesy robocze). */
export function readFindings(): Finding[] {
  if (!existsSync(FINDINGS)) return [];
  return readFileSync(FINDINGS, 'utf8')
    .split('\n')
    .filter((line) => line.trim() !== '')
    .map((line) => JSON.parse(line) as Finding);
}

/** Zwięzły opis ustalenia do komunikatu asercji. */
export function describeFinding(f: Finding): string {
  const where = f.source ? ` @ ${f.source}` : '';
  return `[${f.window} · ${f.phase}] ${f.level}: ${f.text.split('\n')[0] ?? ''}${where}`;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

interface Violation {
  directive: string;
  blocked: string;
  sample: string;
  file: string;
  line: number;
  disposition: string;
}

async function watch(page: Page, window: WindowName, log: Findings): Promise<void> {
  page.on('console', (msg) => {
    const type = msg.type();
    if (type !== 'error' && type !== 'warning') return;
    const text = msg.text();
    const at = msg.location();
    log.add(
      CSP_TEXT.test(text) ? 'csp' : 'console',
      type,
      window,
      text,
      at.url ? `${at.url}:${at.lineNumber}` : '',
    );
  });
  page.on('pageerror', (error) => {
    log.add('pageerror', 'error', window, error.stack ?? `${error.name}: ${error.message}`);
  });
  // `securitypolicyviolation` ze strony (dokładny `sample`) — nazwa wiązania unikalna dla
  // połączenia, bo po restarcie procesu roboczego stara definicja zostaje w dokumencie.
  const binding = `__alfaE2eViolation${Date.now()}`;
  await page.exposeFunction(binding, (v: Violation) => {
    const text = `${v.directive} (${v.disposition}): ${v.blocked} ${v.sample}`.trim();
    log.add('csp', 'error', window, text, v.file ? `${v.file}:${v.line}` : '');
  });
  await page.addInitScript((name: string) => {
    document.addEventListener('securitypolicyviolation', (e) => {
      const report = (window as unknown as Record<string, ((v: Violation) => void) | undefined>)[
        name
      ];
      report?.({
        directive: e.effectiveDirective,
        blocked: e.blockedURI,
        sample: e.sample,
        file: e.sourceFile,
        line: e.lineNumber,
        disposition: e.disposition,
      });
    });
  }, binding);
  // Problemy CDP: CSP (także sprzed podłączenia — `Audits.enable` odtwarza je przed odpowiedzią,
  // stąd faza `HISTORY_PHASE`) oraz inne problemy strony (informacyjnie, bez identyfikatorów węzłów).
  const cdp = await page.context().newCDPSession(page);
  let replay = true;
  cdp.on('Audits.issueAdded', ({ issue }) => {
    const phase = replay ? HISTORY_PHASE : log.phase;
    const csp = issue.details.contentSecurityPolicyIssueDetails;
    if (csp) {
      const loc = csp.sourceCodeLocation;
      const text = `${csp.contentSecurityPolicyViolationType}: ${csp.violatedDirective} ${csp.blockedURL ?? ''}`;
      const level = csp.isReportOnly ? 'warning' : 'error';
      log.add('csp', level, window, text.trim(), loc ? `${loc.url}:${loc.lineNumber}` : '', phase);
      return;
    }
    const generic = issue.details.genericIssueDetails?.errorType;
    const text = generic ?? JSON.stringify(issue.details).slice(0, 300);
    log.add('audit', 'info', window, `${issue.code}: ${text}`, '', phase);
  });
  await cdp.send('Audits.enable');
  replay = false;
}

function windowOf(url: string): WindowName | null {
  let path: string;
  try {
    const parsed = new URL(url);
    if (!['http:', 'https:', 'tauri:'].includes(parsed.protocol)) return null;
    path = parsed.pathname;
  } catch {
    return null;
  }
  if (path === '/' || path === '/index.html') return 'main';
  if (path === '/quick.html') return 'quick';
  if (path === '/pill.html') return 'pill';
  return null;
}

async function connect(): Promise<Browser> {
  const deadline = Date.now() + 120_000;
  let last: unknown = null;
  while (Date.now() < deadline) {
    try {
      return await chromium.connectOverCDP(CDP_URL, { timeout: 15_000 });
    } catch (error) {
      last = error;
      await sleep(1_000);
    }
  }
  throw new Error(`Brak połączenia CDP z ${CDP_URL} (WebView2 Alfy): ${String(last)}`);
}

async function findWindows(browser: Browser): Promise<Partial<Record<WindowName, Page>>> {
  const deadline = Date.now() + 60_000;
  for (;;) {
    const found: Partial<Record<WindowName, Page>> = {};
    for (const context of browser.contexts()) {
      for (const page of context.pages()) {
        const name = windowOf(page.url());
        if (name && !found[name]) found[name] = page;
      }
    }
    if ((found.main && found.quick && found.pill) || Date.now() > deadline) return found;
    await sleep(500);
  }
}

export interface Alfa {
  main: Page;
  windows: Partial<Record<WindowName, Page>>;
  log: Findings;
}

/** `alfa` — połączenie na proces roboczy: okna przeładowane po podpięciu nasłuchu. */
export const test = base.extend<object, { alfa: Alfa }>({
  alfa: [
    // eslint-disable-next-line no-empty-pattern -- Playwright wymaga wzorca obiektu w fixturze.
    async ({}, use) => {
      const log = new Findings();
      log.phase = 'połączenie i przeładowanie okien';
      const browser = await connect();
      try {
        const windows = await findWindows(browser);
        const main = windows.main;
        if (!main) {
          const urls = browser.contexts().flatMap((c) => c.pages().map((p) => p.url()));
          throw new Error(`Brak okna głównego Alfy wśród stron CDP: ${urls.join(', ') || '—'}`);
        }
        const names = (['main', 'quick', 'pill'] as const).filter((n) => windows[n]);
        for (const name of names) {
          const page = windows[name];
          if (page) await watch(page, name, log);
        }
        // Przeładowanie: nasłuch od pierwszej linii dokumentu (także skrypty wstrzykiwane przez
        // Tauri/IPC). Okna ukryte (`quick`, `pill`) — tylko do zatwierdzenia nawigacji.
        for (const name of names) {
          const page = windows[name];
          if (page) {
            await page.reload({ waitUntil: name === 'main' ? 'load' : 'commit', timeout: 60_000 });
          }
        }
        await use({ main, windows, log });
      } finally {
        // Połączenie CDP: `close` tylko rozłącza — WebView2 i aplikacja działają dalej.
        await browser.close();
      }
    },
    { scope: 'worker', timeout: 240_000 },
  ],
});

export { expect };

/** Zrzut okna głównego jako załącznik testu i plik w `out/screens` (błąd zrzutu nie psuje testu). */
export async function shot(alfa: Alfa, name: string): Promise<void> {
  const path = join(OUT, 'screens', `${name}.png`);
  try {
    await alfa.main.screenshot({ path, timeout: 20_000 });
    await test.info().attach(name, { path, contentType: 'image/png' });
  } catch (error) {
    test
      .info()
      .annotations.push({ type: 'zrzut ekranu', description: `${name}: ${String(error)}` });
  }
}

/**
 * axe (WCAG 2.2 AA — 0 naruszeń critical/serious) jak `e2e/helpers.ts`, ale miękko (porażka nie
 * przerywa scenariusza) i w fazie `axe: …` dziennika. Kod narzędzi (axe, Playwright) wchodzi przez
 * CDP `Runtime.evaluate`, którego CSP strony nie blokuje; czekanie na koniec przejść CSS i dwie
 * klatki — w jednym `evaluate`, bez wyrażeń ocenianych później w stronie.
 */
export async function expectAccessible(alfa: Alfa, label: string): Promise<void> {
  const previous = alfa.log.phase;
  alfa.log.phase = `${AXE_PHASE}: ${label}`;
  try {
    await alfa.main.evaluate(async () => {
      const end = Date.now() + 2_000;
      while (Date.now() < end && document.getAnimations().some((a) => a.playState === 'running')) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      await new Promise((resolve) => {
        setTimeout(resolve, 500);
        requestAnimationFrame(() => requestAnimationFrame(resolve));
      });
    });
    const results = await new AxeBuilder({ page: alfa.main })
      .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
      .analyze();
    const blocking = results.violations
      .filter((v) => v.impact === 'critical' || v.impact === 'serious')
      .map(
        (v) =>
          `${v.id} (${v.impact}): ${v.nodes
            .map((n) => n.target.join(' '))
            .slice(0, 3)
            .join(' | ')}`,
      );
    // Miękko: porażka axe nie przerywa scenariusza (kolejne kroki też coś mówią o aplikacji).
    expect.soft(blocking, `axe: ${label}`).toEqual([]);
  } finally {
    alfa.log.phase = previous;
  }
}

/**
 * Natywne okno „Zapisz jako” (rdzeń → `tauri-plugin-dialog`) przez UI Automation Windows
 * (`save-dialog.ps1`): wpisuje `path` i zapisuje; `path = null` — anuluje otwarte okno.
 */
export function driveSaveDialog(
  path: string | null,
  timeoutSec = 45,
): Promise<{ code: number | null; output: string }> {
  const script = fileURLToPath(new URL('./save-dialog.ps1', import.meta.url));
  const args = ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', script];
  args.push(...(path === null ? ['-Cancel'] : ['-Path', path]), '-TimeoutSec', String(timeoutSec));
  return new Promise((resolve) => {
    const ps = spawn('powershell.exe', args, { windowsHide: true });
    let output = '';
    ps.stdout.on('data', (chunk: Buffer) => (output += chunk.toString('utf8')));
    ps.stderr.on('data', (chunk: Buffer) => (output += chunk.toString('utf8')));
    ps.on('error', (error) => resolve({ code: null, output: `${output}\n${String(error)}` }));
    ps.on('close', (code) => resolve({ code, output }));
  });
}
