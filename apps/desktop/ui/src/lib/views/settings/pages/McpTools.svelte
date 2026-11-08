<!--
  Narzędzia serwera MCP ze stanem zgody. Opisy pochodzą od serwera — pokazywane jako zwykły tekst
  (bez Markdownu i odnośników). „Zezwól” wysyła odcisk tej wersji opisu: gdy serwer w międzyczasie
  go zmienił, rdzeń odmówi i lista odświeży się.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { McpServerTools, McpToolView } from '../../../api/types-mcp';
  import { toolReady } from '../../../logic/mcp';
  import { useApp } from '../../../state/context';
  import './work.css';

  interface Props {
    data: McpServerTools;
    onchange: (next: McpServerTools) => void;
  }

  let { data, onchange }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let busy = $state<string | null>(null);

  async function act(tool: McpToolView, approve: boolean) {
    busy = tool.name;
    try {
      const next = approve
        ? await app.client.mcp.approve(data.server.id, tool.name, tool.fingerprint)
        : await app.client.mcp.revoke(data.server.id, tool.name);
      onchange(next);
      if (approve) {
        app.toasts.show({
          kind: 'success',
          message: t('mcp.tool.approvedOk', { name: tool.name }),
        });
      }
    } catch (e) {
      app.toasts.show({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
      onchange(await app.client.mcp.tools(data.server.id));
    } finally {
      busy = null;
    }
  }
</script>

<div class="mcp-tools" role="group" aria-label={t('mcp.tools.title', { name: data.server.name })}>
  <p class="wk-note">{t('mcp.tools.untrustedNote')}</p>
  {#if data.tools.length === 0}
    <p class="wk-meta">{t('mcp.tools.empty')}</p>
  {:else}
    <ul class="wk-list">
      {#each data.tools as tool (tool.name)}
        <li>
          <div class="head">
            <strong class="wk-code">{tool.name}</strong>
            <span
              class={toolReady(tool) ? 'wk-ok' : tool.state === 'changed' ? 'wk-error' : 'wk-warn'}
              >{tk(`mcp.tool.${tool.state}`)}</span
            >
          </div>
          {#if tool.title}<span>{tool.title}</span>{/if}
          {#if tool.description}<span class="desc">{tool.description}</span>{/if}
          {#if tool.params.length}
            <span class="wk-meta">{t('mcp.tool.params', { names: tool.params.join(', ') })}</span>
          {/if}
          {#if tool.suspicious}<span class="wk-error" role="note">{t('mcp.tool.suspicious')}</span
            >{/if}
          <div class="wk-actions">
            {#if toolReady(tool)}
              <Button
                size="sm"
                variant="secondary"
                disabled={busy !== null}
                aria-label={`${t('mcp.tool.revoke')}: ${tool.name}`}
                onclick={() => void act(tool, false)}>{t('mcp.tool.revoke')}</Button
              >
            {:else}
              <Button
                size="sm"
                variant={tool.suspicious ? 'secondary' : 'primary'}
                disabled={busy !== null}
                aria-label={`${t('mcp.tool.approve')}: ${tool.name}`}
                onclick={() => void act(tool, true)}>{t('mcp.tool.approve')}</Button
              >
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .mcp-tools {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding-left: var(--alfa-space-3);
    border-left: 2px solid var(--alfa-color-border);
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--alfa-space-2);
  }
  .desc {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
