<!--
  Wprowadzenie → Mosty CLI (opcjonalnie): wykryte narzędzia `claude` / `codex` i „Zaloguj
  w terminalu" — wbudowany terminal z profilem logowania (gest użytkownika). Logowanie wykonuje
  człowiek; Alfa nie czyta ani nie przechowuje tokenów CLI. Mosty są wyłączone do decyzji
  w Ustawieniach (karty zgodności).
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { BridgeCard } from '../../api/types-tasks';
  import LoadFailed from '../../components/shell/LoadFailed.svelte';
  import { loginProfile } from '../../logic/work';
  import { load, type Loadable } from '../../state/attempt';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let cards = $state<Loadable<readonly BridgeCard[]>>({ status: 'loading' });
  const bridges = $derived(
    (cards.status === 'ready' ? cards.value : []).filter(
      (c) => c.bridge && c.login_command && loginProfile(c.bridge),
    ),
  );

  // Błąd rdzenia to nie „nie wykryto narzędzi" — pokazujemy go z „Ponów".
  async function loadCards() {
    cards = { status: 'loading' };
    cards = await load(() => app.client.bridges.list(false));
  }

  $effect(() => {
    void loadCards();
  });
</script>

{#if cards.status === 'loading'}
  <p role="status">{t('common.loading')}</p>
{:else if cards.status === 'failed'}
  <LoadFailed error={cards.error} onretry={() => void loadCards()} />
{:else if bridges.length === 0}
  <p class="muted">{t('ob.bridges.none')}</p>
{:else}
  <ul class="list">
    {#each bridges as card (card.route_id)}
      {@const profile = card.bridge ? loginProfile(card.bridge) : null}
      <li>
        <span class="who">
          <span class="name">{card.name}</span>
          <span class="state" class:ok={card.detected}
            >{card.detected
              ? t('ob.bridges.installed', { version: card.version ?? '—' })
              : t('ob.bridges.missing')}</span
          >
        </span>
        {#if profile}
          <Button
            variant={card.detected ? 'primary' : 'secondary'}
            disabled={!card.detected}
            aria-label={`${t('bridges.loginTerminal')}: ${card.name}`}
            onclick={() => app.work.openTerminal(profile)}>{t('bridges.loginTerminal')}</Button
          >
        {/if}
      </li>
    {/each}
  </ul>
{/if}
<p class="muted small">{t('ob.bridges.privacy')}</p>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface2);
    font-weight: var(--alfa-weight-semibold);
  }
  .who {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .state {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-regular);
  }
  .state.ok {
    color: var(--alfa-color-success);
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .small {
    font-size: var(--alfa-font-size-xs);
  }
</style>
