<!-- Jarvis jest sposobem współpracy konkretnej persony; zmiana dotyczy kolejnego zadania. -->
<script lang="ts">
  import { Button, Select, agents, type AgentId } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  interface Props {
    agent: AgentId;
  }
  let { agent }: Props = $props();
  const app = useApp();
  const en = $derived(app.i18n.locale === 'en');
  const key = $derived(`agents.${agent}.interaction_mode`);
  const value = $derived(app.str(key, 'inherit'));
  const effective = $derived(
    value === 'inherit' ? app.str('agents.interaction_mode', 'standard') : value,
  );
  const jarvis = $derived(effective === 'jarvis');
  let busy = $state(false);
  const label = $derived(
    en ? `${agents[agent].name} — interaction style` : `${agents[agent].name} — sposób współpracy`,
  );
  const options = $derived([
    { value: 'inherit', label: en ? 'Use common setting' : 'Jak ustawienie wspólne' },
    { value: 'standard', label: en ? 'Standard' : 'Standardowy' },
    { value: 'jarvis', label: 'Jarvis' },
  ]);

  async function change(next: string) {
    if (busy) return;
    busy = true;
    try {
      await app.setSetting(key, next);
    } finally {
      busy = false;
    }
  }
</script>

<div class="interaction" data-agent={agent} data-interaction={effective}>
  <label for="interaction-{agent}">{label}</label>
  <Select
    id="interaction-{agent}"
    size="sm"
    {value}
    {options}
    disabled={busy}
    onchange={(next) => void change(next)}
  />
  {#if jarvis}
    <p class="hint" role="status">
      {en
        ? 'Jarvis: concise updates, requested tasks and voice conversation. Your approval mode and task limits still apply.'
        : 'Jarvis: krótkie meldunki, zlecone zadania i rozmowa głosowa. Obowiązują wybrany tryb zgód i limity zadania.'}
    </p>
    <p class="hint">
      {en
        ? `Turn on the microphone in Voice and address ${agents[agent].name} by name.`
        : `Włącz mikrofon w panelu Głos i zwróć się do agentki po imieniu: ${agents[agent].name}.`}
    </p>
    <div class="actions">
      <Button size="sm" variant="ghost" onclick={() => app.openPanel('voice')}
        >{en ? 'Voice controls' : 'Sterowanie głosem'}</Button
      >
    </div>
  {/if}
</div>

<style>
  .interaction {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    min-width: 0;
    margin-top: var(--alfa-space-2);
  }
  label {
    font-size: var(--alfa-font-size-xs);
    color: var(--alfa-color-text-muted);
  }
  .hint {
    margin: 0;
    font-size: var(--alfa-font-size-xs);
    color: var(--alfa-color-text-muted);
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
</style>
