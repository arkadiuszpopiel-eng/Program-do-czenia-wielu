// Błędy komend rdzenia w stanie okna: każda akcja pokazuje komunikat (toast albo widok błędu),
// cofa zmiany optymistyczne i nie zostawia odrzucenia bez obsługi (vitest zgłasza takie jako błąd).
// Regresja testu na laptopie (2026-10-07): odrzucona komenda = „przycisk nic nie robi".
import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { AppState } from '../app.svelte';
import { AttachmentsState } from '../attachments.svelte';
import { escapeChain, runCommand } from '../commands';
import { flush, setupApp } from './helpers';

beforeAll(() => {
  globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number) as typeof requestAnimationFrame;
});

/** Odrzucenie jak z `invoke` w Tauri: surowy `AppError` z Rust, nie `Error`. */
const coreError = (message: string) => ({ code: 'invalid_input', message });

const lastToast = (app: AppState) => app.toasts.items.at(-1);
const title = (app: AppState, id: string) => app.sessions.list.find((s) => s.id === id)?.title;

describe('AppState — start i otwieranie sesji', () => {
  it('start: niekrytyczne (koszty, agentki, szkic, stan systemu) → toast, okno działa', async () => {
    const { app, client } = setupApp();
    vi.spyOn(client.costs, 'summary').mockRejectedValue(coreError('Koszty niedostępne'));
    vi.spyOn(client.agents, 'list').mockRejectedValue(coreError('Agentki niedostępne'));
    vi.spyOn(client.sessions, 'getDraft').mockRejectedValue(coreError('Szkic niedostępny'));
    vi.spyOn(client.system, 'status').mockRejectedValue(coreError('Stan niedostępny'));
    vi.spyOn(client.voice, 'status').mockRejectedValue(coreError('Głos niedostępny'));
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    await app.start();
    expect(app.view).toBe('chat');
    expect(app.fatal).toBeNull();
    expect(app.activeId).toBe('s-q3');
    expect(app.conversation?.path.length).toBe(6);
    const messages = app.toasts.items.map((t) => t.message);
    expect(messages).toContain('Nie udało się wczytać: Koszty niedostępne');
    expect(messages).toContain('Nie udało się wczytać: Stan niedostępny');
    expect(app.toasts.items.every((t) => t.kind === 'error')).toBe(true);
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('Głos niedostępny'));
    warn.mockRestore();
  });

  it('start: brak listy sesji → widok błędu z komunikatem; „Ponów" startuje bez podwójnej subskrypcji', async () => {
    const { app, client } = setupApp();
    let subscribed = 0;
    const subscribe = client.subscribe.bind(client);
    vi.spyOn(client, 'subscribe').mockImplementation((listener) => {
      subscribed++;
      const off = subscribe(listener);
      return () => {
        subscribed--;
        off();
      };
    });
    vi.spyOn(client.sessions, 'list').mockRejectedValueOnce(coreError('Baza zablokowana'));
    await app.start();
    expect(app.view).toBe('error');
    expect(app.fatal).toBe('Baza zablokowana');
    await app.start();
    expect(app.view).toBe('chat');
    expect(app.fatal).toBeNull();
    expect(subscribed).toBe(1);
  });

  it('openSession: nieudane wczytanie rozmowy → loadError; ponowne otwarcie wczytuje znowu', async () => {
    const { app, client } = setupApp();
    await app.start();
    vi.spyOn(client.turns, 'list').mockRejectedValueOnce(coreError('Plik sesji uszkodzony'));
    await app.openSession('s-api');
    expect(app.activeId).toBe('s-api');
    expect(app.conversation?.loadError).toBe('Plik sesji uszkodzony');
    await app.openSession('s-trip');
    await app.openSession('s-api');
    expect(app.conversation?.loadError).toBeNull();
  });

  it('focusSession: błąd listy sesji → toast, bez odrzucenia', async () => {
    const { app, client } = setupApp();
    await app.start();
    vi.spyOn(client.sessions, 'list').mockRejectedValue(coreError('Rdzeń nie odpowiada'));
    await app.focusSession('s-nowa');
    expect(lastToast(app)?.message).toBe('Nie udało się wczytać: Rdzeń nie odpowiada');
    expect(app.activeId).toBe('s-q3');
  });

  it('openSession: błąd setActiveSession / markRead trafia do dziennika, nie do odrzucenia', async () => {
    const { app, client } = setupApp();
    await app.start();
    vi.spyOn(client.app, 'setActiveSession').mockRejectedValue(coreError('Zapis stanu'));
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    await app.openSession('s-trip');
    await flush();
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('Zapis stanu'));
    warn.mockRestore();
  });
});

describe('AppState — akcje na sesjach', () => {
  it('zmiana nazwy: nowa od razu, po odrzuceniu wraca poprzednia + toast', async () => {
    const { app, client } = setupApp();
    await app.start();
    const before = title(app, 's-trip');
    vi.spyOn(client.sessions, 'rename').mockRejectedValue(coreError('Nazwa za długa'));
    const pending = app.renameSession('s-trip', 'Nowa nazwa');
    expect(title(app, 's-trip')).toBe('Nowa nazwa');
    expect(await pending).toBe(false);
    expect(title(app, 's-trip')).toBe(before);
    expect(lastToast(app)).toMatchObject({ kind: 'error', message: 'Nazwa za długa' });
  });

  it('nowa rozmowa, przypięcie, archiwum, eksport: błąd → toast i wynik false', async () => {
    const { app, client } = setupApp();
    await app.start();
    vi.spyOn(client.sessions, 'create').mockRejectedValue(coreError('Brak miejsca na dysku'));
    vi.spyOn(client.sessions, 'setPinned').mockRejectedValue(coreError('Nie przypięto'));
    vi.spyOn(client.sessions, 'setArchived').mockRejectedValue(coreError('Nie zarchiwizowano'));
    vi.spyOn(client.sessions, 'exportSession').mockRejectedValue(coreError('Brak dostępu'));
    expect(await app.newSession()).toBe(false);
    expect(app.activeId).toBe('s-q3');
    expect(lastToast(app)?.message).toBe('Brak miejsca na dysku');
    expect(await app.setPinned('s-trip', true)).toBe(false);
    expect(lastToast(app)?.message).toBe('Nie przypięto');
    expect(await app.setArchived('s-trip', true)).toBe(false);
    expect(lastToast(app)?.message).toBe('Nie zarchiwizowano');
    expect(await app.exportSession('s-trip')).toBe(false);
    expect(lastToast(app)?.message).toBe('Brak dostępu');
  });

  it('usunięcie: błąd → sesja zostaje; błąd „Cofnij" w toaście → toast z powodem', async () => {
    const { app, client } = setupApp();
    await app.start();
    const remove = vi.spyOn(client.sessions, 'remove');
    remove.mockRejectedValueOnce(coreError('Sesja zablokowana'));
    expect(await app.deleteSession('s-trip')).toBe(false);
    expect(app.sessions.list.some((s) => s.id === 's-trip')).toBe(true);
    expect(lastToast(app)?.message).toBe('Sesja zablokowana');
    expect(await app.deleteSession('s-trip')).toBe(true);
    vi.spyOn(client.sessions, 'undoRemove').mockRejectedValue(coreError('Za późno'));
    const undo = lastToast(app);
    if (undo) app.toasts.act(undo.id);
    await flush();
    expect(lastToast(app)).toMatchObject({ kind: 'error', message: 'Za późno' });
  });

  it('cofnięcie kroku agentki: błąd → toast, krok nie jest oznaczony jako cofnięty', async () => {
    const { app, client } = setupApp();
    await app.start();
    vi.spyOn(client.turns, 'undoStep').mockRejectedValue(coreError('Plik zmieniony'));
    expect(await app.undoStep('tok-1', 'Przeniesiono 14 plików')).toBe(false);
    expect(app.runs.isUndone('tok-1')).toBe(false);
    expect(lastToast(app)).toMatchObject({ kind: 'error', message: 'Plik zmieniony' });
  });

  it('szkic: nieudany zapis → jeden toast na serię, po udanym zapisie znowu zgłaszany', async () => {
    const { app, client } = setupApp();
    await app.start();
    const save = vi.spyOn(client.sessions, 'saveDraft').mockRejectedValue(coreError('Dysk pełny'));
    const draftToasts = () =>
      app.toasts.items.filter((t) => t.message.startsWith('Nie udało się zapisać szkicu')).length;
    app.setDraft('a');
    app.setDraft('ab');
    await flush();
    expect(draftToasts()).toBe(1);
    expect(app.sessions.drafts['s-q3']).toBe('ab');
    save.mockResolvedValue(undefined);
    app.setDraft('abc');
    await flush();
    save.mockRejectedValue(coreError('Dysk pełny'));
    app.setDraft('abcd');
    await flush();
    expect(draftToasts()).toBe(2);
  });
});

describe('AppState — ustawienia i skróty', () => {
  it('ustawienie odrzucone przez rdzeń wraca do poprzedniej wartości + toast (też z palety)', async () => {
    const { app, client } = setupApp();
    await app.start();
    vi.spyOn(client.settings, 'set').mockRejectedValue(coreError('Wartość spoza zakresu'));
    expect(await app.setSetting('ui.column', 'wide')).toBe(false);
    expect(app.str('ui.column', 'narrow')).toBe('narrow');
    expect(lastToast(app)).toMatchObject({ kind: 'error', message: 'Wartość spoza zakresu' });
    runCommand(app, 'view.zoomIn');
    expect(app.num('ui.zoom', 100)).toBe(110);
    await flush();
    expect(app.num('ui.zoom', 100)).toBe(100);
  });

  it('język odrzucony przez rdzeń: interfejs wraca do polskiego', async () => {
    const { app, client } = setupApp();
    await app.start();
    vi.spyOn(client.settings, 'set').mockRejectedValue(coreError('Brak zapisu'));
    expect(await app.setSetting('ui.locale', 'en')).toBe(false);
    expect(app.i18n.locale).toBe('pl');
    expect(app.i18n.t('sessions.title')).toBe('Sesje');
  });

  it('przywrócenie domyślnej: błąd → wartość bez zmian + toast', async () => {
    const { app, client } = setupApp();
    await app.start();
    await app.setSetting('ui.column', 'wide');
    vi.spyOn(client.settings, 'reset').mockRejectedValue(coreError('Nie przywrócono'));
    expect(await app.resetSetting('ui.column')).toBe(false);
    expect(app.str('ui.column', 'narrow')).toBe('wide');
    expect(lastToast(app)?.message).toBe('Nie przywrócono');
  });

  it('skrót Ctrl+Alt+S odrzucony (polski AltGr) → wraca poprzedni + toast z powodem', async () => {
    const { app, client } = setupApp();
    await app.start();
    await app.setShortcut('panel.sessions', 'Ctrl+J');
    const reason = 'Ctrl+Alt+S koliduje z polskim AltGr (ś).';
    const set = vi.spyOn(client.settings, 'setShortcut').mockRejectedValue(coreError(reason));
    expect(await app.setShortcut('panel.sessions', 'Ctrl+Alt+S')).toBe(false);
    expect(app.shortcutOverrides['panel.sessions']).toBe('Ctrl+J');
    expect(app.keymap.get('Ctrl+Alt+S')).toBeUndefined();
    expect(app.keymap.get('Ctrl+J')?.id).toBe('panel.sessions');
    expect(lastToast(app)).toMatchObject({ kind: 'error', message: reason });
    expect(await app.setShortcut('panel.sessions', null)).toBe(false);
    expect(app.shortcutOverrides['panel.sessions']).toBe('Ctrl+J');
    expect(set).toHaveBeenCalledTimes(2);
  });
});

describe('Rozmowa, załączniki, Esc — błędy komend', () => {
  it('ocena i ukrycie odrzucone → adnotacja wraca + toast; Stop bez odrzucenia', async () => {
    const { app, client } = setupApp();
    await app.start();
    const conv = app.conversation;
    const turn = conv?.path.find((t) => t.author !== 'user');
    if (!conv || !turn) throw new Error('brak tury agentki w atrapie');
    const rating = conv.annotations[turn.id]?.rating ?? null;
    vi.spyOn(client.turns, 'rate').mockRejectedValue(coreError('Ocena nie zapisana'));
    vi.spyOn(client.turns, 'setHidden').mockRejectedValue(coreError('Nie ukryto'));
    vi.spyOn(client.turns, 'stop').mockRejectedValue(coreError('Nic nie trwa'));
    expect(await conv.rate(turn, rating === 'up' ? 'down' : 'up')).toBe(false);
    expect(conv.annotations[turn.id]?.rating ?? null).toBe(rating);
    expect(lastToast(app)?.message).toBe('Ocena nie zapisana');
    expect(await conv.setHidden(turn, true)).toBe(false);
    expect(conv.annotations[turn.id]?.hidden ?? false).toBe(false);
    expect(lastToast(app)?.message).toBe('Nie ukryto');
    expect(await app.stopGeneration()).toBe(false);
    expect(lastToast(app)?.message).toBe('Nic nie trwa');
  });

  it('załączniki: błąd listy → pusta lista + toast; błąd usunięcia → załącznik zostaje', async () => {
    const { app, client } = setupApp();
    await app.start();
    const att = new AttachmentsState(app);
    await att.pick();
    const staged = att.items[0];
    if (!staged) throw new Error('atrapa nie przygotowała załącznika');
    vi.spyOn(client.attachments, 'remove').mockRejectedValue(coreError('Plik w użyciu'));
    await att.remove(staged);
    expect(att.items).toContain(staged);
    expect(lastToast(app)?.message).toBe('Plik w użyciu');
    vi.spyOn(client.attachments, 'list').mockRejectedValue(coreError('Katalog sesji niedostępny'));
    await att.load('s-api');
    expect(att.items).toEqual([]);
    expect(lastToast(app)?.message).toBe('Nie udało się wczytać: Katalog sesji niedostępny');
  });

  it('Esc w trakcie mowy: odrzucony stop mowy → toast', async () => {
    const { app, client } = setupApp();
    await app.start();
    app.layout.width = 1600;
    app.micState = 'speaking';
    vi.spyOn(client.voice, 'stopSpeech').mockRejectedValue(coreError('Syntezator nie odpowiada'));
    expect(escapeChain(app)).toBe(true);
    await flush();
    expect(lastToast(app)).toMatchObject({ kind: 'error', message: 'Syntezator nie odpowiada' });
  });
});
