// Serwery MCP i diagnostyka komputera na PRAWDZIWYM rdzeniu. Bez sieci i bez uruchamiania
// serwerów MCP: serwer wskazuje program, którego nie ma (start kończy się czytelnym błędem).
// Diagnostyka: prawdziwy zapis parametrów (PowerShell/CIM, tylko odczyt, bez okna) i lekkie
// testy bez sieci i bez dysku (procesor, pamięć, spójność); raport w folderze instancji testowej.
import { expect, test } from './alfa';
import { call, refused } from './ipc-call';

interface McpServerView {
  id: string;
  state: string;
  resolved: string | null;
  error: string | null;
  env: { name: string; value: string | null; secret: boolean }[];
}

test('serwery MCP: zapis bez sekretu w odpowiedzi, nieudany start, zgody, usunięcie', async ({
  alfa,
}) => {
  const { main } = alfa;
  const before = await call<McpServerView[]>(main, 'mcp_list');
  const saved = await call<McpServerView>(main, 'mcp_save', {
    input: {
      id: null,
      name: 'Serwer próbny e2e',
      command: 'alfa-e2e-nie-ma-takiego-programu',
      args: ['--stdio'],
      cwd: null,
      env: [{ name: 'E2E_TOKEN', value: 'alfa-e2e-wartosc-tajna', secret: true }],
      trust: 'untrusted',
      enabled: true,
      timeout_ms: 5000,
    },
  });
  expect(saved.resolved).toBeNull();
  expect(saved.env).toEqual([{ name: 'E2E_TOKEN', value: null, secret: true }]);
  expect(JSON.stringify(saved)).not.toContain('alfa-e2e-wartosc-tajna');
  const tools = await call<{ server: McpServerView; tools: unknown[] }>(main, 'mcp_tools', {
    id: saved.id,
  });
  expect(tools.server.state).toBe('failed');
  expect(tools.server.error).toContain('Nie znaleziono programu');
  await refused(main, 'mcp_approve', { id: saved.id, tool: 'x', fingerprint: 'zly' });
  await call(main, 'mcp_revoke', { id: saved.id, tool: 'x' });
  await call(main, 'mcp_remove', { id: saved.id });
  expect(await call<McpServerView[]>(main, 'mcp_list')).toHaveLength(before.length);
  await refused(main, 'mcp_remove', { id: saved.id });
});

interface DiagView {
  latest: { cpu: { threads: number }; memory: { total_mb: number } } | null;
  integrity: { verdict: string } | null;
  available: string[];
  running: unknown;
}

test('diagnostyka: prawdziwy zapis parametrów, lekkie testy, historia, raport', async ({
  alfa,
}) => {
  // Dwa pełne odczyty sondy (zapis i test spójności), każdy z limitem 90 s; 1.11 zawieszony
  // czujnik systemu (WMI) wydłużał odczyt, a test kończył się po 120 s, choć program działał.
  test.setTimeout(240_000);
  const { main } = alfa;
  await call<DiagView>(main, 'diag_state');
  const view = await call<DiagView>(main, 'diag_snapshot');
  expect(view.latest?.cpu.threads).toBeGreaterThan(0);
  expect(view.latest?.memory.total_mb).toBeGreaterThan(0);
  expect(view.integrity).not.toBeNull();
  expect(view.available).toContain('cpu_single');
  await refused(main, 'diag_run', { tests: ['format_c'] });
  const run = await call<{ results: { verdict: string }[] }>(main, 'diag_run', {
    tests: ['integrity', 'cpu_single', 'memory'],
  });
  expect(run.results).toHaveLength(3);
  expect(run.results.map((r) => r.verdict)).not.toContain('skipped');
  expect(await call<unknown[]>(main, 'diag_runs', { limit: 5 })).toHaveLength(1);
  await call(main, 'diag_cancel');
  const path = await call<string>(main, 'diag_export');
  expect(path).toMatch(/Diagnostyka[\\/]diagnostyka-.*\.md$/);
});
