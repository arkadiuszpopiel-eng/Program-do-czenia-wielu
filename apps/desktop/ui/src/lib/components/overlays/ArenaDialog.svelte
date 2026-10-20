<!--
  „Porównaj modele (Arena)” (paleta `Ctrl+K`, menu wiadomości; P4): wybór dwóch modeli (lokalne
  i z kont API — lista Routera), dwie odpowiedzi na tę samą wiadomość jako nowe gałęzie rozmowy
  (append-only; rdzeń prowadzi jedną generację na sesję, więc druga startuje po pierwszej), obok
  siebie z czasem, tokenami i kosztem. „Wybierz tę odpowiedź” ustawia gałąź jako bieżącą ścieżkę.
  Bez automatycznej oceny przez model. Zamknięcie okna nie startuje kolejnych odpowiedzi.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button, Chip, SanitizedHtml, Select } from '@alfa/ui-kit';
  import { errorText } from '../../api/command-error';
  import type { Turn } from '../../api/types';
  import type { RoutingModel } from '../../api/types-hub';
  import {
    arenaBest,
    arenaMetrics,
    arenaStep,
    arenaTarget,
    defaultPair,
    modelLabel,
    type ArenaSlot,
  } from '../../logic/arena';
  import { arenaDialog, closeArena } from '../../state/arena.svelte';
  import { attempt, load, type Loadable } from '../../state/attempt';
  import { useApp } from '../../state/context';

  type SlotStatus = 'waiting' | 'streaming' | 'complete' | 'cancelled' | 'error' | 'skipped';

  const app = useApp();
  const { t } = app.i18n;
  const uid = $props.id();
  // Rozmowa i wiadomość z chwili otwarcia — okno dotyczy jednej konkretnej wiadomości.
  const conv = app.conversation;
  const target = conv ? arenaTarget(conv.path, conv.tree.turns, arenaDialog.turnId) : undefined;

  let models = $state<Loadable<readonly RoutingModel[]>>({ status: 'loading' });
  let modelA = $state('');
  let modelB = $state('');
  let slots = $state<ArenaSlot[]>([]);
  let stopped = $state(false);
  let launching = false;

  const list = $derived(models.status === 'ready' ? models.value : []);
  const options = $derived(list.map((m) => ({ value: m.candidate, label: modelLabel(m) })));
  const same = $derived(modelA !== '' && modelA === modelB);
  const turnOf = (slot: ArenaSlot): Turn | undefined =>
    slot.turnId ? conv?.turn(slot.turnId) : undefined;
  const step = $derived(arenaStep(slots, (id) => conv?.turn(id)?.status, stopped));
  const running = $derived(slots.length > 0 && step.kind !== 'done');
  const metrics = $derived(slots.map((slot) => arenaMetrics(turnOf(slot), slot.model)));
  const best = $derived(
    step.kind === 'done' ? arenaBest(metrics) : { faster: null, cheaper: null },
  );

  $effect(() => {
    void load(() => app.client.accounts.routing().then((r) => r.models)).then((loaded) => {
      models = loaded;
      const pair = loaded.status === 'ready' ? defaultPair(loaded.value) : null;
      if (pair) [modelA, modelB] = pair;
    });
  });

  // Prowadzenie areny: kolejne stanowisko startuje, gdy poprzednie się skończyło.
  $effect(() => {
    if (!conv || !target || step.kind !== 'start' || launching) return;
    const slot = slots[step.index];
    if (!slot) return;
    launching = true;
    void conv.arenaAnswer(target.id, slot.model).then(
      (id) => {
        launching = false;
        slot.turnId = id;
      },
      (error: unknown) => {
        launching = false;
        slot.error = errorText(error);
      },
    );
  });

  function label(candidate: string): string {
    const model = list.find((m) => m.candidate === candidate);
    return model ? modelLabel(model) : candidate;
  }

  function start() {
    if (!modelA || !modelB || same) return;
    stopped = false;
    slots = [modelA, modelB].map((model) => ({ model, turnId: null, error: null }));
  }

  function stop() {
    stopped = true;
    if (conv) void conv.stop();
  }

  function statusOf(slot: ArenaSlot): SlotStatus {
    if (slot.error !== null) return 'error';
    if (slot.turnId === null) return stopped ? 'skipped' : 'waiting';
    const status = turnOf(slot)?.status ?? 'streaming';
    return status === 'queued' ? 'waiting' : status;
  }

  function choose(slot: ArenaSlot) {
    const id = slot.turnId;
    if (!conv || !id || !conv.selectTurnById(id)) return;
    closeArena();
    app.toasts.show({ kind: 'success', message: t('arena.chosen', { model: label(slot.model) }) });
    void attempt(app.toasts, () => app.client.turns.selectBranch(conv.sessionId, id));
  }
</script>

<Dialog.Root open onOpenChange={(open) => !open && closeArena()}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-arena-overlay" />
    <Dialog.Content class="alfa-arena">
      <Dialog.Title class="alfa-arena-title">{t('arena.title')}</Dialog.Title>
      <Dialog.Description class="alfa-arena-lead">{t('arena.lead')}</Dialog.Description>
      {#if !conv || !target}
        <p class="note">{t('arena.noTarget')}</p>
      {:else}
        <blockquote class="prompt" aria-label={t('arena.prompt')}>{target.text}</blockquote>
        {#if models.status === 'loading'}
          <p class="note">{t('arena.loading')}</p>
        {:else if models.status === 'failed'}
          <p class="problem" role="alert">{models.error}</p>
        {:else if slots.length === 0 && list.length < 2}
          <p class="note">{t('arena.fewModels')}</p>
        {:else if slots.length === 0}
          <div class="pickers">
            <div class="field">
              <span id="{uid}-a">{t('arena.modelA')}</span>
              <Select labelledby="{uid}-a" bind:value={modelA} {options} />
            </div>
            <div class="field">
              <span id="{uid}-b">{t('arena.modelB')}</span>
              <Select labelledby="{uid}-b" bind:value={modelB} {options} />
            </div>
          </div>
          {#if same}<p class="problem" role="alert">{t('arena.same')}</p>{/if}
          <p class="note">{t('arena.costNote')}</p>
        {:else}
          <div class="results" role="group" aria-label={t('arena.results')}>
            {#each slots as slot, i (slot.model)}
              {@const turn = turnOf(slot)}
              {@const status = statusOf(slot)}
              {@const m = metrics[i]}
              {@const name = label(slot.model)}
              <div class="card" role="group" aria-labelledby="{uid}-m{i}">
                <div class="head">
                  <h3 class="model" id="{uid}-m{i}">{name}</h3>
                  <span class="status" data-testid="arena-status" data-status={status}
                    >{t(`arena.status.${status}`)}</span
                  >
                  {#if best.faster === i}<Chip size="sm" tone="success">{t('arena.faster')}</Chip
                    >{/if}
                  {#if best.cheaper === i}<Chip size="sm" tone="success">{t('arena.cheaper')}</Chip
                    >{/if}
                </div>
                {#if status === 'error'}
                  <p class="problem" role="alert">{slot.error ?? turn?.error?.message ?? ''}</p>
                {/if}
                {#if turn && turn.blocks.length > 0}
                  <div
                    class="body"
                    tabindex="0"
                    role="region"
                    aria-label={t('arena.answerLabel', { model: name })}
                  >
                    {#each turn.blocks as block (block.index)}
                      <SanitizedHtml html_sanitized={block.html_sanitized} />
                    {/each}
                  </div>
                {/if}
                {#if m}
                  {#if m.fallback}
                    <p class="note">
                      {t('arena.fallback', { model: `${m.provider} · ${m.model}` })}
                    </p>
                  {/if}
                  <dl class="metrics">
                    <div>
                      <dt>{t('arena.latency')}</dt>
                      <dd>{app.i18n.duration(m.latencyMs)}</dd>
                    </div>
                    <div>
                      <dt>{t('arena.tokens')}</dt>
                      <dd>
                        {t('arena.tokensValue', {
                          input: app.i18n.int(m.inputTokens),
                          output: app.i18n.int(m.outputTokens),
                        })}
                      </dd>
                    </div>
                    <div>
                      <dt>{t('arena.cost')}</dt>
                      <dd>{m.cost.minor > 0 ? app.i18n.money(m.cost) : t('arena.free')}</dd>
                    </div>
                  </dl>
                {/if}
                <Button
                  variant="secondary"
                  disabled={status !== 'complete' || running}
                  aria-label={t('arena.chooseLabel', { model: name })}
                  onclick={() => choose(slot)}>{t('arena.choose')}</Button
                >
              </div>
            {/each}
          </div>
        {/if}
      {/if}
      <div class="footer">
        {#if running}
          <Button variant="secondary" onclick={stop}>{t('arena.stop')}</Button>
        {/if}
        {#if conv && target && slots.length === 0 && list.length >= 2}
          <Button variant="secondary" onclick={closeArena}>{t('arena.cancel')}</Button>
          <Button variant="primary" disabled={!modelA || !modelB || same} onclick={start}
            >{t('arena.start')}</Button
          >
        {:else}
          <Button variant="primary" onclick={closeArena}>{t('arena.close')}</Button>
        {/if}
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.alfa-arena-overlay) {
    position: fixed;
    inset: 0;
    z-index: 110;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-arena) {
    position: fixed;
    z-index: 111;
    top: 50%;
    left: 50%;
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    width: min(960px, calc(var(--alfa-vw, 100vw) - 32px));
    max-height: min(calc(var(--alfa-vh, 100vh) * 0.9), 860px);
    padding: var(--alfa-space-6);
    overflow: auto;
    transform: translate(-50%, -50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
  }
  :global(.alfa-arena-title) {
    margin: 0;
    font-size: var(--alfa-font-size-lg);
    font-weight: var(--alfa-weight-semibold);
  }
  :global(.alfa-arena-lead),
  .note {
    margin: 0;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .prompt {
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border-left: 3px solid var(--alfa-color-border-strong);
    max-height: 6lh;
    overflow: hidden;
    font-size: var(--alfa-font-size-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .pickers,
  .results {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: var(--alfa-space-3);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
  }
  .problem {
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid color-mix(in srgb, var(--alfa-color-error) 40%, var(--alfa-color-border));
    border-radius: var(--alfa-radius-card);
    font-size: var(--alfa-font-size-sm);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    min-width: 0;
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .model {
    flex: 1;
    min-width: 12ch;
    margin: 0;
    font-size: var(--alfa-font-size-md);
    font-weight: var(--alfa-weight-semibold);
    overflow-wrap: anywhere;
  }
  .status {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .body {
    max-height: 320px;
    overflow: auto;
    font-size: var(--alfa-font-size-sm);
    overflow-wrap: anywhere;
  }
  .metrics {
    display: grid;
    gap: var(--alfa-space-1);
    margin: 0;
    font-size: var(--alfa-font-size-sm);
  }
  .metrics div {
    display: flex;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  dt {
    color: var(--alfa-color-text-muted);
  }
  dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
  }
  .footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
</style>
