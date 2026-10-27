// Atrapa: biblioteka pipeline'ów modeli (`pipelines_*`) — te same reguły co rdzeń: unikalna nazwa
// (bez rozróżniania wielkości liter), 1–6 etapów, zapis własnej wersji nadpisuje wzorzec wbudowany.
import type { PipelineDef, PipelineStage } from '../types';

const stage = (title: string, role: string, goal: string): PipelineStage => ({
  title,
  role,
  persona: null,
  model: null,
  goal,
});

const builtins = (): PipelineDef[] => [
  {
    name: 'ui-design',
    description: 'Szkic/opis → wytyczne wizualne → architektura komponentów → kod React/Tailwind.',
    builtin: true,
    stages: [
      stage('Styl i typografia', 'thinker', 'Dobierz styl, kolory i typografię.\n\n{{input}}'),
      stage('Architektura komponentów', 'thinker', 'Zaprojektuj komponenty, stany i dostępność.\n\n{{prev}}'),
      stage('Kod React/Tailwind', 'coder', 'Wygeneruj finalny kod.\n\n{{prev}}'),
    ],
  },
  {
    name: 'code-review',
    description: 'Implementacja → krytyczny przegląd → poprawki z testami.',
    builtin: true,
    stages: [
      stage('Implementacja', 'coder', '{{input}}'),
      stage('Przegląd', 'critic', 'Zrób przegląd zmian.\n\n{{prev}}'),
      stage('Poprawki', 'coder', 'Wprowadź poprawki.\n\n{{prev}}'),
    ],
  },
  {
    name: 'research-report',
    description: 'Badanie tematu → weryfikacja wiarygodności → zwięzły raport.',
    builtin: true,
    stages: [
      stage('Badanie', 'researcher', '{{input}}'),
      stage('Weryfikacja', 'critic', '{{prev}}'),
      stage('Raport', 'writer', '{{prev}}'),
    ],
  },
];

const same = (a: string, b: string): boolean => a.trim().toLowerCase() === b.trim().toLowerCase();

export class FakePipelines {
  private saved: PipelineDef[] = [];

  list(): PipelineDef[] {
    const rest = builtins().filter((b) => !this.saved.some((d) => same(d.name, b.name)));
    return [...this.saved, ...rest];
  }

  save(pipeline: PipelineDef, previousName: string | null): PipelineDef[] {
    const name = pipeline.name.trim();
    if (!name) throw new Error('Nazwa pipeline’u: 1–60 znaków.');
    if (pipeline.stages.length < 1 || pipeline.stages.length > 6) {
      throw new Error('Pipeline ma mieć od 1 do 6 etapów.');
    }
    const clean: PipelineDef = { ...pipeline, name, builtin: false };
    const prev = previousName?.trim();
    if (prev && !same(prev, name)) {
      if (this.saved.some((d) => same(d.name, name))) throw new Error(`Pipeline „${name}” już istnieje.`);
      this.saved = this.saved.filter((d) => !same(d.name, prev));
    }
    const at = this.saved.findIndex((d) => same(d.name, name));
    if (at >= 0) this.saved[at] = clean;
    else this.saved.push(clean);
    return this.list();
  }

  remove(name: string): PipelineDef[] {
    this.saved = this.saved.filter((d) => !same(d.name, name));
    return this.list();
  }
}
