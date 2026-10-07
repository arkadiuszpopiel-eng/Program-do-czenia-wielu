<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    description?: string;
    icon?: Snippet;
    children?: Snippet;
    headingLevel?: 2 | 3 | 4;
  }

  let { title, description, icon, children, headingLevel = 3 }: Props = $props();
</script>

<div class="empty">
  {#if icon}<div class="icon" aria-hidden="true">{@render icon()}</div>{/if}
  <svelte:element this={`h${headingLevel}`} class="title">{title}</svelte:element>
  {#if description}<p class="desc">{description}</p>{/if}
  {#if children}<div class="actions">{@render children()}</div>{/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-8) var(--alfa-space-4);
    text-align: center;
  }
  .icon {
    display: inline-flex;
    padding: var(--alfa-space-3);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text-muted);
  }
  .title {
    font-size: var(--alfa-font-size-lg);
  }
  .desc {
    max-width: 44ch;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--alfa-space-2);
    margin-top: var(--alfa-space-2);
  }
</style>
