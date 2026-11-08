// Start przebiegu: czysty dziennik ustaleń i zrzuty (katalog `out` zostaje — run.ps1 dokłada logi).
// Bez importu `./alfa` (fixture'y Playwrighta nie należą do procesu głównego) — ten sam katalog.
import { rmSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const OUT = process.env['ALFA_E2E_OUT'] ?? fileURLToPath(new URL('./out', import.meta.url));

export default function globalSetup(): void {
  rmSync(join(OUT, 'console.jsonl'), { force: true });
  rmSync(join(OUT, 'screens'), { recursive: true, force: true });
}
