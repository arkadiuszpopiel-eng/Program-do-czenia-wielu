<!--
  Nowy wyzwalacz: nazwa, rodzaj (harmonogram cron z podglądem najbliższych uruchomień, co N minut,
  nowy plik w katalogu, ręcznie), cel dla agentki, agentka, „nie przeszkadzać". Właściciel = Ty;
  most CLI z harmonogramu wymaga osobnej zgody na karcie mostu.
-->
<script lang="ts">
  import {
    Button,
    Checkbox,
    SegmentedControl,
    Select,
    TextField,
    agents as AGENTS,
  } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { CronPreview, TriggerKindView } from '../../../api/types-tasks';
  import { DUTIES_ENABLED, DUTY_TEMPLATES, type DutyTemplate } from '../../../logic/duties';
  import { useApp } from '../../../state/context';

  interface Props {
    oncreated: () => void;
  }

  let { oncreated }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let name = $state('');
  let kind = $state('cron');
  let expr = $state('0 8 * * 1-5');
  let every = $state(60);
  let dir = $state('');
  let pattern = $state('');
  let goal = $state('');
  let agent = $state('');
  let dnd = $state(true);
  let preview = $state<CronPreview | null>(null);

  $effect(() => {
    if (kind !== 'cron') return;
    const current = expr;
    // Błąd komendy (nie samego wyrażenia) też trafia pod pole — nie jako nieobsłużone odrzucenie.
    void app.client.triggers.previewCron(current).then(
      (p) => {
        if (current === expr) preview = p;
      },
      (error: unknown) => {
        if (current === expr) preview = { valid: false, error: errorText(error), next: [] };
      },
    );
  });

  /** Gotowy obowiązek: wypełnia formularz (nazwa, harmonogram, polecenie, agentka) — nic nie tworzy. */
  function useDuty(duty: DutyTemplate) {
    name = t(duty.name);
    kind = 'cron';
    expr = duty.cron;
    goal = t(duty.goal);
    agent = duty.agent;
    app.announcement = t('duty.filled', { name: t(duty.name) });
  }

  function view(): TriggerKindView {
    switch (kind) {
      case 'cron':
        return { kind: 'cron', expr: expr.trim() };
      case 'interval':
        return { kind: 'interval', every_minutes: Math.max(1, Math.round(Number(every) || 1)) };
      case 'file_in_dir':
        return { kind: 'file_in_dir', dir: dir.trim(), pattern: pattern.trim() || null };
      default:
        return { kind: 'manual' };
    }
  }

  async function create() {
    try {
      const created = await app.client.triggers.create({
        name: name.trim(),
        kind: view(),
        title: name.trim(),
        goal: goal.trim(),
        agent: agent || null,
        bridge: null,
        respect_dnd: dnd,
      });
      app.toasts.show({ kind: 'success', message: t('triggers.created', { name: created.name }) });
      name = '';
      goal = '';
      oncreated();
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
</script>

<form
  class="form"
  aria-labelledby="trg-new"
  onsubmit={(e) => {
    e.preventDefault();
    void create();
  }}
>
  <h3 id="trg-new">{t('triggers.new')}</h3>
  {#if DUTIES_ENABLED}
    <div class="duties" role="group" aria-labelledby="trg-duties">
      <h4 id="trg-duties">{t('duty.title')}</h4>
      <p class="hint">{t('duty.hint')}</p>
      <ul>
        {#each DUTY_TEMPLATES as duty (duty.id)}
          <li>
            <Button size="sm" variant="secondary" onclick={() => useDuty(duty)}
              >{t(duty.name)}</Button
            >
            <span class="hint">{t(duty.when)} · {AGENTS[duty.agent].name}</span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
  <TextField label={t('triggers.name')} bind:value={name} />
  <SegmentedControl
    label={t('triggers.kind')}
    bind:value={kind}
    options={[
      { value: 'cron', label: t('triggers.kind.cron') },
      { value: 'interval', label: t('triggers.kind.interval') },
      { value: 'file_in_dir', label: t('triggers.kind.file_in_dir') },
      { value: 'manual', label: t('triggers.kind.manual') },
    ]}
  />
  {#if kind === 'cron'}
    <TextField
      label={t('triggers.cron')}
      bind:value={expr}
      error={preview && !preview.valid ? (preview.error ?? '') : undefined}
    />
    {#if preview?.valid}
      <div class="preview">
        <h4 id="trg-preview">{t('triggers.preview')}</h4>
        <ul aria-labelledby="trg-preview">
          {#each preview.next as at (at)}<li>{app.i18n.dateTime(at)}</li>{/each}
        </ul>
      </div>
    {/if}
  {:else if kind === 'interval'}
    <TextField label={t('triggers.interval')} type="number" min="1" bind:value={every} />
  {:else if kind === 'file_in_dir'}
    <TextField label={t('triggers.dir')} bind:value={dir} />
    <TextField label={t('triggers.pattern')} bind:value={pattern} />
    <p class="note">{t('triggers.watchUnavailable')}</p>
  {/if}
  <TextField label={t('triggers.goal')} bind:value={goal} />
  <Select
    label={t('triggers.agent')}
    size="sm"
    bind:value={agent}
    options={[
      { value: '', label: t('triggers.agentAny') },
      ...Object.values(AGENTS).map((a) => ({ value: a.id, label: a.name })),
    ]}
  />
  <Checkbox label={t('triggers.dnd')} bind:checked={dnd} />
  <div>
    <Button type="submit" disabled={!name.trim() || !goal.trim()}>{t('triggers.create')}</Button>
  </div>
</form>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  h4 {
    font-size: var(--alfa-font-size-sm);
  }
  .preview ul {
    margin: var(--alfa-space-1) 0 0;
    padding-left: var(--alfa-space-4);
    font-size: var(--alfa-font-size-sm);
  }
  .note {
    font-size: var(--alfa-font-size-xs);
    font-style: italic;
  }
  .duties {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
  }
  .duties ul {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .duties li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .hint {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
