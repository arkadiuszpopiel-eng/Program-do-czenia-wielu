<!--
  Dziennik rodzica (plan 50 §4.4, Z.7; ADR 0029 pkt 2): po PIN-ie rodzica — rozmowy (temat, czas)
  i działania agentek z osi czasu (narzędzia, Audyt: zgody, odmowy, blokady, kłódka). Treść
  rozmów nie jest pokazywana (decyzja właściciela 3.11: włącza ją rodzic jawnie — osobny krok).
  PIN sprawdza Sędzia Cieni z tym samym limitem prób; dziennik znika po „Ukryj” i po wyjściu.
-->
<script lang="ts">
  import { Button, TextField } from '@alfa/ui-kit';
  import type { TimelineEvent, TimelineKind } from '../../../api/types';
  import { showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  /** Najwięcej rozmów i działań na rozmowę w dzienniku. */
  const SESSIONS = 20;
  const EVENTS = 30;
  const KINDS: TimelineKind[] = ['tool', 'audit'];
  const app = useApp();
  const { t } = app.i18n;

  interface Entry {
    id: string;
    title: string;
    when: string;
    events: TimelineEvent[];
  }
  let pin = $state('');
  let busy = $state(false);
  let failed = $state(false);
  let log = $state<Entry[] | null>(null);

  async function load() {
    const sessions = [...(await app.client.sessions.list())]
      .sort((a, b) => b.updated_at.localeCompare(a.updated_at))
      .slice(0, SESSIONS);
    log = await Promise.all(
      sessions.map(async (s) => {
        const events = await app.client.timeline.list(s.id, { kinds: KINDS, min_level: 'info' });
        return {
          id: s.id,
          title: s.title,
          when: `${app.i18n.day(s.updated_at)}, ${app.i18n.time(s.updated_at)}`,
          events: events.slice(-EVENTS).reverse(),
        };
      }),
    );
  }

  async function show(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    failed = false;
    const given = pin;
    pin = '';
    try {
      await app.client.permissions.childLock({ op: 'verify', pin: given });
    } catch (error) {
      showError(app.toasts, error);
      busy = false;
      return;
    }
    try {
      await load();
    } catch {
      failed = true;
    } finally {
      busy = false;
    }
  }
</script>

<section class="card" aria-labelledby="parent-log-title">
  <h3 id="parent-log-title">{t('parentLog.title')}</h3>
  <p class="desc">{t('parentLog.intro')}</p>
  {#if log === null}
    <form class="row" onsubmit={show}>
      <TextField
        label={t('child.pin')}
        type="password"
        inputmode="numeric"
        maxlength={8}
        bind:value={pin}
      />
      <Button type="submit" disabled={busy || !pin}>{t('parentLog.show')}</Button>
    </form>
    {#if busy}<p class="desc" role="status">{t('parentLog.loading')}</p>{/if}
    {#if failed}<p class="desc" role="alert">{t('parentLog.failed')}</p>{/if}
  {:else}
    <div>
      <Button onclick={() => (log = null)}>{t('parentLog.hide')}</Button>
    </div>
    {#if log.length === 0}
      <p class="desc">{t('parentLog.empty')}</p>
    {:else}
      <ul class="sessions" aria-label={t('parentLog.title')}>
        {#each log as s (s.id)}
          <li>
            <h4>{t('parentLog.session', { title: s.title, when: s.when })}</h4>
            {#if s.events.length === 0}
              <p class="desc">{t('parentLog.none')}</p>
            {:else}
              <ul class="events">
                {#each s.events as e (e.id)}
                  <li>
                    <time datetime={e.ts}>{app.i18n.time(e.ts)}</time>
                    {#if e.agent}<span class="agent">{e.agent}</span>{/if}
                    <span>{e.title}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
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
  h4 {
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-3);
  }
  .sessions,
  .events {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .events {
    gap: var(--alfa-space-1);
    padding-left: var(--alfa-space-3);
    font-size: var(--alfa-font-size-sm);
  }
  .events li {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-2);
  }
  time,
  .agent {
    color: var(--alfa-color-text-muted);
    font-variant-numeric: tabular-nums;
  }
  .agent {
    text-transform: capitalize;
  }
</style>
