<!--
  Ustawienia → Kamery (plan v6, C3): przełącznik główny (zwykłe ustawienie `camera.enabled`), lista
  kamer z nazwą, stanem i własnym przełącznikiem, podgląd na żywo na żądanie. Kamera otwiera się
  dopiero, gdy ktoś prosi o klatkę (podgląd), i sama zasypia; program niczego nie nagrywa.
-->
<script lang="ts">
  import { Button, Chip, EmptyState, IconButton, Switch, TextField } from '@alfa/ui-kit';
  import Camera from '@lucide/svelte/icons/camera';
  import Check from '@lucide/svelte/icons/check';
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';
  import Pencil from '@lucide/svelte/icons/pencil';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import { errorText } from '../../../api/command-error';
  import type { CameraFrame, CameraView } from '../../../api/types-cameras';
  import type { SettingDef } from '../../../api/types-system';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { useApp } from '../../../state/context';
  import SettingRow from '../SettingRow.svelte';

  interface Props {
    /** Ustawienia strony (przełącznik główny) — pokazywane nad listą kamer. */
    settings?: readonly SettingDef[];
  }
  let { settings = [] }: Props = $props();

  const app = useApp();
  const { t, tk } = app.i18n;
  /** Odstęp między klatkami podglądu (ms) i rozmiar klatki. */
  const PREVIEW_MS = 400;
  const PREVIEW_SIDE = 480;

  let failed = $state<string | null>(null);
  let watching = $state<string[]>([]);
  let frames = $state<Record<string, CameraFrame>>({});
  let errors = $state<Record<string, string>>({});
  let renaming = $state<string | null>(null);
  let draft = $state('');
  let searching = $state(false);

  const hub = $derived(app.cameras);
  const master = $derived(app.bool('camera.enabled', false));

  async function load() {
    failed = null;
    try {
      app.cameras = await app.client.cameras.state();
    } catch (error) {
      failed = errorText(error);
    }
  }

  // Przełącznik główny to zwykłe ustawienie — po jego zmianie odświeżamy stan kamer.
  $effect(() => {
    void master;
    void load();
  });

  // Podgląd: klatki tylko dla kamer, które użytkownik ogląda, i tylko gdy kamery są włączone.
  $effect(() => {
    if (!master || watching.length === 0) return;
    const ids = [...watching];
    let stopped = false;
    const tick = async () => {
      for (const id of ids) {
        try {
          const frame = await app.client.cameras.frame(id, PREVIEW_SIDE);
          if (stopped) return;
          frames = { ...frames, [id]: frame };
          if (errors[id]) errors = { ...errors, [id]: '' };
        } catch (error) {
          if (stopped) return;
          errors = { ...errors, [id]: errorText(error) };
          // Błąd (kamera zatrzymana, odłączona, STOP) kończy podgląd tej kamery — kolejne
          // żądania budziłyby ją na nowo wbrew zatrzymaniu.
          watching = watching.filter((other) => other !== id);
        }
      }
      if (!stopped) timer = window.setTimeout(() => void tick(), PREVIEW_MS);
    };
    let timer = window.setTimeout(() => void tick(), 0);
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  });

  // Wyłączenie kamer albo jednej kamery kończy jej podgląd.
  $effect(() => {
    const off = (hub?.cameras ?? []).filter((c) => !c.enabled || c.state === 'unplugged');
    if (!master && watching.length > 0) watching = [];
    else if (off.some((c) => watching.includes(c.id)))
      watching = watching.filter((id) => !off.some((c) => c.id === id));
  });

  function toggleWatch(id: string) {
    watching = watching.includes(id) ? watching.filter((other) => other !== id) : [...watching, id];
    if (errors[id]) errors = { ...errors, [id]: '' };
    if (!watching.includes(id)) {
      const { [id]: _gone, ...rest } = frames;
      frames = rest;
    }
  }

  async function act(action: () => Promise<unknown>) {
    try {
      await action();
    } catch (error) {
      app.toasts.show({ kind: 'error', message: errorText(error) });
    }
  }

  async function search() {
    searching = true;
    await act(async () => (app.cameras = await app.client.cameras.refresh()));
    searching = false;
  }

  function startRename(camera: CameraView) {
    renaming = camera.id;
    draft = camera.name;
  }

  async function saveName(id: string) {
    const name = draft.trim();
    renaming = null;
    if (name) await act(async () => (app.cameras = await app.client.cameras.rename(id, name)));
  }

  function nameKey(event: KeyboardEvent, id: string) {
    if (event.key === 'Enter') {
      event.preventDefault();
      void saveName(id);
    } else if (event.key === 'Escape') {
      // Escape anuluje zmianę nazwy, a nie zamyka Ustawień.
      event.preventDefault();
      event.stopPropagation();
      renaming = null;
    }
  }
</script>

<p class="lead">{t('cam.lead')}</p>
{#each settings as def (def.key)}<SettingRow {def} />{/each}

<section class="list" aria-labelledby="cam-list">
  <div class="head">
    <h3 id="cam-list">{t('cam.list')}</h3>
    <Button size="sm" variant="secondary" disabled={searching} onclick={() => void search()}>
      {#snippet icon()}<RefreshCw size={14} strokeWidth={1.5} />{/snippet}
      {t('cam.search')}
    </Button>
  </div>
  {#if failed}
    <LoadFailed error={failed} onretry={() => void load()} />
  {:else if !hub}
    <Loading label={t('cam.loading')} />
  {:else if hub.cameras.length === 0}
    <EmptyState title={t('cam.none')} description={t('cam.noneHint')}>
      {#snippet icon()}<Camera size={20} strokeWidth={1.5} />{/snippet}
    </EmptyState>
  {:else}
    {#if !master}<p class="off-note" role="status">{t('cam.masterOff')}</p>{/if}
    <ul class="cameras">
      {#each hub.cameras as camera (camera.id)}
        {@const usable = master && camera.enabled && camera.state !== 'unplugged'}
        {@const live = watching.includes(camera.id)}
        {@const frame = frames[camera.id]}
        <li class="camera" class:dim={!usable} data-camera={camera.id}>
          <div class="view" class:live>
            {#if live && frame}
              <img
                src="data:image/png;base64,{frame.png_base64}"
                alt={t('cam.previewAlt', { name: camera.name })}
                width={frame.width}
                height={frame.height}
              />
            {:else}
              <span class="placeholder" aria-hidden="true">
                <Camera size={28} strokeWidth={1.25} />
              </span>
            {/if}
            {#if live}<span class="rec" aria-hidden="true"></span>{/if}
          </div>
          <div class="info">
            <div class="title">
              {#if renaming === camera.id}
                <TextField
                  aria-label={t('cam.nameLabel')}
                  bind:value={draft}
                  onkeydown={(event: KeyboardEvent) => nameKey(event, camera.id)}
                />
                <IconButton
                  label={t('cam.saveName')}
                  size="sm"
                  onclick={() => void saveName(camera.id)}
                >
                  <Check size={15} strokeWidth={1.5} />
                </IconButton>
              {:else}
                <span class="name">{camera.name}</span>
                <IconButton
                  label={t('cam.rename', { name: camera.name })}
                  size="sm"
                  onclick={() => startRename(camera)}
                >
                  <Pencil size={14} strokeWidth={1.5} />
                </IconButton>
              {/if}
            </div>
            <div class="chips">
              <Chip>{tk(`cam.kind.${camera.kind}`)}</Chip>
              <span class="state {camera.state}">{tk(`cam.state.${camera.state}`)}</span>
              {#if camera.state === 'live'}
                <span class="muted">{camera.width} × {camera.height}</span>
              {/if}
            </div>
            {#if camera.state === 'failed' && camera.detail}
              <p class="problem" role="alert">{camera.detail}</p>
            {/if}
            {#if errors[camera.id]}
              <p class="problem" role="alert">{errors[camera.id]}</p>
            {/if}
            <div class="actions">
              <Button
                size="sm"
                variant="secondary"
                disabled={!usable}
                onclick={() => toggleWatch(camera.id)}
              >
                {#snippet icon()}
                  {#if live}<EyeOff size={14} strokeWidth={1.5} />{:else}<Eye
                      size={14}
                      strokeWidth={1.5}
                    />{/if}
                {/snippet}
                {live ? t('cam.stopPreview') : t('cam.preview')}
              </Button>
              <label class="switch">
                <Switch
                  checked={camera.enabled}
                  label={t('cam.enable', { name: camera.name })}
                  disabled={camera.state === 'unplugged'}
                  onchange={(enabled) =>
                    void act(
                      async () =>
                        (app.cameras = await app.client.cameras.setEnabled(camera.id, enabled)),
                    )}
                />
                <span>{camera.enabled ? t('cam.on') : t('cam.off')}</span>
              </label>
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<section class="notes" aria-labelledby="cam-privacy">
  <h3 id="cam-privacy">{t('cam.privacy')}</h3>
  <ul>
    <li>{t('cam.privacy.record')}</li>
    <li>{t('cam.privacy.sleep')}</li>
    <li>{t('cam.privacy.indicator')}</li>
    <li>{t('cam.privacy.next')}</li>
  </ul>
</section>

<style>
  .lead {
    margin: 0 0 var(--alfa-space-2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .list,
  .notes {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-top: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-3);
  }
  h3 {
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  .off-note {
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px dashed var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-control);
    color: var(--alfa-color-text-muted);
  }
  .cameras {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: var(--alfa-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .camera {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-bg);
  }
  .view {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    aspect-ratio: 16 / 9;
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text-muted);
  }
  .view img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .dim .view {
    opacity: 0.6;
  }
  /* Nazwa niedostępnej kamery: kolor przygaszony, nie przezroczystość (czytelność). */
  .dim .name {
    color: var(--alfa-color-text-muted);
  }
  /* Znak „podgląd działa” w rogu obrazu. */
  .rec {
    position: absolute;
    top: var(--alfa-space-2);
    right: var(--alfa-space-2);
    width: 10px;
    height: 10px;
    border: 2px solid var(--alfa-color-bg);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-error);
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    min-height: 32px;
  }
  .title :global(.field) {
    flex: 1;
    min-width: 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    font-weight: var(--alfa-weight-semibold);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .state {
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
    line-height: 20px;
  }
  .state.live {
    border-color: var(--alfa-color-success);
  }
  .state.failed,
  .state.unplugged {
    border-color: var(--alfa-color-warning);
  }
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-variant-numeric: tabular-nums;
  }
  .problem {
    margin: 0;
    color: var(--alfa-color-error);
    font-size: var(--alfa-font-size-xs);
  }
  .actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .switch {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .notes ul {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: 0;
    padding-left: var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
  }
</style>
