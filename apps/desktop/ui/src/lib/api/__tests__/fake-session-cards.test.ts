import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';

function client() {
  return new FakeAlfaClient({ scheduler: new VirtualScheduler() });
}

describe('atrapa: karty rozmów (sessions.cards)', () => {
  it('karta „Raport Q3”: agentki, które odpowiadały, ostatnia wiadomość, model i koszt', async () => {
    const [card] = await client().sessions.cards(['s-q3']);
    expect(card?.session_id).toBe('s-q3');
    expect(card?.agents).toEqual(['gama', 'delta']);
    expect(card?.snippet_author).toBe('delta');
    expect(card?.snippet).toMatch(/^Utworzyłam szkic raport-Q3\.docx/);
    expect(card?.model).toBe('claude-opus-5-5');
    expect(card?.cost).toEqual({ minor: 108, currency: 'PLN' });
  });

  it('czysty tekst bez Markdownu; nieznane i powtórzone rozmowy pominięte', async () => {
    const cards = await client().sessions.cards(['s-q3', 'nie-ma', 's-q3', 's-shop']);
    expect(cards.map((c) => c.session_id)).toEqual(['s-q3', 's-shop']);
    for (const card of cards) {
      expect(card.snippet ?? '').not.toMatch(/[`*#<>]/);
      expect(card.snippet ?? '').not.toContain('\n');
    }
  });

  it('nowa rozmowa bez wiadomości: obsada domyślna, bez fragmentu i kosztu', async () => {
    const fake = client();
    const created = await fake.sessions.create('empty');
    const [card] = await fake.sessions.cards([created.id]);
    expect(card?.snippet).toBeNull();
    expect(card?.cost).toBeNull();
    expect(card?.model).toBeNull();
    expect(card?.agents).toEqual(['alfa']);
  });
});

describe('atrapa: projekty i katalogi projektów (sessions.projects*)', () => {
  it('dodaje projekt ze wskazaniem katalogu, ustawia katalog rozmowy, zmienia nazwę i usuwa', async () => {
    const fake = client();
    const initial = await fake.sessions.projectsList();
    expect(initial.map((p) => p.name)).toEqual(['Projekt X', 'Sklep']);

    const picked = await fake.sessions.projectsPickDir();
    expect(picked).toBe('C:\\Users\\Ty\\Dokumenty\\Projekt');

    const afterSave = await fake.sessions.projectsSave('', picked);
    const added = afterSave.find((p) => p.name === 'Projekt');
    expect(added).toEqual({
      name: 'Projekt',
      workdir: 'C:\\Users\\Ty\\Dokumenty\\Projekt',
      session_count: 0,
    });

    await fake.sessions.setProject('s-trip', 'Projekt');
    const workdir = await fake.sessions.workdir('s-trip');
    expect(workdir.path).toBe('C:\\Users\\Ty\\Dokumenty\\Projekt');

    const afterRename = await fake.sessions.projectsRename('Projekt', 'Projekt Alfa');
    expect(afterRename.find((p) => p.name === 'Projekt Alfa')?.session_count).toBe(1);
    const sessions = await fake.sessions.list();
    expect(sessions.find((s) => s.id === 's-trip')?.project?.name).toBe('Projekt Alfa');

    const afterRemove = await fake.sessions.projectsRemove('Projekt Alfa');
    expect(afterRemove.find((p) => p.name === 'Projekt Alfa')).toBeUndefined();
    const afterSessions = await fake.sessions.list();
    expect(afterSessions.find((s) => s.id === 's-trip')?.project).toBeNull();
  });
});
