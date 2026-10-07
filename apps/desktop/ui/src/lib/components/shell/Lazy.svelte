<!-- Moduł ładowany leniwie (`import()`): panele, ustawienia, paleta. Szkielet dopiero po 300 ms. -->
<script lang="ts" generics="P extends Record<string, unknown>">
  import type { Component } from 'svelte';
  import { Skeleton } from '@alfa/ui-kit';

  interface Props {
    load: () => Promise<{ default: Component<P> }>;
    props: P;
    label: string;
  }

  let { load, props, label }: Props = $props();
  const promise = $derived(load());
  let slow = $state(false);

  $effect(() => {
    void promise;
    slow = false;
    const handle = setTimeout(() => (slow = true), 300);
    return () => clearTimeout(handle);
  });
</script>

{#await promise}
  {#if slow}<div class="pad"><Skeleton lines={4} {label} /></div>{/if}
{:then module}
  <module.default {...props} />
{:catch error}
  <p class="error" role="alert">{String(error)}</p>
{/await}

<style>
  .pad {
    padding: var(--alfa-space-4);
  }
  .error {
    padding: var(--alfa-space-4);
    color: var(--alfa-color-error);
  }
</style>
