<!--
  Wyszukiwanie w Ustawieniach: do trzech trafień z instrukcji (komenda `help_search`) pod listą
  ustawień. Kliknięcie otwiera stronę „Instrukcja” na dziale z trafieniem.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { HelpHit } from '../../api/types-help';
  import { useApp } from '../../state/context';
  import { helpTarget } from '../../state/help-target.svelte';

  const { query, onopen }: { query: string; onopen: (page: string) => void } = $props();
  const app = useApp();
  const { t } = app.i18n;
  let hits = $state<readonly HelpHit[]>([]);

  $effect(() => {
    const text = query.trim();
    if (text.length < 3) {
      hits = [];
      return;
    }
    const timer = setTimeout(() => {
      app.client.help.search(text).then(
        (r) => (hits = r.slice(0, 3)),
        () => (hits = []),
      );
    }, 250);
    return () => clearTimeout(timer);
  });

  function pick(hit: HelpHit) {
    helpTarget.topic = hit.id;
    onopen('help');
  }
</script>

{#if hits.length}
  <section class="help-hits" aria-labelledby="settings-help-hits">
    <h3 id="settings-help-hits">{t('help.inGuide')}</h3>
    <ul>
      {#each hits as hit, i (`${hit.id}-${i}`)}
        <li>
          <Button size="sm" variant="ghost" onclick={() => pick(hit)}
            >{hit.title}{hit.section ? ` → ${hit.section}` : ''}</Button
          >
          <span class="snippet">{hit.snippet}</span>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .help-hits {
    margin-top: var(--alfa-space-4);
    border-top: 1px solid var(--alfa-color-border);
    padding-top: var(--alfa-space-3);
  }
  h3 {
    margin: 0 0 var(--alfa-space-2);
    font-size: var(--alfa-font-size-md);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--alfa-space-2);
  }
  li {
    display: grid;
    gap: var(--alfa-space-1);
    justify-items: start;
  }
  .snippet {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
</style>
