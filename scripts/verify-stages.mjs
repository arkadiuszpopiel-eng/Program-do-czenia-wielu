// Pomocnicze bramki `verify.mjs`: przeglądarki testowe (liczba, kanał) i liczby do raportu z wyjścia etapów.
import { existsSync, readFileSync } from 'node:fs';
import { freemem } from 'node:os';
import { join } from 'node:path';

/**
 * Ile przeglądarek testowych naraz: każda zajmuje ok. 1,5 GB. Przy małej wolnej pamięci (inne programy
 * otwarte obok) cztery naraz dławią maszynę i testy padają na przekroczeniu czasu w losowych
 * miejscach (2026-10-14: 2,6 GB wolnej pamięci → 11–14 losowych czerwonych). `PW_WORKERS` nadpisuje.
 */
export function e2eWorkers() {
  if (process.env.PW_WORKERS) return process.env.PW_WORKERS;
  const freeGb = freemem() / 1024 ** 3;
  return String(Math.max(1, Math.min(4, Math.floor(freeGb / 1.5))));
}

// Testy w przeglądarce celują w Edge (silnik WebView2). Komputer bez Edge (stacjonarny, 2026-10-21)
// dostaje Chromium Playwrighta zamiast zawsze czerwonego etapu — z adnotacją w raporcie.
// `PW_CHANNEL` z otoczenia ma pierwszeństwo (pusty = Chromium).
export function e2eChannel() {
  if (process.env.PW_CHANNEL !== undefined) return process.env.PW_CHANNEL;
  if (process.platform !== 'win32') return 'msedge';
  const edge = ['C:\\Program Files (x86)', 'C:\\Program Files'].map((dir) =>
    join(dir, 'Microsoft', 'Edge', 'Application', 'msedge.exe'),
  );
  return edge.some((path) => existsSync(path)) ? 'msedge' : '';
}

/** Liczby do raportu wyjęte z wyjścia etapu (nie z pamięci). */
export function numbersFrom(id, output, root) {
  if (id === 'testy') {
    const rows = [
      ...output.matchAll(/test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored/g),
    ];
    const sum = (i) => rows.reduce((total, m) => total + Number(m[i]), 0);
    const tests = `${rows.length} programów testowych, ${sum(1)} zielonych, ${sum(2)} czerwonych, ${sum(3)} pominiętych`;
    // Pierwsze zdanie raportu diagnostyki narzędzi agentek (docs/plan/45 §6.5).
    const diag = join(root, 'target/verify/narzedzia/raport.md');
    const tools = existsSync(diag) ? readFileSync(diag, 'utf8').split('\n')[2] : '';
    return [tests, tools && `narzędzia agentek: ${tools}`].filter(Boolean).join('; ');
  }
  if (id === 'interfejs') return output.match(/Tests\s+(.+)/)?.[1]?.trim() ?? '';
  if (id === 'przegladarka') {
    const passed = output.match(/^\s*(\d+ passed.*)$/m)?.[1]?.trim() ?? '';
    return e2eChannel() ? passed : `${passed} (Chromium Playwrighta — brak Edge)`;
  }
  if (id === 'aplikacja') {
    const passed = output.match(/^\s*(\d+ passed.*)$/m)?.[1]?.trim() ?? '';
    const coverage = output.match(/przećwiczone w prawdziwej aplikacji: (.+)/)?.[1] ?? '';
    return [passed, coverage && `pokrycie łącza ${coverage}`].filter(Boolean).join('; ');
  }
  if (id === 'sprzet' || id === 'obowiazki') {
    return output
      .split(/\r?\n/)
      .filter((line) => line.startsWith('— '))
      .map((line) => line.slice(2).trim())
      .join(' · ');
  }
  return '';
}
