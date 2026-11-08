<!--
  Ustawienia → Serwery MCP: gotowe przepisy (przeglądarka, narzędzia programisty Chrome, pliki, Git,
  GitHub, strony WWW, notatnik wiedzy), własny serwer, lista serwerów ze stanem i liczbą
  dozwolonych narzędzi, włączanie, edycja, usuwanie i zgody na narzędzia. Program serwera
  uruchamia się dopiero przy pierwszym użyciu (albo „Sprawdź”).
-->
<script lang="ts">
  import { Button, ConfirmDialog, Switch } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { McpServerTools, McpServerView } from '../../../api/types-mcp';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import {
    MCP_PRESETS,
    emptyForm,
    formFromPreset,
    formFromServer,
    runtimeOf,
    type McpForm,
    type McpPreset,
  } from '../../../logic/mcp';
  import { useApp } from '../../../state/context';
  import McpServerForm from './McpServerForm.svelte';
  import McpTools from './McpTools.svelte';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let servers = $state<readonly McpServerView[] | null>(null);
  let loadError = $state<string | null>(null);
  let form = $state<McpForm | null>(null);
  let folderSlot = $state(false);
  let open = $state<Record<string, McpServerTools>>({});
  let checking = $state<string | null>(null);
  let removing = $state<McpServerView | null>(null);
  let removeOpen = $state(false);

  async function load() {
    try {
      servers = await app.client.mcp.list();
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  /** Czy na komputerze jest program przepisu (`npx`/`uvx`) — według serwera z tym poleceniem. */
  function missingRuntime(p: McpPreset): boolean {
    const same = servers?.find((s) => runtimeOf(s.command) === p.runtime);
    return same ? same.resolved === null : false;
  }

  function startPreset(p: McpPreset) {
    form = formFromPreset(p, p.needsFolder ? '{folder}' : '');
    folderSlot = p.needsFolder;
  }

  function startCustom() {
    form = emptyForm();
    folderSlot = false;
  }

  async function check(s: McpServerView) {
    if (open[s.id]) {
      const { [s.id]: _, ...rest } = open;
      open = rest;
      return;
    }
    checking = s.id;
    try {
      const data = await app.client.mcp.tools(s.id);
      open = { ...open, [s.id]: data };
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      checking = null;
      await load();
    }
  }

  async function toggle(s: McpServerView, enabled: boolean) {
    try {
      const f = formFromServer(s);
      await app.client.mcp.save({
        id: s.id,
        name: s.name,
        command: s.command,
        args: [...s.args],
        cwd: s.cwd,
        env: f.env,
        trust: s.trust,
        enabled,
        timeout_ms: s.timeout_ms,
      });
      const { [s.id]: _, ...rest } = open;
      open = rest;
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    }
    await load();
  }

  function stateClass(s: McpServerView): string {
    if (s.state === 'connected') return 'wk-ok';
    if (s.state === 'failed') return 'wk-error';
    return 'wk-meta';
  }
</script>

<section class="wk-card" aria-labelledby="mcp-title">
  <h3 id="mcp-title">{t('mcp.title')}</h3>
  <p>{t('mcp.intro')}</p>
</section>

{#if form}
  <McpServerForm
    bind:form
    {folderSlot}
    onsaved={(name) => {
      form = null;
      app.toasts.show({ kind: 'success', message: t('mcp.saved', { name }) });
      void load();
    }}
    oncancel={() => (form = null)}
  />
{:else}
  <section class="wk-card" aria-labelledby="mcp-presets">
    <h3 id="mcp-presets">{t('mcp.presets')}</h3>
    <p class="wk-meta">{t('mcp.presetsHint')}</p>
    <ul class="wk-list">
      {#each MCP_PRESETS as p (p.id)}
        <li>
          <strong>{p.name}</strong>
          <span>{tk(`mcp.preset.${p.id}`)}</span>
          <span class="wk-code">{p.command} {p.args.join(' ')}</span>
          {#if missingRuntime(p)}<span class="wk-warn">{tk(`mcp.needs.${p.runtime}`)}</span>{/if}
          <div class="wk-actions">
            <Button
              size="sm"
              aria-label={`${t('mcp.add')}: ${p.name}`}
              onclick={() => startPreset(p)}>{t('mcp.add')}</Button
            >
          </div>
        </li>
      {/each}
    </ul>
    <div class="wk-actions">
      <Button size="sm" variant="secondary" onclick={startCustom}>{t('mcp.addCustom')}</Button>
    </div>
  </section>
{/if}

<section class="wk-card" aria-labelledby="mcp-servers">
  <h3 id="mcp-servers">{t('mcp.servers')}</h3>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />
  {:else if !servers}<Loading />
  {:else if servers.length === 0}<p class="wk-meta">{t('mcp.empty')}</p>
  {:else}
    <ul class="wk-list">
      {#each servers as s (s.id)}
        <li>
          <div class="wk-actions">
            <strong>{s.name}</strong>
            <span class={stateClass(s)}>{tk(`mcp.state.${s.state}`)}</span>
            <Switch
              label={s.enabled ? t('mcp.disable') : t('mcp.enable')}
              checked={s.enabled}
              onchange={(on) => void toggle(s, on)}
            />
          </div>
          <span class="wk-code">{s.command} {s.args.join(' ')}</span>
          {#if s.resolved === null}
            {@const runtime = runtimeOf(s.command)}
            <span class="wk-warn">{runtime ? tk(`mcp.needs.${runtime}`) : t('mcp.notFound')}</span>
          {/if}
          {#if s.server_info}<span class="wk-meta">{s.server_info}</span>{/if}
          {#if s.tools_total > 0}
            <span class="wk-meta"
              >{t('mcp.toolsCount', { approved: s.tools_approved, total: s.tools_total })}</span
            >
          {/if}
          {#if s.error}<span class="wk-error" role="alert">{s.error}</span>{/if}
          <div class="wk-actions">
            <Button
              size="sm"
              variant="primary"
              disabled={!s.enabled || checking !== null}
              aria-expanded={open[s.id] ? 'true' : 'false'}
              onclick={() => void check(s)}
              >{open[s.id] ? t('mcp.hideTools') : t('mcp.check')}</Button
            >
            <Button
              size="sm"
              variant="secondary"
              onclick={() => {
                form = formFromServer(s);
                folderSlot = false;
              }}>{t('mcp.edit')}</Button
            >
            <Button
              size="sm"
              variant="secondary"
              onclick={() => {
                removing = s;
                removeOpen = true;
              }}>{t('mcp.remove')}</Button
            >
          </div>
          {#if open[s.id]}
            {@const tools = open[s.id]!}
            <McpTools
              data={tools}
              onchange={(next) => {
                open = { ...open, [s.id]: next };
                void load();
              }}
            />
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<ConfirmDialog
  bind:open={removeOpen}
  title={t('mcp.removeTitle', { name: removing?.name ?? '' })}
  description={t('mcp.removeBody')}
  confirmLabel={t('mcp.remove')}
  cancelLabel={t('mcp.form.cancel')}
  danger
  onconfirm={() => {
    removeOpen = false;
    const target = removing;
    if (!target) return;
    void (async () => {
      try {
        await app.client.mcp.remove(target.id);
        app.toasts.show({ kind: 'success', message: t('mcp.removed', { name: target.name }) });
      } catch (e) {
        app.toasts.show({ kind: 'error', message: errorText(e) });
      }
      await load();
    })();
  }}
/>
