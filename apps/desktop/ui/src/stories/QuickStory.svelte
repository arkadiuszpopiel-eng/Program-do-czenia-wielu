<!-- Makieta 5: Szybkie pytanie (okno 640 px) z odpowiedzią strumieniowaną z atrapy. -->
<script lang="ts">
  import { untrack } from 'svelte';
  import QuickApp from '../quick/QuickApp.svelte';
  import { FakeAlfaClient } from '../lib/api/fake/fake-client';

  interface Props {
    theme?: 'light' | 'dark';
    question?: string;
  }

  const props: Props = $props();
  const theme = untrack(() => props.theme ?? 'light');
  const client = new FakeAlfaClient();
  document.documentElement.setAttribute('data-theme', theme);
  $effect(() => {
    const question = untrack(() => props.question);
    if (question) void client.quick.ask(question);
  });
</script>

<div class="frame">
  <QuickApp {client} />
</div>

<style>
  .frame {
    width: 700px;
    padding: var(--alfa-space-8) 0;
  }
</style>
