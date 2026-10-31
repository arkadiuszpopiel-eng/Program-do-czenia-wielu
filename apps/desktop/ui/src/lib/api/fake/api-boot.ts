// Atrapa: dziennik rozruchu (`boot_report`). Scenariusze `boot-ok` / `boot-warnings` / `boot-errors` —
// stany do sprawdzenia w każdym stylu i motywie; domyślnie rozruch „trwa” (bez toastu w e2e).
import type { BootLine, BootReport } from '../types-boot';
import type { FakeCore } from './core';

const line = (level: BootLine['level'], text: string): BootLine => ({
  level,
  text: `2026-10-22T09:16:3${text.length % 10}.000Z  ${level.padEnd(5)} ${text}`,
});

const BASE: readonly BootLine[] = [
  line('INFO', 'alfa_desktop_lib: start powłoki Alfy wersja=0.0.72'),
  line('INFO', 'app_core::compose: kolejność startu modułów'),
  line(
    'INFO',
    'app_broker::notice: alfa-watchdog gotowy — skrót Ctrl+Shift+F12 obsługuje watchdog gotowy_po_ms=2210',
  ),
  line('INFO', 'app_modules::tts: silniki TTS wykryte pocket=false piper=true'),
];

const WARNINGS: readonly BootLine[] = [
  line(
    'WARN',
    'app_broker::supervise: tryb przenośny: uruchomiono alfa-broker --console (słabsza izolacja) pid=31328',
  ),
  line(
    'WARN',
    'alfa_desktop_lib::shortcuts: rejestracja skrótu globalnego nie powiodła się skrot=Ctrl+Alt+Space',
  ),
];

const ERRORS: readonly BootLine[] = [
  line(
    'ERROR',
    'alfa_desktop_lib::boot: poprzedni start Alfy się nie powiódł (program zamknął się przed oknem) powod="Sędzia Cieni nie odpowiedział"',
  ),
];

export function bootReport(core: FakeCore): BootReport {
  const warnings = core.scenario === 'boot-warnings' || core.scenario === 'boot-errors';
  const errors = core.scenario === 'boot-errors';
  const lines = [...BASE, ...(warnings ? WARNINGS : []), ...(errors ? ERRORS : [])];
  return {
    started_at: core.isoNow(),
    finished: core.scenario.startsWith('boot-'),
    elapsed_ms: 2840,
    verdict: errors ? 'errors' : warnings ? 'warnings' : 'ok',
    warnings: warnings ? WARNINGS.length : 0,
    errors: errors ? ERRORS.length : 0,
    lines,
    dropped: 0,
    log_file: String.raw`C:\Users\ala\AppData\Local\Alfa\logs\alfa.2026-10-22.000.log`,
  };
}
