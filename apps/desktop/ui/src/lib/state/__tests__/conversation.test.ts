import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../../api/fake/fake-client';
import { isChatEvent } from '../../logic/apply-event';
import { ConversationState } from '../conversation.svelte';
import { flush } from './helpers';

function setup() {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler });
  const texts: string[] = [];
  let stops = 0;
  const conv = new ConversationState(client, 's-q3', {
    onText: (_id, text) => texts.push(text),
    onStop: () => stops++,
  });
  client.subscribe((batch) => {
    for (const e of batch) if (isChatEvent(e) && e.session_id === 's-q3') conv.apply(e);
  });
  const run = async (ms: number) => {
    scheduler.advance(ms);
    await flush();
  };
  return { client, conv, run, texts, stops: () => stops };
}

describe('ConversationState (runy)', () => {
  it('ładuje drzewo i pokazuje najnowsze warianty', async () => {
    const { conv } = setup();
    await conv.load();
    expect(conv.loaded).toBe(true);
    expect(conv.path.map((t) => t.id)).toEqual(['q1', 'q2', 'q3', 'q4b', 'q5', 'q6']);
  });

  it('regresja: nowa tura pod liściem (nowa tablica dzieci) trafia do ścieżki', async () => {
    const { conv, run } = setup();
    await conv.load();
    await conv.send('Zaplanuj tydzień', null, null);
    await run(5000);
    expect(conv.path).toHaveLength(8);
    expect(conv.path[6]?.author).toBe('user');
    expect(conv.path[7]?.status).toBe('complete');
    expect(conv.streaming).toBeUndefined();
  });

  it('strumień: tekst narasta, bloki z Rust, aria-live dostaje delty, Stop kończy', async () => {
    const { conv, run, texts, stops } = setup();
    await conv.load();
    await conv.send('Hej', null, null);
    // 600 ms „myślenia" w skrypcie atrapy, potem tekst ~100 tok/s.
    await run(900);
    const answer = conv.path[7];
    expect(answer?.status).toBe('streaming');
    expect(conv.streaming?.id).toBe(answer?.id);
    expect(texts.length).toBeGreaterThan(0);
    await run(5000);
    expect(conv.path[7]?.blocks.every((b) => b.closed)).toBe(true);
    expect(stops()).toBe(1);
  });

  it('ponów → nowy wariant jest wybrany; przełączanie ‹ ›', async () => {
    const { conv, run } = setup();
    await conv.load();
    const last = conv.path[5];
    if (!last) throw new Error('brak');
    await conv.regenerate(last);
    await run(5000);
    const variant = conv.path[5];
    expect(variant?.id).not.toBe('q6');
    expect(variant && conv.siblings(variant)).toEqual({ index: 2, total: 2 });
    if (variant) conv.selectVariant(variant, -1);
    expect(conv.path[5]?.id).toBe('q6');
  });

  it('edytuj → gałąź; stara gałąź zostaje nietknięta', async () => {
    const { conv, run } = setup();
    await conv.load();
    const q5 = conv.turn('q5');
    if (!q5) throw new Error('brak');
    const before = q5.text;
    await conv.editAndResend(q5, 'Beta, zrób notatkę');
    await run(5000);
    expect(conv.path[4]?.text).toBe('Beta, zrób notatkę');
    expect(conv.turn('q5')?.text).toBe(before);
    const branch = conv.path[4];
    expect(branch && conv.siblings(branch).total).toBe(2);
  });

  it('ocena i ukrycie to adnotacje (przełączane)', async () => {
    const { conv } = setup();
    await conv.load();
    const q6 = conv.turn('q6');
    if (!q6) throw new Error('brak');
    await conv.rate(q6, 'up');
    expect(conv.annotations['q6']?.rating).toBe('up');
    await conv.rate(q6, 'up');
    expect(conv.annotations['q6']?.rating).toBeNull();
    await conv.setHidden(q6, true);
    expect(conv.annotations['q6']?.hidden).toBe(true);
  });

  it('Stop anuluje i oznacza turę', async () => {
    const { conv, run } = setup();
    await conv.load();
    await conv.send('Napisz szczegółowy esej', null, null);
    await run(100);
    await conv.stop();
    await run(100);
    expect(conv.path[7]?.status).toBe('cancelled');
  });
});
