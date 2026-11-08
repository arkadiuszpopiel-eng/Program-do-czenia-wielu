// Polecenia z systemowym oknem wyboru pliku/katalogu na PRAWDZIWYM rdzeniu (1.11): okno
// systemowe wymaga pierwszego planu, a testy nie wchodzą w sesję właściciela — budowa testowa
// bierze odpowiedź okna z kolejki (`e2e_answer_dialog`, cecha `e2e`). Pliki i katalogi tylko
// w katalogu wyników testu; propozycja umiejętności odrzucona, katalog kopii (wyłączonych) zostaje
// w katalogu wyników — zmienić go można tylko oknem wyboru. Bez dźwięku, sieci i okien systemowych.
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, test, OUT } from './alfa';
import { call, raw, refused } from './ipc-call';

const DIR = join(OUT, 'okna-wyboru');

/** Odpowiedź następnego okna wyboru (`null` — właściciel anulował). */
async function answer(page: Page, path: string | null) {
  const reply = await raw(page, 'e2e_answer_dialog', { path });
  expect(reply.ok, `e2e_answer_dialog: ${JSON.stringify(reply)}`).toBe(true);
}

type Backup = { config: { dir: string | null } };
type Exported = { status: 'saved'; path: string; files: number } | { status: 'cancelled' };
type Skill = { id: string; name: string; version: string; state: string; trusted: boolean };
type Imported = { proposed: Skill[]; skipped: unknown[] };
type Teach = { name: string; text: string; truncated: boolean };

test.beforeAll(() => {
  rmSync(DIR, { recursive: true, force: true });
  mkdirSync(DIR, { recursive: true });
});

test.beforeEach(({ alfa }, info) => {
  alfa.log.phase = info.title;
});

test('katalog projektu: wybrany katalog wraca, anulowanie daje pusty wynik', async ({ alfa }) => {
  const { main } = alfa;
  const project = join(DIR, 'Projekt z okna');
  mkdirSync(project, { recursive: true });
  await answer(main, project);
  const picked = await call<string | null>(main, 'projects_pick_dir');
  expect(picked?.toLowerCase()).toContain('projekt z okna');
  await answer(main, null);
  expect(await call<string | null>(main, 'projects_pick_dir')).toBeNull();
});

test('katalog kopii zapasowych: wybór zapisany, katalog danych Alfy odrzucony', async ({
  alfa,
}) => {
  const { main } = alfa;
  const before = await call<Backup>(main, 'backups_status');
  const target = join(DIR, 'Kopie');
  mkdirSync(target, { recursive: true });
  await answer(main, target);
  const chosen = await call<Backup>(main, 'backups_choose_dir');
  expect(chosen.config.dir?.toLowerCase()).toContain('kopie');
  // Anulowanie nie zmienia wyboru.
  await answer(main, null);
  expect((await call<Backup>(main, 'backups_choose_dir')).config.dir).toBe(chosen.config.dir);
  // Dane samej Alfy (profil instancji testowej) — odmowa przed zapisem.
  const data = (process.env['LOCALAPPDATA'] ?? '') + '\\Alfa';
  await answer(main, data);
  await refused(main, 'backups_choose_dir', {});
  // Katalog tylko z okna wyboru: zapis ustawień z innym katalogiem go nie zmienia.
  const configured = await call<Backup>(main, 'backups_configure', {
    config: { ...chosen.config, dir: before.config.dir ?? join(DIR, 'Inny') },
  });
  expect(configured.config.dir).toBe(chosen.config.dir);
});

test('„Naucz Alfę”: plik z okna czytany do podglądu, anulowanie bez pliku', async ({ alfa }) => {
  const { main } = alfa;
  const file = join(DIR, 'notatka-do-nauki.md');
  writeFileSync(file, '# Wyjazd\n\nPociąg z Krakowa o 7:12, peron 3.\n', 'utf8');
  await answer(main, file);
  const taught = await call<Teach | null>(main, 'memory_teach_pick_file');
  expect(taught?.name).toBe('notatka-do-nauki.md');
  expect(taught?.text).toContain('Pociąg z Krakowa o 7:12');
  await answer(main, null);
  expect(await call<Teach | null>(main, 'memory_teach_pick_file')).toBeNull();
});

test('umiejętności: eksport do pliku z okna, import SKILL.md do kwarantanny', async ({ alfa }) => {
  const { main } = alfa;
  const bundle = join(DIR, 'umiejetnosci.alfa');
  await answer(main, bundle);
  const exported = await call<Exported>(main, 'skills_export');
  expect(exported.status).toBe('saved');
  await answer(main, null);
  expect((await call<Exported>(main, 'skills_export')).status).toBe('cancelled');
  // Paczka z eksportu wraca bez błędu (zainstalowane już są — nic nowego do kwarantanny).
  await answer(main, bundle);
  await call<Imported>(main, 'skills_import');

  const skillMd = join(DIR, 'SKILL.md');
  writeFileSync(
    skillMd,
    '---\nname: Raport e2e z okna\ndescription: Zbiera notatki w krótki raport (test).\n---\n\n# Kroki\n\nPrzeczytaj notatki.\n',
    'utf8',
  );
  await answer(main, skillMd);
  const imported = await call<Imported>(main, 'skills_import');
  expect(imported.proposed).toHaveLength(1);
  const skill = imported.proposed[0];
  expect(skill?.name).toBe('Raport e2e z okna');
  expect(skill?.state).toBe('quarantined');
  expect(skill?.trusted).toBe(false);
  // Z zewnątrz — nigdy instalacja; propozycję odrzucamy, profil wraca do stanu wyjściowego.
  if (skill) await call(main, 'skills_reject', { skillId: skill.id, version: skill.version });
  await answer(main, null);
  const cancelled = await call<Imported>(main, 'skills_import');
  expect(cancelled.proposed).toEqual([]);
});
