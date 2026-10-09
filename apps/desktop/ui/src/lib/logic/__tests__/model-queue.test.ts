// Kolejki modeli konta API: dopasowanie modelu do klasy zadań, operacje na liście i filtry.
import { describe, expect, it } from 'vitest';
import { OPENAI_MODELS, fakeModel, suggestFake } from '../../api/fake/model-info';
import { en } from '../../i18n/en';
import { pl } from '../../i18n/pl';
import {
  MAX_QUEUE,
  PURPOSES,
  STRATEGIES,
  TASK_CLASSES,
  addBlock,
  countByPurpose,
  filterModels,
  fitsClass,
  moved,
} from '../model-queue';

const everything = { query: '', purposes: new Set<never>(), hideSnapshots: false, fitting: null };

describe('dopasowanie modelu do klasy zadań', () => {
  it('rozmowa nie przyjmuje syntezy mowy, embeddingi przyjmują tylko embeddingi', () => {
    expect(fitsClass('chat', 'chat')).toBe(true);
    expect(fitsClass('chat', 'reasoning')).toBe(true);
    expect(fitsClass('chat', 'speech')).toBe(false);
    expect(fitsClass('embeddings', 'embedding')).toBe(true);
    expect(fitsClass('embeddings', 'chat')).toBe(false);
    // Szybka odpowiedź głosowa i sterowanie ekranem nie czekają na model rozumujący.
    expect(fitsClass('voice_fast', 'reasoning')).toBe(false);
    expect(fitsClass('gui_vision', 'reasoning')).toBe(false);
    expect(fitsClass('planning', 'reasoning')).toBe(true);
  });

  it('mówi, dlaczego modelu nie da się dodać', () => {
    const queue = ['gpt-6-sol'];
    expect(addBlock(queue, 'chat', fakeModel('gpt-6-luna'))).toBeNull();
    expect(addBlock(queue, 'chat', fakeModel('gpt-6-sol'))).toBe('present');
    expect(addBlock(queue, 'chat', fakeModel('tts-1'))).toBe('unfit');
    const full = Array.from({ length: MAX_QUEUE }, (_, n) => `model-${n}`);
    expect(addBlock(full, 'chat', fakeModel('gpt-6-luna'))).toBe('full');
  });
});

describe('operacje na kolejce', () => {
  it('przesuwa pozycję i nie rusza listy poza zakresem', () => {
    const queue = ['a', 'b', 'c'];
    expect(moved(queue, 2, -1)).toEqual(['a', 'c', 'b']);
    expect(moved(queue, 0, 1)).toEqual(['b', 'a', 'c']);
    expect(moved(queue, 0, -1)).toEqual(queue);
    expect(moved(queue, 2, 1)).toEqual(queue);
    expect(moved(queue, 7, 1)).toEqual(queue);
    expect(queue).toEqual(['a', 'b', 'c']);
  });
});

describe('przeglądarka modeli', () => {
  it('szuka po nazwie i identyfikatorze, każde słowo osobno', () => {
    const found = filterModels(OPENAI_MODELS, { ...everything, query: '  MINI 4o ' });
    expect(found.map((m) => m.id)).toEqual(['gpt-4o-mini', 'gpt-4o-mini-tts']);
  });

  it('filtruje po rodzaju, klasie i migawkach', () => {
    const speech = filterModels(OPENAI_MODELS, { ...everything, purposes: new Set(['speech']) });
    expect(speech.map((m) => m.id)).toEqual(['tts-1', 'tts-1-hd', 'gpt-4o-mini-tts']);
    const chat = filterModels(OPENAI_MODELS, { ...everything, fitting: 'chat' });
    expect(chat.every((m) => fitsClass('chat', m.purpose))).toBe(true);
    expect(chat.some((m) => m.snapshot)).toBe(true);
    const undated = filterModels(OPENAI_MODELS, { ...everything, hideSnapshots: true });
    expect(undated.some((m) => m.snapshot)).toBe(false);
    const counts = countByPurpose(OPENAI_MODELS);
    expect(counts.get('speech')).toBe(3);
    expect([...counts.values()].reduce((a, b) => a + b, 0)).toBe(OPENAI_MODELS.length);
  });

  it('propozycja atrapy ma tylko pasujące modele, bez migawek, najwyżej dziesięć', () => {
    for (const taskClass of TASK_CLASSES)
      for (const strategy of STRATEGIES) {
        const queue = suggestFake(OPENAI_MODELS, taskClass, strategy);
        expect(queue.length, `${taskClass}/${strategy}`).toBeGreaterThan(0);
        expect(queue.length).toBeLessThanOrEqual(MAX_QUEUE);
        for (const id of queue) expect(addBlock([], taskClass, fakeModel(id))).toBeNull();
      }
    expect(suggestFake(OPENAI_MODELS, 'chat', 'economy')[0]).toContain('nano');
  });
});

describe('teksty strony API', () => {
  for (const [name, text] of [
    ['pl', pl as Record<string, unknown>],
    ['en', en as Record<string, unknown>],
  ] as const)
    it(`${name}: każda klasa, strategia, rodzaj i blokada mają tekst`, () => {
      for (const id of TASK_CLASSES) {
        expect(text[`api.class.${id}`], id).toBeTruthy();
        expect(text[`api.classHint.${id}`], id).toBeTruthy();
      }
      for (const id of STRATEGIES) expect(text[`api.strategy.${id}`], id).toBeTruthy();
      for (const id of PURPOSES) expect(text[`api.purpose.${id}`], id).toBeTruthy();
      for (const id of ['present', 'full', 'unfit'])
        expect(text[`api.block.${id}`], id).toBeTruthy();
      for (const id of ['standard', 'small', 'tiny', 'premium'])
        expect(text[`api.tier.${id}`], id).toBeTruthy();
    });
});
