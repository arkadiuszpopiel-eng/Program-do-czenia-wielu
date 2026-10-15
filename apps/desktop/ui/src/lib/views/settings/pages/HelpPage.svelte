<!--
  Ustawienia → Instrukcja (P5): przewodnik użytkownika wbudowany w program. Spis działów,
  wyszukiwanie (bez wielkości liter i polskich znaków) z fragmentami trafień i treść działu —
  HTML zsanitowany w Rust, wstawiany wyłącznie przez `SanitizedHtml`.
-->
<script lang="ts">
  import { Button, SanitizedHtml, Select, TextField } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { HelpHit, HelpPage, HelpTopic } from '../../../api/types-help';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { useApp } from '../../../state/context';
  import { takeHelpTopic } from '../../../state/help-target.svelte';
  import './work.css';

  const app = useApp();
  const { t } = app.i18n;
  let topics = $state<readonly HelpTopic[] | null>(null);
  let current = $state<HelpPage | null>(null);
  let chosen = $state('');
  let loadError = $state<string | null>(null);
  let query = $state('');
  let hits = $state<readonly HelpHit[] | null>(null);
  const options = $derived((topics ?? []).map((x) => ({ value: x.id, label: x.title })));

  async function open(id: string) {
    chosen = id;
    try {
      current = await app.client.help.page(id);
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  async function load() {
    try {
      topics = await app.client.help.topics();
      loadError = null;
      const wanted = takeHelpTopic();
      const first = topics.find((x) => x.id === wanted) ?? topics[0];
      if (first) await open(first.id);
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  $effect(() => {
    const text = query.trim();
    if (!text) {
      hits = null;
      return;
    }
    let stale = false;
    const timer = setTimeout(() => {
      app.client.help.search(text).then(
        (r) => {
          if (!stale) hits = r;
        },
        () => {
          if (!stale) hits = [];
        },
      );
    }, 250);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  });

  function pick(hit: HelpHit) {
    query = '';
    void open(hit.id);
  }
</script>

<section class="wk-card" aria-labelledby="help-title">
  <h3 id="help-title">{t('help.title')}</h3>
  <p>{t('help.intro')}</p>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />
  {:else if !topics}<Loading />
  {:else}
    <div class="wk-grid">
      <div class="wk-field">
        <span id="help-topic">{t('help.topic')}</span>
        <Select labelledby="help-topic" value={chosen} {options} onchange={(id) => void open(id)} />
      </div>
      <TextField label={t('help.search')} bind:value={query} spellcheck="false" />
    </div>
    {#if hits}
      <div role="status">
        {#if hits.length === 0}
          <p class="wk-meta">{t('help.noHits')}</p>
        {:else}
          <p class="wk-meta">{t('help.hits', { n: hits.length })}</p>
        {/if}
      </div>
      <ul class="wk-list" aria-label={t('help.results')}>
        {#each hits as hit, i (`${hit.id}-${i}`)}
          <li>
            <Button size="sm" variant="ghost" onclick={() => pick(hit)}
              >{hit.title}{hit.section ? ` → ${hit.section}` : ''}</Button
            >
            <span class="wk-meta">{hit.snippet}</span>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

{#if current && !hits}
  <section class="wk-card help-body" aria-label={current.title}>
    <SanitizedHtml html_sanitized={current.html_sanitized} />
  </section>
{/if}

<style>
  .help-body {
    font-size: var(--alfa-font-size-md);
    line-height: 1.6;
  }
</style>
