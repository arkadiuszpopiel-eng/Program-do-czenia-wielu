import { describe, expect, it } from 'vitest';
import type { BoardItem } from '../../api/types-board';
import { boardMarkdown, filterBoard } from '../board';

const item = (id: string, text: string, title: string, agent = 'conductor'): BoardItem => ({
  id,
  session_id: `s-${id}`,
  session_title: title,
  turn_id: `t-${id}`,
  agent,
  text,
  html_sanitized: `<p>${text}</p>`,
  pinned_at: '2026-10-15T20:00:00Z',
});

const names = (id: string) => (id === 'conductor' ? 'Dyrygentka' : id);

describe('board', () => {
  const items = [
    item('1', 'Plan tygodnia: poniedziałek — raport', 'Planowanie Q3'),
    item('2', 'Przepis na żurek', 'Kuchnia', 'scribe'),
  ];

  it('wyszukiwanie: bez polskich znaków, każde słowo, tytuł i agentka', () => {
    expect(filterBoard(items, '', names)).toHaveLength(2);
    expect(filterBoard(items, 'zurek', names).map((x) => x.id)).toEqual(['2']);
    expect(filterBoard(items, 'PONIEDZIAŁEK q3', names).map((x) => x.id)).toEqual(['1']);
    expect(filterBoard(items, 'dyrygentka', names).map((x) => x.id)).toEqual(['1']);
    expect(filterBoard(items, 'raport kuchnia', names)).toHaveLength(0);
  });

  it('eksport Markdown', () => {
    const md = boardMarkdown('Tablica', items.slice(0, 1), names, () => '15.10.2026 22:00');
    expect(md).toBe(
      '# Tablica\n\n## Dyrygentka · Planowanie Q3 · 15.10.2026 22:00\n\nPlan tygodnia: poniedziałek — raport',
    );
  });
});
