import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../../api/fake/fake-client';
import { mcpSlug } from '../../api/fake/api-mcp';
import type { McpServerView } from '../../api/types-mcp';
import { en } from '../../i18n/en';
import { pl } from '../../i18n/pl';
import {
  MCP_PRESETS,
  emptyForm,
  formFromPreset,
  formFromServer,
  formProblems,
  parseArgs,
  runtimeOf,
  toInput,
  toolReady,
} from '../mcp';

const client = () => new FakeAlfaClient({ scheduler: new VirtualScheduler() });

describe('logika serwerów MCP', () => {
  it('przepisy mają opis w obu językach i poprawne polecenia', () => {
    for (const p of MCP_PRESETS) {
      expect(pl[`mcp.preset.${p.id}` as keyof typeof pl]).toBeTruthy();
      expect(en[`mcp.preset.${p.id}` as keyof typeof en]).toBeTruthy();
      expect(runtimeOf(p.command)).toBe(p.runtime);
      expect(p.args.includes('{folder}')).toBe(p.needsFolder);
    }
    expect(MCP_PRESETS.map((p) => p.id)).toContain('playwright');
  });

  it('formularz z przepisu: folder w argumentach, sekret wymagany', () => {
    const fs = MCP_PRESETS.find((p) => p.id === 'filesystem')!;
    const withFolder = formFromPreset(fs, 'C:\\Projekty');
    expect(parseArgs(withFolder.argsText)).toEqual([
      '-y',
      '@modelcontextprotocol/server-filesystem',
      'C:\\Projekty',
    ]);
    expect(formProblems(formFromPreset(fs, '{folder}'))).toContain('mcp.form.needFolder');
    const gh = formFromPreset(MCP_PRESETS.find((p) => p.id === 'github')!);
    expect(formProblems(gh)).toEqual(['mcp.form.needSecret']);
    gh.env[0] = { ...gh.env[0]!, value: 'tajne' };
    expect(formProblems(gh)).toEqual([]);
  });

  it('pusty formularz i dane komendy', () => {
    expect(formProblems(emptyForm())).toEqual(['mcp.form.needName', 'mcp.form.needCommand']);
    const f = {
      ...emptyForm(),
      name: ' A ',
      command: ' npx ',
      argsText: ' -y \n\n pakiet z spacją \n',
    };
    f.env = [
      { name: 'LOG', value: 'x', secret: false },
      { name: ' ', value: 'pomijane', secret: false },
    ];
    f.timeoutSec = 0;
    const input = toInput(f);
    expect(input).toMatchObject({ name: 'A', command: 'npx', cwd: null, timeout_ms: 1000 });
    expect(input.args).toEqual(['-y', 'pakiet z spacją']);
    expect(input.env).toEqual([{ name: 'LOG', value: 'x', secret: false }]);
  });

  it('edycja: zapisany sekret (null) nie blokuje zapisu, nowy pusty — blokuje', () => {
    const view = {
      id: 'gh',
      name: 'GitHub',
      command: 'npx',
      args: [],
      cwd: null,
      trust: 'untrusted',
      enabled: true,
      timeout_ms: 30000,
      env: [{ name: 'GITHUB_TOKEN', value: null, secret: true }],
    } as unknown as McpServerView;
    const f = formFromServer(view);
    expect(formProblems(f)).toEqual([]);
    f.env.push({ name: 'API_KEY', value: '', secret: true });
    expect(formProblems(f)).toEqual(['mcp.form.needSecret']);
  });

  it('identyfikator z nazwy jak w rdzeniu', () => {
    expect(mcpSlug('Pliki na dysku Ż')).toBe('pliki-na-dysku-z');
    expect(mcpSlug('  GitHub (praca) ')).toBe('github-praca');
    expect(mcpSlug('!!!')).toBe('serwer');
  });
});

describe('atrapa serwerów MCP', () => {
  it('lista, narzędzia, zgoda z odciskiem, cofnięcie', async () => {
    const c = client();
    const list = await c.mcp.list();
    expect(list.map((s) => s.id)).toEqual(['pliki', 'github']);
    const t = await c.mcp.tools('pliki');
    expect(t.server.state).toBe('connected');
    const write = t.tools.find((x) => x.name === 'write_file')!;
    expect(toolReady(write)).toBe(false);
    await expect(c.mcp.approve('pliki', 'write_file', 'zly')).rejects.toThrow(/Odcisk/);
    const after = await c.mcp.approve('pliki', 'write_file', write.fingerprint);
    expect(after.server.tools_approved).toBe(3);
    expect(t.tools.find((x) => x.name === 'move_file')!.suspicious).toBe(true);
    const back = await c.mcp.revoke('pliki', 'write_file');
    expect(back.server.tools_approved).toBe(2);
  });

  it('wyłączony serwer bez narzędzi; brak programu — błąd z podpowiedzią', async () => {
    const c = client();
    const off = await c.mcp.tools('github');
    expect(off.server.state).toBe('disabled');
    expect(off.tools).toEqual([]);
    const saved = await c.mcp.save({
      ...toInput({ ...emptyForm(), name: 'Zły', command: 'brak-programu' }),
    });
    expect(saved.resolved).toBeNull();
    const failed = await c.mcp.tools(saved.id);
    expect(failed.server.state).toBe('failed');
    expect(failed.server.error).toMatch(/Nie znaleziono programu/);
  });

  it('zapis: tajne nazwy zawsze tajne, wartości nie wracają, zmiana programu kasuje zgody', async () => {
    const c = client();
    const v = await c.mcp.save(
      toInput({
        ...emptyForm(),
        name: 'Pliki',
        command: 'npx',
        env: [{ name: 'MY_TOKEN', value: 'abc', secret: false }],
      }),
    );
    expect(v.id).toBe('pliki-2');
    expect(v.env).toEqual([{ name: 'MY_TOKEN', value: null, secret: true }]);
    await expect(
      c.mcp.save(
        toInput({
          ...emptyForm(),
          name: 'X',
          command: 'npx',
          env: [{ name: 'API_KEY', value: '', secret: true }],
        }),
      ),
    ).rejects.toThrow(/tajnej/);
    const files = formFromServer((await c.mcp.list())[0]!);
    files.argsText = '-y\ninny-pakiet';
    await c.mcp.save(toInput(files));
    const tools = await c.mcp.tools('pliki');
    expect(tools.tools.every((t) => t.state === 'new')).toBe(true);
    await c.mcp.remove('pliki');
    expect((await c.mcp.list()).map((s) => s.id)).not.toContain('pliki');
  });
});
