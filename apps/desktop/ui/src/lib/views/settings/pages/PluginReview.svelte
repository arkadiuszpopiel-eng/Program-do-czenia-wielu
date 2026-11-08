<!--
  Karta zatwierdzenia wtyczki Wasm: co wtyczka może (zdolności z zakresami), limity piaskownicy,
  narzędzia widziane przez agentki, hash przejrzanej wersji i hash modułu, propozycja R2.
  „Zainstaluj” / „Włącz ponownie” wysyła dokładnie ten hash — rdzeń odmówi, jeśli treść zmieniła
  się od podglądu. Zatwierdza wyłącznie właściciel gestem w oknie (zdarzenie zaufane).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { PluginInfo } from '../../../api/types-plugins';
  import { useApp } from '../../../state/context';

  interface Props {
    plugin: PluginInfo;
    /** `install` — propozycja; `enable` — ponowne włączenie wyłączonej wersji. */
    mode: 'install' | 'enable';
    ondone: () => void;
  }

  let { plugin, mode, ondone }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let busy = $state(false);
  const titleId = $derived(`pl-review-${plugin.id}`);

  async function decide(event: MouseEvent, kind: 'approve' | 'reject') {
    // Tylko gest właściciela w oknie — zdarzenia syntetyczne (skrypt strony) są ignorowane.
    if (!event.isTrusted || busy) return;
    busy = true;
    try {
      if (kind === 'reject') {
        await app.client.plugins.reject(plugin.id, plugin.version);
        app.toasts.show({ kind: 'info', message: t('plugins.rejectedOk') });
      } else if (mode === 'enable') {
        await app.client.plugins.enable(plugin.id, plugin.review_hash);
        app.toasts.show({ kind: 'success', message: t('plugins.enabledOk') });
      } else {
        await app.client.plugins.approve(plugin.id, plugin.version, plugin.review_hash);
        app.toasts.show({ kind: 'success', message: t('plugins.installedOk') });
      }
      ondone();
    } catch (e) {
      app.toasts.show({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    } finally {
      busy = false;
    }
  }
</script>

<section class="wk-card wk-accent" aria-labelledby={titleId}>
  <h3 id={titleId}>
    {t(mode === 'enable' ? 'plugins.enableTitle' : 'plugins.reviewTitle', {
      name: plugin.id,
      version: plugin.version,
    })}
  </h3>
  <p>{plugin.description}</p>
  <p class="wk-meta">
    {t('plugins.author', { author: plugin.author })} · {tk(`plugins.origin.${plugin.origin}`)}
  </p>
  {#if plugin.side_effects}<p class="wk-warn">{t('plugins.sideEffects')}</p>{/if}

  <h4>{t('plugins.capabilities')}</h4>
  {#if plugin.capabilities.length === 0}
    <p class="wk-meta">{t('plugins.noCapabilities')}</p>
  {:else}
    <ul class="wk-plain">
      {#each plugin.capabilities as c (`${c.family}:${c.scope}`)}
        <li>
          <strong>{tk(`plugins.cap.${c.family}`)}</strong>
          <code class="wk-code">{c.scope}</code>
        </li>
      {/each}
    </ul>
    <p class="wk-meta">{t('plugins.capabilitiesNote')}</p>
  {/if}

  <h4>{t('plugins.limits')}</h4>
  <dl class="wk-grid">
    <div>
      <dt class="wk-meta">{t('plugins.limit.memory')}</dt>
      <dd>{t('plugins.limit.mib', { n: plugin.limits.memory_mib })}</dd>
    </div>
    <div>
      <dt class="wk-meta">{t('plugins.limit.time')}</dt>
      <dd>{t('plugins.limit.ms', { n: plugin.limits.wall_ms })}</dd>
    </div>
    <div>
      <dt class="wk-meta">{t('plugins.limit.fuel')}</dt>
      <dd>{app.i18n.int(plugin.limits.fuel_per_call)}</dd>
    </div>
    <div>
      <dt class="wk-meta">{t('plugins.limit.io')}</dt>
      <dd>
        {app.i18n.bytes(plugin.limits.max_input_bytes)} / {app.i18n.bytes(
          plugin.limits.max_output_bytes,
        )}
      </dd>
    </div>
    <div>
      <dt class="wk-meta">{t('plugins.limit.hostCalls')}</dt>
      <dd>{plugin.limits.max_host_calls}</dd>
    </div>
  </dl>

  <h4>{t('plugins.tools')}</h4>
  <ul class="wk-plain">
    {#each plugin.tools as tool (tool.name)}
      <li>
        <strong>{tool.title}</strong> <code class="wk-code">{tool.name}</code>
        {#if tool.mutating}<span class="wk-warn">· {t('plugins.mutating')}</span>{/if}
        <br /><span class="wk-meta">{tool.description}</span>
      </li>
    {/each}
  </ul>

  {#if plugin.r2}
    <h4>{t('plugins.r2')}</h4>
    <p class="wk-meta">
      {t('plugins.r2Note', { key: plugin.r2.key })}
      {#if plugin.r2.from_version}{t('plugins.r2From', { version: plugin.r2.from_version })}{/if}
      {#if plugin.r2.added_capabilities.length}{t('plugins.r2Added', {
          caps: plugin.r2.added_capabilities.join(', '),
        })}{/if}
    </p>
  {/if}

  <p class="wk-meta">
    {t('plugins.reviewHash')}:
    <code class="wk-code" data-testid="review-hash">{plugin.review_hash}</code>
  </p>
  <p class="wk-meta">{t('plugins.wasmHash')}: <code class="wk-code">{plugin.wasm_sha256}</code></p>
  <p class="wk-note">{t('plugins.approveNote')}</p>
  <div class="wk-actions">
    <Button size="sm" variant="primary" disabled={busy} onclick={(e) => decide(e, 'approve')}
      >{mode === 'enable' ? t('plugins.enable') : t('plugins.install')}</Button
    >
    {#if mode === 'install'}
      <Button size="sm" variant="secondary" disabled={busy} onclick={(e) => decide(e, 'reject')}
        >{t('plugins.reject')}</Button
      >
    {/if}
    <Button size="sm" variant="ghost" onclick={ondone}>{t('common.cancel')}</Button>
  </div>
</section>
