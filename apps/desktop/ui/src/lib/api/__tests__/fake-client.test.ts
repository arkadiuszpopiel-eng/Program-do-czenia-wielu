import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';
import { renderBlocks, escapeHtml } from '../fake/render';
import type { AlfaEvent } from '../types-system';

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function setup(scenario: ConstructorParameters<typeof FakeAlfaClient>[0] = {}) {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler, ...scenario });
  const events: AlfaEvent[] = [];
  client.subscribe((batch) => events.push(...batch));
  return { client, scheduler, events };
}

describe('FakeAlfaClient — strumień', () => {
  it('~100 tokenów/s z TextDelta, Usage i Stop na końcu', async () => {
    const { client, scheduler, events } = setup();
    const sid = (await client.sessions.create('empty')).id;
    const sent = await client.turns.send(sid, {
      parent_id: null,
      text: 'Zaplanuj tydzień',
      addressed_to: null,
      profile: null,
    });
    expect(sent.assistant_turn_id).not.toBeNull();
    scheduler.advance(200);
    await flush();
    const deltas = events.filter((e) => e.type === 'TextDelta');
    // 200 ms przy 100 tok/s ≈ 20 delt (pierwsza od razu).
    expect(deltas.length).toBeGreaterThanOrEqual(20);
    expect(deltas.length).toBeLessThanOrEqual(21);
    scheduler.runAll();
    await flush();
    const types = events.map((e) => e.type);
    expect(types).toContain('Usage');
    expect(types[types.lastIndexOf('Stop')]).toBe('Stop');
    const snapshot = await client.turns.list(sid);
    const answer = snapshot.turns.find((t) => t.id === sent.assistant_turn_id);
    expect(answer?.status).toBe('complete');
    expect(answer?.blocks.every((b) => b.closed)).toBe(true);
  });

  it('deterministyczny: te same wejścia → te same zdarzenia', async () => {
    const run = async () => {
      const { client, scheduler, events } = setup();
      await client.turns.send('s-api', {
        parent_id: 's-api-2',
        text: 'Napisz kod',
        addressed_to: null,
        profile: null,
      });
      scheduler.runAll();
      await flush();
      return events.map((e) => (e.type === 'TextDelta' ? e.text : e.type)).join('|');
    };
    expect(await run()).toBe(await run());
  });

  it('Stop anuluje strumień', async () => {
    const { client, scheduler, events } = setup();
    const sent = await client.turns.send('s-api', {
      parent_id: 's-api-2',
      text: 'cokolwiek',
      addressed_to: null,
      profile: null,
    });
    scheduler.advance(900);
    await client.turns.stop('s-api');
    scheduler.runAll();
    await flush();
    const stop = events.find((e) => e.type === 'Stop');
    expect(stop && stop.type === 'Stop' && stop.reason).toBe('cancelled');
    const list = await client.turns.list('s-api');
    expect(list.turns.find((t) => t.id === sent.assistant_turn_id)?.status).toBe('cancelled');
  });

  it('ponów → wariant obok; edytuj → gałąź; tury się nie zmieniają', async () => {
    const { client, scheduler } = setup();
    const before = await client.turns.list('s-q3');
    const variant = await client.turns.regenerate('s-q3', 'q6', null);
    const branch = await client.turns.editAndResend('s-q3', 'q5', 'Beta, zrób z tego notatkę');
    scheduler.runAll();
    const after = await client.turns.list('s-q3');
    for (const old of before.turns) {
      const same = after.turns.find((t) => t.id === old.id);
      expect(same?.text).toBe(old.text);
      expect(same?.parent_id).toBe(old.parent_id);
    }
    expect(after.turns.find((t) => t.id === variant)?.parent_id).toBe('q5');
    expect(after.turns.find((t) => t.id === branch.user_turn_id)?.parent_id).toBe('q4b');
  });
});

describe('FakeAlfaClient — scenariusze', () => {
  it('offline: wiadomość w kolejce, po powrocie wysłana', async () => {
    const { client, scheduler, events } = setup({ scenario: 'offline' });
    const sent = await client.turns.send('s-api', {
      parent_id: 's-api-2',
      text: 'test',
      addressed_to: null,
      profile: null,
    });
    expect(sent.assistant_turn_id).toBeNull();
    expect((await client.system.status()).queued_messages).toBe(1);
    client.setOnline(true);
    scheduler.runAll();
    await flush();
    expect(events.some((e) => e.type === 'TurnStatus' && e.status === 'complete')).toBe(true);
    expect((await client.system.status()).queued_messages).toBe(0);
  });

  it('429: błąd z czasem odnowienia', async () => {
    const { client, scheduler, events } = setup({ scenario: 'rate-limited' });
    await client.turns.send('s-api', {
      parent_id: 's-api-2',
      text: 'test',
      addressed_to: null,
      profile: null,
    });
    scheduler.runAll();
    await flush();
    const error = events.find((e) => e.type === 'Error');
    expect(error && error.type === 'Error' && error.error.code).toBe('rate_limited');
    expect(error && error.type === 'Error' && error.error.retry_at).toMatch(/^2026-09-30T08:48/);
  });

  it('brak kluczy: profil lokalny, dodanie klucza odblokowuje', async () => {
    const { client } = setup({ scenario: 'no-keys' });
    expect((await client.system.status()).profile).toBe('local');
    const account = await client.accounts.add({
      provider_id: 'anthropic',
      label: 'A',
      secret: 'sk-test',
      base_url: null,
    });
    expect(JSON.stringify(account)).not.toContain('sk-test');
    const report = await client.accounts.test(account.id);
    expect(report.ok).toBe(true);
    expect((await client.system.status()).keys_configured).toBe(true);
  });

  it('usunięcie sesji cofalne przez 10 s', async () => {
    const { client, scheduler } = setup();
    const ticket = await client.sessions.remove('s-trip');
    expect((await client.sessions.list()).some((s) => s.id === 's-trip')).toBe(false);
    await client.sessions.undoRemove(ticket.token);
    expect((await client.sessions.list()).some((s) => s.id === 's-trip')).toBe(true);
    const again = await client.sessions.remove('s-trip');
    scheduler.advance(10_001);
    await client.sessions.undoRemove(again.token);
    expect((await client.sessions.list()).some((s) => s.id === 's-trip')).toBe(false);
  });

  it('pierwsze uruchomienie: onboarding i pusta lista', async () => {
    const { client } = setup({ scenario: 'first-run' });
    const boot = await client.app.bootstrap();
    expect(boot.onboarding_done).toBe(false);
    expect(await client.sessions.list()).toEqual([]);
  });
});

describe('atrapa renderera — bezpieczny HTML', () => {
  it('escape i bloki kodu', () => {
    expect(escapeHtml('<img src=x onerror=alert(1)>')).toBe('&lt;img src=x onerror=alert(1)&gt;');
    const blocks = renderBlocks('Akapit <b>\n\n```js\nalert("x")\n```\n\n- a\n- b', false);
    expect(blocks.map((b) => b.kind)).toEqual(['text', 'code', 'text']);
    expect(blocks[0]?.html_sanitized).toBe('<p>Akapit &lt;b&gt;</p>');
    expect(blocks[1]?.html_sanitized).toContain('class="language-js"');
    expect(blocks[1]?.html_sanitized).not.toContain('"x"');
    expect(blocks[2]?.html_sanitized).toBe('<ul><li>a</li><li>b</li></ul>');
  });

  it('otwarty blok w trakcie strumienia', () => {
    const blocks = renderBlocks('Pierwszy.\n\nDrugi w toku', true);
    expect(blocks.map((b) => b.closed)).toEqual([true, false]);
    expect(renderBlocks('```ts\nconst a', true)[0]?.closed).toBe(false);
  });
});
