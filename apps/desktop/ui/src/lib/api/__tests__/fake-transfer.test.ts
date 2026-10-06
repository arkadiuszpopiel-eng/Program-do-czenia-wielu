// Atrapa: plik importu `.alfa` tylko jednorazowym uchwytem (jak w rdzeniu) — dialog, wynik
// podglądu, „Przywróć…” z listy kopii (15 s); ścieżka podana przez UI nie działa.
import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';
import { RESTORE_HANDLE_TTL_MS } from '../fake/transfer-handles';

function setup() {
  const scheduler = new VirtualScheduler();
  return { client: new FakeAlfaClient({ scheduler }), scheduler };
}

const request = (handle: string) => ({
  handle,
  mode: 'merge' as const,
  resolutions: {},
  password: null,
});

describe('atrapa: uchwyty plików importu', () => {
  it('dialog → podgląd z uchwytem → import zużywa uchwyt; ścieżka z UI odrzucona', async () => {
    const { client } = setup();
    const res = await client.transfer.inspect(null, null);
    if (res.status !== 'inspected') throw new Error(res.status);
    expect(res.handle).not.toContain('\\');
    await expect(
      client.transfer.inspect(null, 'C:\\Users\\Ty\\Pobrane\\laptop.alfa'),
    ).rejects.toThrow(/Uchwyt pliku wygasł/);
    expect((await client.transfer.importPackage(request(res.handle))).imported).toBeGreaterThan(0);
    await expect(client.transfer.importPackage(request(res.handle))).rejects.toThrow(
      /został już użyty/,
    );
  });

  it('„Przywróć…”: uchwyt kopii ważny 15 s; kopia szyfrowana prosi o hasło', async () => {
    const { client, scheduler } = setup();
    await client.backups.chooseDir();
    await client.backups.setPassword('długie hasło kopii');
    const view = await client.backups.runNow();
    const file = view.entries[0]?.file ?? '';
    await expect(client.backups.restore('nie-ma.alfa')).rejects.toThrow(/Brak kopii/);

    const stale = await client.backups.restore(file);
    scheduler.advance(RESTORE_HANDLE_TTL_MS + 1);
    await expect(client.transfer.inspect(null, stale)).rejects.toThrow(/wygasł/);

    const locked = await client.transfer.inspect(null, await client.backups.restore(file));
    if (locked.status !== 'needs_password') throw new Error(locked.status);
    const open = await client.transfer.inspect('długie hasło kopii', locked.handle);
    if (open.status !== 'inspected') throw new Error(open.status);
    expect(open.manifest.encrypted).toBe(true);
    expect(open.handle).not.toBe(locked.handle);
  });
});
