// Seria 1 (0.0.60) na PRAWDZIWYM rdzeniu: moduły, edytor konfiguracji, polecenia bez modelu
// (tester, własne polecenia), szablony promptów i trasy zgodności. Wszystko na profilu instancji
// testowej; zapisy konfiguracji są cofane na końcu testu.
import { expect, test } from './alfa';
import { call, refused } from './ipc-call';

interface ModulesView {
  modules: { id: string; protected: boolean; can_disable: boolean }[];
}

interface RawConfig {
  shared: string;
  machine: string;
  shared_path: string;
}

interface QuickCommandsView {
  commands: { id: string; name: string }[];
  builtins: { title: string }[];
  enabled: boolean;
}

interface QuickTest {
  source: string;
  name: string | null;
  reply: string | null;
}

test('moduły i edytor konfiguracji: lista, odmowa Jądra, sprawdzenie składni', async ({ alfa }) => {
  const { main } = alfa;
  const modules = await call<ModulesView>(main, 'modules_list');
  expect(modules.modules.find((m) => m.id === 'safety-broker')?.protected).toBe(true);
  await refused(main, 'modules_set_enabled', { id: 'safety-broker', enabled: false });
  const raw = await call<RawConfig>(main, 'config_raw_get');
  expect(raw.shared_path).toMatch(/shared\.toml$/);
  expect(await call<number>(main, 'config_raw_check', { text: '[ui]\ntheme = "dark"\n' })).toBe(1);
  await refused(main, 'config_raw_check', { text: '[ui\n' });
  await refused(main, 'config_raw_save', {
    layer: 'shared',
    text: '[kernel]\nautonomy_max = "L4"\n',
  });
  expect((await call<RawConfig>(main, 'config_raw_get')).shared).toBe(raw.shared);
});

test('polecenia bez modelu: tester, własne polecenie, szablony, trasy danych', async ({ alfa }) => {
  const { main } = alfa;
  const view = await call<QuickCommandsView>(main, 'quick_commands_get');
  expect(view.builtins.length).toBeGreaterThanOrEqual(5);
  const math = await call<QuickTest>(main, 'quick_commands_test', {
    message: 'Ile to jest 7 razy 8?',
  });
  expect(math.source).toBe('builtin');
  expect(math.reply).toBe('7 × 8 = **56**');
  const saved = await call<QuickCommandsView>(main, 'quick_commands_set', {
    commands: [
      ...view.commands,
      {
        id: '',
        name: 'Zanotuj e2e',
        phrases: ['zanotuj'],
        mode: 'starts',
        reply: 'Zapisuję: {reszta}',
        enabled: true,
        priority: 0,
      },
    ],
  });
  expect(saved.commands.some((c) => c.name === 'Zanotuj e2e')).toBe(true);
  const own = await call<QuickTest>(main, 'quick_commands_test', { message: 'Zanotuj: mleko' });
  expect(own.reply).toBe('Zapisuję: mleko');
  await call(main, 'quick_commands_set', { commands: view.commands });

  const templates = await call<{ id: string }[]>(main, 'prompt_templates_get');
  await refused(main, 'prompt_templates_set', {
    templates: [{ id: '', name: 'Zepsuty', text: 'Do {{adresat' }],
  });
  expect(await call<{ id: string }[]>(main, 'prompt_templates_get')).toEqual(templates);

  const routes = await call<{ routes: { private_allowed: boolean; jurisdiction: string[] }[] }>(
    main,
    'compliance_routes',
  );
  expect(routes.routes.length).toBeGreaterThan(0);
  for (const r of routes.routes.filter((x) => x.jurisdiction.includes('CN'))) {
    expect(r.private_allowed).toBe(false);
  }
});
