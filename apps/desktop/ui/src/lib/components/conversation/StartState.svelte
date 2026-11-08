<!-- Pusty stan rozmowy (makieta 1): obsada, 3 podpowiedzi, podpowiedź „dodaj klucz" bez kluczy. -->
<script lang="ts">
  import { Avatar, Button, agentIds } from '@alfa/ui-kit';
  import Sparkles from '@lucide/svelte/icons/sparkles';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const suggestions = [1, 2, 3].map((n) => ({
    title: app.i18n.tk(`conv.start.s${n}.title`),
    text: app.i18n.tk(`conv.start.s${n}.text`),
  }));

  function pick(text: string) {
    app.setDraft(text);
    requestAnimationFrame(() => document.getElementById('alfa-composer')?.focus());
  }
</script>

<div class="start">
  <div class="cast" aria-hidden="true">
    {#each agentIds as id (id)}<Avatar agent={id} size={32} />{/each}
  </div>
  <h2 class="title">{t('conv.start.title')}</h2>
  <p class="lead">{t('conv.start.subtitle')}</p>
  <ul class="cards">
    {#each suggestions as s (s.title)}
      <li>
        <button type="button" class="card" onclick={() => pick(s.text)}>
          <Sparkles size={16} strokeWidth={1.5} aria-hidden="true" />
          <span class="card-title">{s.title}</span>
          <span class="card-text">{s.text}</span>
        </button>
      </li>
    {/each}
  </ul>
  {#if app.system && !app.system.keys_configured}
    <p class="keys">
      {t('banner.noKeys')}
      <Button size="sm" variant="secondary" onclick={() => app.addProviderKey()}
        >{t('banner.addKey')}</Button
      >
    </p>
  {/if}
</div>

<style>
  .start {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--alfa-space-3);
    width: min(var(--alfa-size-reading-column-wide), 100%);
    margin: auto;
    padding: var(--alfa-space-8) var(--alfa-space-4);
    text-align: center;
  }
  .cast {
    display: flex;
    gap: var(--alfa-space-2);
  }
  .title {
    font-size: var(--alfa-font-size-2xl);
  }
  .lead {
    color: var(--alfa-color-text-muted);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: var(--alfa-space-3);
    width: 100%;
    margin: var(--alfa-space-4) 0 0;
    padding: 0;
    list-style: none;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-1);
    width: 100%;
    height: 100%;
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    color: var(--alfa-color-text);
    text-align: left;
    transition: transform var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .card:hover {
    background: var(--alfa-color-surface2);
  }
  .card:active {
    transform: scale(0.99);
  }
  .card-title {
    font-weight: var(--alfa-weight-semibold);
  }
  .card-text {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: var(--alfa-space-2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
</style>
