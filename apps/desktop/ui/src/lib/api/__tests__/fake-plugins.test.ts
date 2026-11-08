import { describe, expect, it } from 'vitest';
import { fakeSha } from '../fake/api-plugins';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';

const COMPONENT = btoa(String.fromCharCode(0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00));
const CORE = btoa(String.fromCharCode(0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00));

function client() {
  return new FakeAlfaClient({ scheduler: new VirtualScheduler() });
}

const manifest = (caps: unknown[] = []) => ({
  id: 'notatnik',
  version: '1.0.0',
  description: 'Dopisuje notatki.',
  capabilities: caps,
  tools: [{ name: 'append_note', title: 'Dopisz', description: 'Dopisuje notatkę do pliku.' }],
});

describe('atrapa: wtyczki', () => {
  it('biblioteka startowa: propozycja Ulepszacza z R2, zainstalowana, problem', async () => {
    const c = client();
    const view = await c.plugins.list();
    expect(view.available).toBe(true);
    const fx = view.plugins.find((p) => p.id === 'kurs-walut');
    expect(fx?.state).toBe('proposed');
    expect(fx?.r2?.value).toBe(fx?.review_hash);
    expect(view.plugins.find((p) => p.id === 'licznik-slow')?.state).toBe('installed');
    expect(view.problems[0]?.kind).toBe('trapped');
  });

  it('kontrola modułu: komponent tak, moduł rdzeniowy nie, śmieci — błąd', async () => {
    const c = client();
    expect((await c.plugins.inspect(COMPONENT)).ok).toBe(true);
    const core = await c.plugins.inspect(CORE);
    expect(core.ok).toBe(false);
    expect(core.error).toMatch(/komponent/);
    await expect(c.plugins.inspect('%%%')).rejects.toThrow(/base64/);
  });

  it('propozycja → zatwierdzenie tylko z hashem → wyłącz → włącz z hashem → usuń', async () => {
    const c = client();
    const p = await c.plugins.propose(manifest(), COMPONENT);
    expect(p.state).toBe('proposed');
    expect(p.tools[0]?.name).toBe('plugin_append_note');
    await expect(c.plugins.approve(p.id, p.version, fakeSha('inny'))).rejects.toThrow(/hash/);
    expect((await c.plugins.approve(p.id, p.version, p.review_hash)).state).toBe('installed');
    expect((await c.plugins.disable(p.id)).state).toBe('disabled');
    await expect(c.plugins.enable(p.id, 'x')).rejects.toThrow(/hash/);
    expect((await c.plugins.enable(p.id, p.review_hash)).state).toBe('installed');
    const left = await c.plugins.remove(p.id);
    expect(left.plugins.some((x) => x.id === p.id)).toBe(false);
  });

  it('zakazane zdolności i zła wersja są odrzucane', async () => {
    const c = client();
    await expect(
      c.plugins.propose(manifest([{ cap: 'shell.exec', scope: 'C:\\x' }]), COMPONENT),
    ).rejects.toThrow(/shell\.exec/);
    await expect(c.plugins.propose({ id: 'x' }, COMPONENT)).rejects.toThrow(/manifest/);
    const fx = (await c.plugins.list()).plugins.find((p) => p.state === 'proposed');
    if (!fx) throw new Error('brak propozycji');
    expect((await c.plugins.reject(fx.id, fx.version)).state).toBe('rejected');
    expect(fakeSha('a')).toHaveLength(64);
  });
});
