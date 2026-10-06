import { describe, expect, it } from 'vitest';
import { ATTACHMENT_LIMITS } from '../fake/api-files';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';

function setup() {
  const scheduler = new VirtualScheduler();
  return { client: new FakeAlfaClient({ scheduler }), scheduler };
}

describe('atrapa: załączniki composera', () => {
  it('wybór, wklejenie i upuszczenie z limitami; wysyłka zdejmuje je z composera', async () => {
    const { client } = setup();
    const picked = await client.attachments.pick('s-q3');
    expect(picked.added).toHaveLength(1);
    expect(picked.added[0]?.kind).toBe('document');
    const pasted = await client.attachments.paste('s-q3');
    expect(pasted.added[0]?.kind).toBe('image');
    expect(pasted.added[0]?.tokens).toBeGreaterThan(1_600);
    const dropped = await client.attachments.addDropped('s-q3', [
      { name: 'notatki.md', bytes: 400_000, mime: 'text/markdown' },
      { name: 'film.mp4', bytes: ATTACHMENT_LIMITS.maxFileBytes + 1, mime: 'video/mp4' },
      { name: 'C:\\Users\\Ty\\.claude\\token.json', bytes: 10, mime: 'application/json' },
      { name: 'pusty.txt', bytes: 0, mime: 'text/plain' },
    ]);
    expect(dropped.added.map((a) => a.delivery)).toEqual(['truncated']);
    expect(dropped.rejected.map((r) => r.reason)).toEqual(['too_large', 'denied', 'empty']);
    expect(dropped.staged).toHaveLength(3);
    const ids = dropped.staged.map((a) => a.id);
    const sent = await client.turns.send('s-q3', {
      parent_id: null,
      text: '',
      addressed_to: null,
      profile: null,
      attachments: ids,
    });
    const turn = (await client.turns.list('s-q3')).turns.find((t) => t.id === sent.user_turn_id);
    expect(turn?.attachments?.map((a) => a.name)).toEqual(dropped.staged.map((a) => a.name));
    expect(await client.attachments.list('s-q3')).toEqual([]);
    await expect(
      client.turns.send('s-q3', {
        parent_id: null,
        text: 'x',
        addressed_to: null,
        profile: null,
        attachments: ['nieznany'],
      }),
    ).rejects.toThrow(/Nieznany/);
  });

  it('limit liczby plików i usuwanie', async () => {
    const { client } = setup();
    const hints = Array.from({ length: ATTACHMENT_LIMITS.maxFiles + 2 }, (_, i) => ({
      name: `p${i}.txt`,
      bytes: 10,
      mime: 'text/plain',
    }));
    const out = await client.attachments.addDropped('s-api', hints);
    expect(out.added).toHaveLength(ATTACHMENT_LIMITS.maxFiles);
    expect(out.rejected.every((r) => r.reason === 'too_many')).toBe(true);
    const first = out.staged[0];
    if (!first) throw new Error('brak');
    const left = await client.attachments.remove('s-api', first.id);
    expect(left).toHaveLength(ATTACHMENT_LIMITS.maxFiles - 1);
    expect(client.attachments.previewUrl('C:\\a.png')).toBeNull();
    await expect(client.attachments.pick('brak-sesji')).rejects.toThrow();
  });
});

describe('atrapa: eksport rozmowy i kopie zapasowe', () => {
  it('eksport całej rozmowy i jednej wiadomości', async () => {
    const { client } = setup();
    const md = await client.conversation.exportConversation('s-q3', 'markdown', null);
    expect(md.status === 'saved' && md.path.endsWith('.md')).toBe(true);
    const html = await client.conversation.exportConversation('s-q3', 'html', 'q3');
    expect(html.status === 'saved' && html.path.endsWith('.html')).toBe(true);
    await expect(client.conversation.exportConversation('s-x', 'html', null)).rejects.toThrow();
  });

  it('katalog tylko z dialogu, rotacja, hasło i test przywracania', async () => {
    const { client } = setup();
    const start = await client.backups.status();
    await expect(client.backups.runNow()).rejects.toThrow(/katalog/);
    const ignored = await client.backups.configure({
      ...start.config,
      enabled: true,
      dir: 'C:\\obcy',
      keep: 2,
    });
    expect(ignored.config.dir).toBeNull();
    expect(ignored.config.enabled).toBe(false);
    await client.backups.chooseDir();
    const on = await client.backups.configure({ ...start.config, enabled: true, keep: 2 });
    expect(on.config.enabled && on.next_due !== null).toBe(true);
    await client.backups.runNow();
    await client.backups.runNow();
    const view = await client.backups.runNow();
    expect(view.entries).toHaveLength(2);
    await expect(client.backups.setPassword('krótko')).rejects.toThrow();
    expect((await client.backups.setPassword('długie hasło kopii')).password_set).toBe(true);
    const newest = view.entries[0];
    if (!newest) throw new Error('brak kopii');
    const check = await client.backups.verify(newest.file);
    expect(check.ok && check.encrypted).toBe(true);
    await expect(client.backups.verify('../../x.alfa')).rejects.toThrow();
  });
});
