<!--
  Ustawienia → Zadania w tle i wyzwalacze: lista (rodzaj, najbliższe uruchomienie, statystyki,
  włącznik, „Uruchom teraz", usuń), formularz nowego wyzwalacza i dziennik uruchomień.
-->
<script lang="ts">
  import { Button, EmptyState, Switch } from '@alfa/ui-kit';
  import Timer from '@lucide/svelte/icons/timer';
  import type { TriggerInfo, TriggerKindView, TriggerRunInfo } from '../../../api/types-tasks';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { attempt, load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import TriggerForm from './TriggerForm.svelte';

  const app = useApp();
  const { t, tk } = app.i18n;
  let triggers = $state<readonly TriggerInfo[]>([]);
  let log = $state<readonly TriggerRunInfo[]>([]);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);

  /** Lista i dziennik; błąd → komunikat z „Ponów" zamiast pustej listy. */
  async function reload() {
    const result = await load(() =>
      Promise.all([app.client.triggers.list(), app.client.triggers.log(null)]),
    );
    if (result.status === 'ready') {
      [triggers, log] = result.value;
      loaded = true;
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void reload();
    return app.on((event) => {
      if (event.type === 'TriggerFired') void reload();
    });
  });

  function describe(kind: TriggerKindView): string {
    switch (kind.kind) {
      case 'cron':
        return `${t('triggers.kind.cron')}: ${kind.expr}`;
      case 'interval':
        return t('triggers.every', { n: kind.every_minutes });
      case 'file_in_dir':
        return `${t('triggers.kind.file_in_dir')}: ${kind.dir}${kind.pattern ? ` (${kind.pattern})` : ''}`;
      default:
        return tk(`triggers.kind.${kind.kind}`);
    }
  }

  async function run(action: () => Promise<unknown>, message?: string) {
    if (!(await attempt(app.toasts, action))) {
      // Przełącznik sam zmienia swoje `checked`; nowe obiekty przywracają w nim stan rdzenia.
      triggers = triggers.map((x) => ({ ...x }));
      return;
    }
    if (message) app.toasts.show({ kind: 'success', message });
    await reload();
  }

  const nameOf = (id: string): string => triggers.find((x) => x.id === id)?.name ?? id;
</script>

<section class="card" aria-labelledby="trg-title">
  <h3 id="trg-title">{t('triggers.title')}</h3>
  <p class="desc">{t('triggers.intro')}</p>
  {#if loadError}
    <LoadFailed error={loadError} onretry={() => void reload()} />
  {/if}
  {#if loaded && triggers.length === 0}
    <EmptyState title={t('triggers.title')} description={t('triggers.empty')}>
      {#snippet icon()}<Timer size={20} strokeWidth={1.5} />{/snippet}
    </EmptyState>
  {:else}
    <ul class="list" aria-label={t('triggers.list')}>
      {#each triggers as trg (trg.id)}
        <li class="item">
          <div class="head">
            <span class="name" id={`trg-${trg.id}`}>{trg.name}</span>
            <Switch
              checked={trg.enabled}
              label={`${t('triggers.enabled')}: ${trg.name}`}
              onchange={(on) => run(() => app.client.triggers.setEnabled(trg.id, on))}
            />
          </div>
          <p class="meta">
            {describe(trg.kind)}{#if trg.next_fire_at}
              · {t('triggers.next', { when: app.i18n.dateTime(trg.next_fire_at) })}{/if}
            · {t('triggers.stats', { fired: trg.fired, suppressed: trg.suppressed })}
          </p>
          <p class="goal">{trg.goal}</p>
          {#if trg.watch_unavailable}<p class="note">{t('triggers.watchUnavailable')}</p>{/if}
          <div class="actions">
            <Button
              size="sm"
              variant="secondary"
              aria-describedby={`trg-${trg.id}`}
              onclick={() =>
                run(async () => {
                  const r = await app.client.triggers.fireNow(trg.id);
                  app.toasts.show({
                    kind: 'success',
                    message: t('triggers.fired', { task: r.task_id ?? '—' }),
                  });
                })}>{t('triggers.fire')}</Button
            >
            <Button
              size="sm"
              variant="ghost"
              aria-describedby={`trg-${trg.id}`}
              onclick={() => run(() => app.client.triggers.remove(trg.id), t('triggers.removed'))}
              >{t('triggers.remove')}</Button
            >
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<section class="card">
  <TriggerForm oncreated={reload} />
</section>

<section class="card" aria-labelledby="trg-log">
  <h3 id="trg-log">{t('triggers.log')}</h3>
  {#if loaded && log.length === 0}
    <p class="meta">{t('triggers.logEmpty')}</p>
  {:else if log.length}
    <ul class="log">
      {#each [...log].reverse().slice(0, 30) as r, i (`${r.at}-${r.trigger_id}-${i}`)}
        <li>
          <span class="meta">{app.i18n.dateTime(r.at)}</span>
          {nameOf(r.trigger_id)} — {r.cause} → {tk(`triggers.outcome.${r.outcome}`)}{#if r.task_id}
            ({r.task_id}){/if}{#if r.detail}: {r.detail}{/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  .desc,
  .goal {
    font-size: var(--alfa-font-size-sm);
  }
  .list,
  .log {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-2) 0;
    border-bottom: 1px solid var(--alfa-color-border);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .note {
    font-size: var(--alfa-font-size-xs);
    font-style: italic;
  }
  .actions {
    display: flex;
    gap: var(--alfa-space-2);
  }
  .log li {
    font-size: var(--alfa-font-size-sm);
  }
</style>
