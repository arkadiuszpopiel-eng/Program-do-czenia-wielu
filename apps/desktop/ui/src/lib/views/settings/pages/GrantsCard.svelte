<!--
  Zgody „zawsze w tym zakresie” (Ustawienia → Uprawnienia): czas wybiera właściciel w oknie
  Sędziego Cieni (1 h, 3 h, doba albo do odwołania). Tu lista i „Odwołaj” — odwołanie tylko zawęża
  uprawnienia, więc działa od razu, bez okna Sędziego. Nadawanie zgód — wyłącznie okno Sędziego.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import Clock from '@lucide/svelte/icons/clock';
  import InfinityIcon from '@lucide/svelte/icons/infinity';
  import type { GrantView } from '../../../api/types-hub';
  import { showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let grants = $state<readonly GrantView[] | null>(null);
  let busy = $state<string | null>(null);

  async function reload() {
    try {
      grants = await app.client.permissions.grants();
    } catch {
      grants = [];
    }
  }

  // Lista odświeża się przy każdej zmianie stanu Sędziego (restart, STOP czyści zgody).
  $effect(() => {
    void app.broker.view;
    void reload();
  });

  function until(g: GrantView): string {
    if (g.until_ms === null) return t('broker.grants.untilRevoked');
    const iso = new Date(g.until_ms).toISOString();
    const today = new Date().toDateString() === new Date(g.until_ms).toDateString();
    const at = app.i18n.time(iso);
    return t('broker.grants.until', { when: today ? at : `${app.i18n.day(iso)}, ${at}` });
  }

  function who(g: GrantView): string {
    if (!g.agent) return t('broker.grants.allAgents');
    return g.agent.charAt(0).toUpperCase() + g.agent.slice(1);
  }

  async function revoke(g: GrantView) {
    busy = g.id;
    try {
      await app.client.permissions.revokeGrant(g.id);
      app.toasts.show({ kind: 'success', message: t('broker.grants.revoked') });
      await reload();
    } catch (error) {
      showError(app.toasts, error);
    } finally {
      busy = null;
    }
  }
</script>

<section class="card" aria-labelledby="grants-title">
  <h3 id="grants-title">{t('broker.grants.title')}</h3>
  <p class="desc">{t('broker.grants.intro')}</p>
  {#if grants && grants.length === 0}
    <p class="empty">{t('broker.grants.empty')}</p>
  {:else if grants}
    <ul>
      {#each grants as g (g.id)}
        <li>
          <span class="icon" aria-hidden="true">
            {#if g.until_ms === null}<InfinityIcon size={16} strokeWidth={1.5} />{:else}<Clock
                size={16}
                strokeWidth={1.5}
              />{/if}
          </span>
          <span class="text">
            <span class="scope">{g.scope}</span>
            <span class="meta">{who(g)} · {until(g)}</span>
          </span>
          <Button
            variant="secondary"
            size="sm"
            disabled={busy === g.id}
            aria-label={t('broker.grants.revokeLabel', { scope: g.scope })}
            onclick={() => void revoke(g)}>{t('broker.grants.revoke')}</Button
          >
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
  .empty {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  ul {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3) 0;
    border-top: 1px solid var(--alfa-color-border);
  }
  .icon {
    display: flex;
    color: var(--alfa-color-text-muted);
  }
  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .scope {
    overflow-wrap: anywhere;
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
