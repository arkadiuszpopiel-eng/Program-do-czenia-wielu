// Pipeline'y modeli (okno „Pipeline'y”): czysta logika edycji — bez dostępu do API i DOM.
import type { PipelineDef, PipelineStage } from '../api/types';

/** Najwięcej etapów w jednym pipeline (jak w rdzeniu). */
export const MAX_STAGES = 6;

/** Role wykonawcze dostępne w etapie (identyfikatory obsady Alfy). */
export const STAGE_ROLES: readonly { id: string; pl: string; en: string }[] = [
  { id: 'thinker', pl: 'Myślicielka (analiza, projekt)', en: 'Thinker (analysis, design)' },
  { id: 'coder', pl: 'Koderka (kod, pliki)', en: 'Coder (code, files)' },
  { id: 'operator', pl: 'Wykonawczyni (polecenia, system)', en: 'Operator (commands, system)' },
  { id: 'researcher', pl: 'Badaczka (źródła, fakty)', en: 'Researcher (sources, facts)' },
  { id: 'critic', pl: 'Krytyczka (przegląd, weryfikacja)', en: 'Critic (review, verification)' },
  { id: 'writer', pl: 'Pisarka (dokumenty, teksty)', en: 'Writer (documents, text)' },
  { id: 'keeper', pl: 'Strażniczka (pamięć, porządek)', en: 'Keeper (memory, order)' },
];

/** Agentki obsady (pusta wartość = wybór według roli). */
export const STAGE_PERSONAS: readonly { id: string; label: string }[] = [
  { id: 'delta', label: 'Delta (kod, pliki, terminal)' },
  { id: 'gama', label: 'Gama (analiza, badanie, przegląd)' },
  { id: 'beta', label: 'Beta (dokumentacja, pamięć)' },
];

/** Wbudowane nazwy wzorców (nadpisany wzorzec można przywrócić do domyślnego). */
export const BUILTIN_NAMES: readonly string[] = ['ui-design', 'code-review', 'research-report'];

/** Podpowiadane identyfikatory modeli, gdy właściciel chce wpisać konkretny model na etapie. */
export const SUGGESTED_MODELS: readonly { candidate: string; label: string }[] = [
  { candidate: 'openai:gpt-6-astra', label: 'OpenAI · GPT-6 Astra (styl / wizja)' },
  { candidate: 'anthropic:claude-opus-5-5', label: 'Anthropic · Claude Opus 5.5 (architektura)' },
  { candidate: 'openai:gpt-6.1-sol', label: 'OpenAI · GPT-6.1 Sol (kod / implementacja)' },
  { candidate: 'anthropic:claude-sonnet-5-5', label: 'Anthropic · Claude Sonnet 5.5' },
  { candidate: 'openai:gpt-5', label: 'OpenAI · GPT-5' },
  { candidate: 'google:gemini-2.5-pro', label: 'Google · Gemini 2.5 Pro' },
];

export function insertGoalToken(goal: string, token: '{{input}}' | '{{prev}}'): string {
  if (goal.includes(token)) return goal;
  const trimmed = goal.trimEnd();
  return trimmed ? `${trimmed}\n\n${token}` : token;
}

export function blankStage(): PipelineStage {
  return { title: '', role: 'thinker', persona: null, model: null, goal: '{{prev}}' };
}

export function blankPipeline(): PipelineDef {
  return {
    name: '',
    description: '',
    stages: [{ ...blankStage(), goal: '{{input}}' }],
    builtin: false,
  };
}

/** Przesuwa etap o `delta` miejsc (poza zakresem — bez zmian). */
export function moveStage(
  stages: readonly PipelineStage[],
  index: number,
  delta: -1 | 1,
): PipelineStage[] {
  const next = [...stages];
  const to = index + delta;
  if (index < 0 || index >= next.length || to < 0 || to >= next.length) return next;
  [next[index], next[to]] = [next[to]!, next[index]!];
  return next;
}

/** Kopia pod wolną nazwą („Nazwa (kopia)”, „Nazwa (kopia 2)”…), zawsze jako własny pipeline. */
export function copyOf(def: PipelineDef, takenNames: readonly string[]): PipelineDef {
  const taken = new Set(takenNames.map((n) => n.trim().toLowerCase()));
  let name = `${def.name} (kopia)`;
  for (let i = 2; taken.has(name.toLowerCase()); i += 1) name = `${def.name} (kopia ${i})`;
  return { ...def, name, builtin: false, stages: def.stages.map((s) => ({ ...s })) };
}

/** Model w zapisie `dostawca:model` (albo pusty = Router). */
export function validModel(model: string | null): boolean {
  const t = (model ?? '').trim();
  return t === '' || /^[^:\s]+:\S+$/.test(t);
}

/** Komunikat o pierwszym błędzie edycji (`null` — pipeline można zapisać). */
export function validatePipeline(def: PipelineDef, en: boolean): string | null {
  if (!def.name.trim()) return en ? 'Give the pipeline a name.' : 'Podaj nazwę pipeline’u.';
  if (def.stages.length === 0 || def.stages.length > MAX_STAGES) {
    return en ? `A pipeline needs 1–${MAX_STAGES} stages.` : `Pipeline ma mieć 1–${MAX_STAGES} etapów.`;
  }
  for (const [i, s] of def.stages.entries()) {
    const n = i + 1;
    if (!s.goal.trim()) return en ? `Stage ${n}: describe the task.` : `Etap ${n}: opisz zadanie.`;
    if (!s.role.trim() && !s.persona) return en ? `Stage ${n}: pick a role.` : `Etap ${n}: wybierz rolę.`;
    if (!validModel(s.model)) {
      return en
        ? `Stage ${n}: use the provider:model format.`
        : `Etap ${n}: użyj zapisu dostawca:model.`;
    }
  }
  return null;
}

/** Wiadomość uruchamiająca pipeline po nazwie (Dyrygentka wywołuje `run_pipeline`). */
export function runMessage(name: string, input: string): string {
  return `Uruchom pipeline „${name}”.\n\nWejście:\n${input.trim()}`;
}

/** Łańcuch ról do podglądu na liście („Myślicielka → Koderka”). */
export function chainLabel(def: PipelineDef, en: boolean): string {
  return def.stages
    .map((s) => {
      const role = STAGE_ROLES.find((r) => r.id === s.role);
      const label = role ? (en ? role.en : role.pl).split(' (')[0]! : s.role;
      return s.model ? `${label} · ${s.model.split(':').slice(1).join(':')}` : label;
    })
    .join(' → ');
}
