<!--
  Panel „Ekran" (computer use): kto steruje, „Zatrzymaj sterowanie" (anuluje akcje GUI i tury —
  fizyczne przejęcie) / „Oddaj sterowanie", ostatni zrzut agentki (zamaskowany w porcie: okna Alfy
  i Brokera, deny-lista, pola haseł; tylko w pamięci), ostatnie akcje (bez wpisywanej treści)
  i prośba o stały podgląd pulpitu (okno Brokera).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { GuiScreenshot } from '../../api/types-work';
  import { agentName } from '../../logic/work';
  import { attempt, load } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import LoadFailed from '../shell/LoadFailed.svelte';
  import DesktopGrant from './DesktopGrant.svelte';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const gui = $derived(app.work.gui);
  const shotAt = $derived(gui?.screenshot?.at ?? null);
  let shot = $state<GuiScreenshot | null>(null);
  let statusError = $state<string | null>(null);
  let shotError = $state<string | null>(null);

  // Błąd stanu = „Nie udało się wczytać" z „Ponów" zamiast „Ładowanie…" na zawsze.
  async function loadStatus() {
    statusError = null;
    const result = await load(() => app.client.gui.status());
    if (result.status === 'ready') app.work.applyGui(result.value);
    else if (result.status === 'failed') statusError = result.error;
  }

  async function loadShot(at: string) {
    shotError = null;
    const result = await load(() => app.client.gui.screenshot());
    if (at !== shotAt) return;
    if (result.status === 'ready') {
      shot = result.value;
    } else if (result.status === 'failed') {
      shot = null;
      shotError = result.error;
    }
  }

  function retryShot() {
    if (shotAt) void loadShot(shotAt);
  }

  $effect(() => {
    void loadStatus();
  });

  $effect(() => {
    const at = shotAt;
    if (at) {
      void loadShot(at);
      return;
    }
    shot = null;
    shotError = null;
  });

  async function act(action: () => Promise<unknown>) {
    await attempt(app.toasts, action);
  }
</script>

<div class="screen">
  {#if statusError}
    <LoadFailed error={statusError} onretry={() => void loadStatus()} />
  {/if}
  {#if !gui}
    {#if !statusError}<p class="muted">{t('common.loading')}</p>{/if}
  {:else}
    {#if !gui.available}
      <p class="note" role="status">
        {gui.reason ? app.i18n.text(gui.reason) : t('gui.unavailable')}
      </p>
    {/if}
    <section class="state" aria-labelledby="gui-state">
      <h3 id="gui-state">{t('gui.title')}</h3>
      {#if gui.taken_over}
        <p class="status taken" role="status">{t('gui.takenOverLong')}</p>
        <Button size="sm" variant="primary" onclick={() => act(() => app.client.gui.release())}
          >{t('gui.release')}</Button
        >
      {:else}
        <p class="status" class:live={gui.control} role="status">
          {gui.control
            ? t('gui.controllingTool', {
                name: agentName(gui.control.agent),
                tool: gui.control.tool,
              })
            : t('gui.idle')}
        </p>
        <Button size="sm" variant="danger" onclick={() => act(() => app.client.gui.stop())}
          >{t('gui.stop')}</Button
        >
        <p class="muted">{t('gui.stopHint')}</p>
      {/if}
    </section>

    <section aria-labelledby="gui-shot">
      <h3 id="gui-shot">{t('gui.shot')}</h3>
      {#if gui.screenshot && shot}
        {@const info = gui.screenshot}
        <img
          class="shot"
          src={shot.data_url}
          width={info.width}
          height={info.height}
          alt={t('gui.shotAlt', {
            name: agentName(info.agent),
            width: info.width,
            height: info.height,
            n: info.masked,
          })}
        />
        <p class="muted">
          {t('gui.shotMeta', { time: app.i18n.time(info.at), n: info.masked })}
        </p>
        {#if info.black_frame}<p class="note">{t('gui.blackFrame')}</p>{/if}
      {:else if gui.screenshot && shotError}
        <LoadFailed error={shotError} onretry={retryShot} />
      {:else}
        <p class="muted">{t('gui.noShot')}</p>
      {/if}
      <p class="muted">{t('gui.masking')}</p>
    </section>

    <section aria-labelledby="gui-actions">
      <h3 id="gui-actions">{t('gui.actions')}</h3>
      {#if gui.actions.length === 0}
        <p class="muted">{t('gui.noActions')}</p>
      {:else}
        <ol class="actions">
          {#each gui.actions as a (a.id)}
            <li>
              <span class="what">{a.summary}</span>
              <span class="meta">
                {agentName(a.agent)}{a.target ? ` · ${a.target}` : ''} · {app.i18n.time(a.at)}
                {#if a.duration_ms !== null}· {app.i18n.duration(a.duration_ms)}{/if}
              </span>
              <span class="badge badge-{a.status}">{tk(`gui.status.${a.status}`)}</span>
            </li>
          {/each}
        </ol>
      {/if}
    </section>

    <section aria-labelledby="gui-grant">
      <h3 id="gui-grant">{t('gui.grantTitle')}</h3>
      <DesktopGrant {sessionId} />
    </section>
  {/if}
</div>

<style>
  .screen {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-4);
    padding: var(--alfa-space-2);
    font-size: var(--alfa-font-size-sm);
  }
  section {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-2);
  }
  h3 {
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .status.live {
    color: var(--alfa-color-warning);
  }
  .status.taken {
    font-weight: var(--alfa-weight-semibold);
  }
  .shot {
    width: 100%;
    height: auto;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    width: 100%;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .actions li {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 2px var(--alfa-space-2);
  }
  .meta {
    grid-column: 1;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .badge {
    grid-row: 1 / span 2;
    grid-column: 2;
    align-self: center;
    font-size: var(--alfa-font-size-xs);
  }
  .badge-denied,
  .badge-failed {
    color: var(--alfa-color-error);
  }
  .badge-running {
    color: var(--alfa-color-warning);
  }
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .note {
    font-size: var(--alfa-font-size-xs);
    font-style: italic;
  }
</style>
