// Atrapa: szablony promptów w pamięci; walidacja jak w rdzeniu (nazwa, treść, zmienne, limity).
import type { TemplatesApi } from '../client-templates';
import type { PromptTemplate } from '../types-templates';
import type { FakeCore } from './core';

const SEED: readonly PromptTemplate[] = [
  {
    id: 'e-mail',
    name: 'E-mail',
    text: 'Napisz uprzejmy e-mail do {{adresat}} w sprawie: {{temat}}. Krótko, po polsku.',
  },
  {
    id: 'streszczenie',
    name: 'Streszczenie',
    text: 'Streść w 5 punktach:\n\n{{tekst}}',
  },
];

function check(t: PromptTemplate): PromptTemplate {
  const name = t.name.trim();
  const text = t.text.trim();
  if (!name || name.length > 60) throw new Error('nazwa szablonu od 1 do 60 znaków');
  if (!text || text.length > 8000) throw new Error(`„${name}”: treść od 1 do 8000 znaków`);
  const open = (text.match(/\{\{/g) ?? []).length;
  const closed = (text.match(/\}\}/g) ?? []).length;
  if (open !== closed) throw new Error(`„${name}”: niedomknięta zmienna „{{”`);
  return { ...t, name, text };
}

const slug = (name: string): string =>
  name
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '') || 'szablon';

export class FakeTemplates {
  private templates: PromptTemplate[] = [...SEED];

  constructor(private readonly core: FakeCore) {}

  private save(list: readonly PromptTemplate[]): readonly PromptTemplate[] {
    if (list.length > 100) throw new Error('najwyżej 100 szablonów');
    const out: PromptTemplate[] = [];
    for (const t of list) {
      const ok = check(t);
      const taken = new Set(out.map((o) => o.id));
      let id = ok.id || slug(ok.name);
      for (let n = 2; taken.has(id); n += 1) id = `${slug(ok.name)}-${n}`;
      out.push({ ...ok, id });
    }
    this.templates = out;
    return this.templates;
  }

  api(): TemplatesApi {
    return {
      list: () => this.core.reply(this.templates),
      save: async (templates) => this.core.reply(this.save(templates)),
    };
  }
}
