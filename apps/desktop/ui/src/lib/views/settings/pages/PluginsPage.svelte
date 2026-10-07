<!--
  Ustawienia → Wtyczki (F8): wtyczki Wasm w piaskownicy (bez dostępu do systemu; pliki i sieć
  wyłącznie przez Brokera w imieniu agentki). Dodanie = manifest JSON + moduł `.wasm` → kontrola
  modułu → propozycja; instalacja dopiero po przejrzeniu karty (zdolności, limity, hash)
  i kliknięciu właściciela. Lista stanów, wyłączanie, ponowne włączenie (znów z hashem),
  usuwanie i ostatnie problemy (przerwania piaskownicy, odrzucone moduły — też w „Zdrowiu systemu”).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { PluginInfo, PluginInspection, PluginsView } from '../../../api/types-plugins';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { useApp } from '../../../state/context';
  import PluginReview from './PluginReview.svelte';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let view = $state<PluginsView | null>(null);
  let loadError = $state<string | null>(null);
  let reviewing = $state<{ plugin: PluginInfo; mode: 'install' | 'enable' } | null>(null);
  let manifest = $state('');
  let manifestError = $state<string | null>(null);
  let wasm = $state<{ name: string; b64: string } | null>(null);
  let inspection = $state<PluginInspection | null>(null);
  const plugins = $derived(view?.plugins ?? []);
  const pending = $derived(plugins.filter((p) => p.state === 'proposed'));
  const active = $derived(plugins.filter((p) => p.state === 'installed' || p.state === 'disabled'));
  const other = $derived(plugins.filter((p) => !pending.includes(p) && !active.includes(p)));

  /** Lista wtyczek; błąd → „Nie udało się wczytać" z „Ponów" (nie mylące „brak wtyczek"). */
  async function load() {
    try {
      view = await app.client.plugins.list();
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  async function run(action: () => Promise<unknown>, message?: string) {
    try {
      await action();
      if (message) app.toasts.show({ kind: 'success', message });
    } catch (e) {
      app.toasts.show({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
    await load();
  }

  function toBase64(buffer: ArrayBuffer): string {
    const bytes = new Uint8Array(buffer);
    let binary = '';
    for (let i = 0; i < bytes.length; i += 0x8000) {
      binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    }
    return btoa(binary);
  }

  async function pickManifest(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (file) manifest = await file.text();
  }

  async function pickModule(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    inspection = null;
    wasm = null;
    if (!file) return;
    const b64 = toBase64(await file.arrayBuffer());
    wasm = { name: file.name, b64 };
    await run(async () => {
      inspection = await app.client.plugins.inspect(b64);
    });
  }

  async function propose() {
    manifestError = null;
    let parsed: unknown;
    try {
      parsed = JSON.parse(manifest);
    } catch {
      manifestError = t('plugins.manifestInvalid');
      return;
    }
    const module = wasm;
    if (!module) return;
    await run(async () => {
      const p = await app.client.plugins.propose(parsed, module.b64);
      manifest = '';
      wasm = null;
      inspection = null;
      reviewing = { plugin: p, mode: 'install' };
    }, t('plugins.proposedOk'));
  }

  function label(p: PluginInfo): string {
    return `${p.id} ${p.version}`;
  }
</script>

<section class="wk-card" aria-labelledby="pl-title">
  <h3 id="pl-title">{t('plugins.title')}</h3>
  <p>{t('plugins.intro')}</p>
  {#if view && !view.available}
    <p class="wk-warn" role="status">
      {t('plugins.unavailable', { reason: view.unavailable_reason ?? '' })}
    </p>
  {/if}
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />{/if}
</section>

{#if reviewing}
  <PluginReview
    plugin={reviewing.plugin}
    mode={reviewing.mode}
    ondone={() => {
      reviewing = null;
      void load();
    }}
  />
{/if}

<section class="wk-card" aria-labelledby="pl-pending">
  <h3 id="pl-pending">{t('plugins.pending')}</h3>
  {#if view && pending.length === 0}
    <p class="wk-meta">{t('plugins.pendingEmpty')}</p>
  {:else if pending.length}
    <ul class="wk-list">
      {#each pending as p (`${p.id}@${p.version}`)}
        <li>
          <strong>{label(p)}</strong>
          <span class="wk-meta"
            >{tk(`plugins.origin.${p.origin}`)} · {app.i18n.dateTime(p.proposed_at)}</span
          >
          <span>{p.description}</span>
          {#if p.side_effects}<span class="wk-warn">{t('plugins.sideEffects')}</span>{/if}
          <div class="wk-actions">
            <Button
              size="sm"
              variant="primary"
              aria-label={`${t('plugins.review')}: ${label(p)}`}
              onclick={() => (reviewing = { plugin: p, mode: 'install' })}
              >{t('plugins.review')}</Button
            >
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<section class="wk-card" aria-labelledby="pl-active">
  <h3 id="pl-active">{t('plugins.installed')}</h3>
  {#if view && active.length === 0}
    <p class="wk-meta">{t('plugins.installedEmpty')}</p>
  {:else if active.length}
    <ul class="wk-list">
      {#each active as p (`${p.id}@${p.version}`)}
        <li>
          <strong>{label(p)}</strong>
          <span class="wk-meta"
            >{tk(`plugins.state.${p.state}`)} · {p.tools.map((x) => x.name).join(', ')}</span
          >
          <span>{p.description}</span>
          <div class="wk-actions">
            {#if p.state === 'installed'}
              <Button
                size="sm"
                variant="secondary"
                aria-label={`${t('plugins.disable')}: ${label(p)}`}
                onclick={() => run(() => app.client.plugins.disable(p.id), t('plugins.disabledOk'))}
                >{t('plugins.disable')}</Button
              >
            {:else}
              <Button
                size="sm"
                variant="secondary"
                aria-label={`${t('plugins.enableReview')}: ${label(p)}`}
                onclick={() => (reviewing = { plugin: p, mode: 'enable' })}
                >{t('plugins.enableReview')}</Button
              >
            {/if}
            <Button
              size="sm"
              variant="ghost"
              aria-label={`${t('plugins.remove')}: ${label(p)}`}
              onclick={() => run(() => app.client.plugins.remove(p.id), t('plugins.removedOk'))}
              >{t('plugins.remove')}</Button
            >
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if other.length}
  <section class="wk-card" aria-labelledby="pl-other">
    <h3 id="pl-other">{t('plugins.other')}</h3>
    <ul class="wk-plain wk-meta">
      {#each other as p (`${p.id}@${p.version}`)}
        <li>{label(p)} — {tk(`plugins.state.${p.state}`)}</li>
      {/each}
    </ul>
  </section>
{/if}

{#if view?.problems.length}
  <section class="wk-card" aria-labelledby="pl-problems">
    <h3 id="pl-problems">{t('plugins.problems')}</h3>
    <ul class="wk-plain">
      {#each view.problems as pr, i (i)}
        <li>
          <span class="wk-error">{tk(`plugins.problem.${pr.kind}`)}</span>
          <strong>{pr.plugin} {pr.version}</strong>
          <span class="wk-meta">{pr.detail} · {app.i18n.dateTime(pr.at)}</span>
        </li>
      {/each}
    </ul>
    <p class="wk-meta">{t('plugins.problemsNote')}</p>
  </section>
{/if}

<section class="wk-card" aria-labelledby="pl-add">
  <h3 id="pl-add">{t('plugins.add')}</h3>
  <p class="wk-meta">{t('plugins.addIntro')}</p>
  <label class="wk-field">
    <span>{t('plugins.moduleFile')}</span>
    <input type="file" accept=".wasm,application/wasm" onchange={pickModule} />
  </label>
  {#if inspection}
    <p class={inspection.ok ? 'wk-ok' : 'wk-error'} role="status">
      {inspection.ok
        ? t('plugins.inspectOk', { size: app.i18n.bytes(inspection.bytes) })
        : t('plugins.inspectFailed', { error: inspection.error ?? '' })}
    </p>
    <p class="wk-meta">
      {t('plugins.wasmHash')}: <code class="wk-code">{inspection.wasm_sha256}</code>
    </p>
  {/if}
  <label class="wk-field">
    <span>{t('plugins.manifestFile')}</span>
    <input type="file" accept=".json,application/json" onchange={pickManifest} />
  </label>
  <label class="wk-field">
    <span>{t('plugins.manifest')}</span>
    <textarea
      rows="6"
      spellcheck="false"
      bind:value={manifest}
      aria-invalid={manifestError ? 'true' : undefined}></textarea>
    {#if manifestError}<span class="wk-error" role="alert">{manifestError}</span>{/if}
  </label>
  <div class="wk-actions">
    <Button
      size="sm"
      variant="secondary"
      disabled={!manifest.trim() || !wasm || inspection?.ok === false}
      onclick={propose}>{t('plugins.propose')}</Button
    >
  </div>
</section>
