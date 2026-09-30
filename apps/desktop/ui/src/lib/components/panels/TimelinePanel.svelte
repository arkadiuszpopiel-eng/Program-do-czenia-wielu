<!-- Oś czasu v0 (makieta 7): zdarzenia sesji z filtrami (rodzaj, poziom), koszt i opóźnienie. -->
<script lang="ts">
  import { Avatar, Button, Chip, Select } from '@alfa/ui-kit';
  import Cpu from '@lucide/svelte/icons/cpu';
  import Wrench from '@lucide/svelte/icons/wrench';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import MousePointer from '@lucide/svelte/icons/mouse-pointer';
  import Mic from '@lucide/svelte/icons/mic';
  import Activity from '@lucide/svelte/icons/activity';
  import type { EventLevel, TimelineEvent, TimelineKind } from '../../api/types';
  import { useApp } from '../../state/context';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const KINDS: readonly TimelineKind[] = [
    'model_call',
    'tool',
    'audit',
    'ui',
    'voice',
    'diagnostics',
  ];
  const LEVELS: readonly EventLevel[] = ['trace', 'debug', 'info', 'warn', 'error', 'audit'];
  const ICONS = {
    model_call: Cpu,
    tool: Wrench,
    audit: ShieldCheck,
    ui: MousePointer,
    voice: Mic,
    diagnostics: Activity,
  };

  let kinds = $state<TimelineKind[]>([]);
  let minLevel = $state<string>('info');
  let events = $state<TimelineEvent[]>([]);

  $effect(() => {
    const filter = { kinds: [...kinds], min_level: minLevel as EventLevel };
    void app.client.timeline.list(sessionId, filter).then((list) => (events = [...list]));
  });

  $effect(() =>
    app.on((event) => {
      if (event.type !== 'TimelineAppended' || event.event.session_id !== sessionId) return;
      const e = event.event;
      const levelOk = LEVELS.indexOf(e.level) >= LEVELS.indexOf(minLevel as EventLevel);
      if ((kinds.length === 0 || kinds.includes(e.kind)) && levelOk) events = [...events, e];
    }),
  );

  const shown = $derived(
    app.timelineTurn ? events.filter((e) => e.turn_id === app.timelineTurn) : events,
  );
  const total = $derived(shown.reduce((sum, e) => sum + (e.cost?.minor ?? 0), 0));

  function toggle(kind: TimelineKind) {
    kinds = kinds.includes(kind) ? kinds.filter((k) => k !== kind) : [...kinds, kind];
  }
</script>

<div class="timeline">
  <div class="filters">
    <div class="kinds" role="group" aria-label={t('timeline.kind')}>
      {#each KINDS as kind (kind)}
        <Chip size="sm" selected={kinds.includes(kind)} onclick={() => toggle(kind)}
          >{t(`timeline.kind.${kind}`)}</Chip
        >
      {/each}
    </div>
    <label class="level">
      <span>{t('timeline.level')}</span>
      <Select
        bind:value={minLevel}
        size="sm"
        label={t('timeline.level')}
        options={LEVELS.map((l) => ({ value: l, label: t(`timeline.level.${l}`) }))}
      />
    </label>
  </div>
  {#if app.timelineTurn}
    <div class="turn-filter">
      <span>{t('timeline.forTurn')}</span>
      <Button size="sm" variant="ghost" onclick={() => (app.timelineTurn = null)}
        >{t('timeline.showAll')}</Button
      >
    </div>
  {/if}
  <p class="summary" aria-live="polite">
    {t('timeline.count', { n: shown.length })}{#if total > 0}
      · {t('timeline.total', { cost: app.i18n.money({ minor: total, currency: 'PLN' }) })}{/if}
  </p>
  {#if shown.length === 0}
    <p class="empty">{t('timeline.empty')}</p>
  {:else}
    <ol class="list" aria-label={t('timeline.list')}>
      {#each [...shown].reverse() as e (e.id)}
        {@const Icon = ICONS[e.kind]}
        <li class="event level-{e.level}">
          <span class="icon" aria-hidden="true"><Icon size={14} strokeWidth={1.5} /></span>
          <div class="text">
            <div class="line">
              <time datetime={e.ts} title={app.i18n.dateTime(e.ts)}>{app.i18n.time(e.ts)}</time>
              {#if e.agent}<Avatar agent={e.agent} size={20} />{:else}<span class="sys"
                  >{t('timeline.system')}</span
                >{/if}
              <span class="title">{e.title}</span>
            </div>
            {#if e.detail}<p class="detail">{e.detail}</p>{/if}
            <p class="meta">
              <span>{t(`timeline.kind.${e.kind}`)} · {t(`timeline.level.${e.level}`)}</span>
              {#if e.cost}<span>· {app.i18n.money(e.cost)}</span>{/if}
              {#if e.latency_ms !== null}<span>· {app.i18n.duration(e.latency_ms)}</span>{/if}
            </p>
          </div>
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .timeline {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .filters {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
  .level {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    font-size: var(--alfa-font-size-sm);
  }
  .turn-filter {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--alfa-space-1) var(--alfa-space-2);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
    font-size: var(--alfa-font-size-sm);
  }
  .summary,
  .empty {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .list {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .event {
    display: flex;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-2) 0;
    border-bottom: 1px solid var(--alfa-color-border);
  }
  .icon {
    display: inline-flex;
    padding-top: 2px;
    color: var(--alfa-color-text-muted);
  }
  .level-warn .icon {
    color: var(--alfa-color-warning);
  }
  .level-error .icon {
    color: var(--alfa-color-error);
  }
  .level-audit .icon {
    color: var(--alfa-color-info);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    font-size: var(--alfa-font-size-sm);
  }
  time,
  .sys {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail,
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
</style>
