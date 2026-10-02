<!--
  Uruchomienie umiejętności: parametry (JSON, szablon ze schematu) i agentka → zadanie agentki
  w bieżącej sesji (widoczne w panelu Zadania i w Replay). Parametry waliduje rdzeń schematem.
-->
<script lang="ts">
  import { Button, Select } from '@alfa/ui-kit';
  import type { SkillInfo } from '../../../api/types-work';
  import { agentName, paramsTemplate } from '../../../logic/work';
  import { useApp } from '../../../state/context';

  interface Props {
    skill: SkillInfo;
    ondone: () => void;
  }

  let { skill, ondone }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const sessionId = $derived(app.activeId);
  const options = $derived([
    { value: '', label: t('skills.agentAuto') },
    ...(sessionId ? (app.agents[sessionId] ?? []) : []).map((a) => ({
      value: a.id as string,
      label: agentName(a.id),
    })),
  ]);
  let agent = $state('');
  let params = $state('');
  let error = $state<string | null>(null);

  $effect(() => {
    params = JSON.stringify(paramsTemplate(skill.parameters), null, 2);
    error = null;
  });

  async function start() {
    if (!sessionId) return;
    error = null;
    let parsed: unknown;
    try {
      parsed = JSON.parse(params);
    } catch {
      error = t('skills.paramsInvalid');
      return;
    }
    try {
      const task = await app.client.skills.run(skill.id, sessionId, agent || null, parsed);
      app.toasts.show({ kind: 'success', message: t('skills.started', { title: task.title }) });
      app.openPanel('tasks');
      ondone();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<section class="wk-card wk-accent" aria-labelledby="sk-run">
  <h3 id="sk-run">{t('skills.runTitle', { name: skill.name })}</h3>
  <p class="wk-meta">
    {t('skills.runSession', { title: app.sessions.active?.title ?? sessionId ?? '—' })}
  </p>
  <Select bind:value={agent} {options} label={t('skills.agent')} />
  <label class="wk-field">
    <span>{t('skills.params')}</span>
    <textarea
      rows="5"
      spellcheck="false"
      bind:value={params}
      aria-invalid={error ? 'true' : undefined}></textarea>
  </label>
  {#if error}<p class="wk-error" role="alert">{error}</p>{/if}
  <div class="wk-actions">
    <Button size="sm" variant="primary" disabled={!sessionId} onclick={start}
      >{t('skills.runStart')}</Button
    >
    <Button size="sm" variant="ghost" onclick={ondone}>{t('common.cancel')}</Button>
  </div>
</section>
