<!--
  Chipy załączników composera (PLAN §14.8): miniatura obrazu przez protokół zasobów (`asset:`,
  nigdy bajty przez IPC), nazwa, rozmiar, szacunek tokenów, ostrzeżenie o ucięciu / samych
  metadanych, usuwanie. Przy dużych załącznikach — szacunek tokenów, udziału w oknie kontekstu
  i kosztu (stawka z ostatnich odpowiedzi; profil lokalny — bez kosztu).
-->
<script lang="ts">
  import { IconButton } from '@alfa/ui-kit';
  import FileIcon from '@lucide/svelte/icons/file';
  import FileText from '@lucide/svelte/icons/file-text';
  import ImageIcon from '@lucide/svelte/icons/image';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import X from '@lucide/svelte/icons/x';
  import type { ModelProfile, Turn } from '../../api/types';
  import {
    LARGE_ATTACHMENT_TOKENS,
    contextShare,
    estimateCost,
    totalTokens,
  } from '../../logic/attachments';
  import type { AttachmentsState } from '../../state/attachments.svelte';
  import { useApp } from '../../state/context';

  interface Props {
    att: AttachmentsState;
    profile: ModelProfile;
    turns: readonly Turn[];
  }

  let { att, profile, turns }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;

  const tokens = $derived(totalTokens(att.items));
  const large = $derived(tokens >= LARGE_ATTACHMENT_TOKENS);
  const share = $derived(contextShare(tokens, app.costs?.context.max_tokens ?? 200_000));
  const cost = $derived(large ? estimateCost(tokens, profile, turns) : null);
  const costText = $derived(
    cost === null
      ? ''
      : cost.basis === 'local'
        ? t('att.costLocal')
        : t('att.cost', { cost: app.i18n.money({ minor: cost.minor, currency: 'PLN' }) }),
  );

  function preview(path: string): string | null {
    return app.client.attachments.previewUrl(path);
  }
</script>

{#if att.items.length > 0}
  <div class="attachments">
    <ul class="chips" aria-label={t('att.list', { n: att.items.length })}>
      {#each att.items as a (a.id)}
        {@const src = a.kind === 'image' ? preview(a.path) : null}
        <li class="chip">
          {#if src}
            <img class="thumb" {src} alt="" loading="lazy" decoding="async" />
          {:else}
            <span class="icon" aria-hidden="true">
              {#if a.kind === 'image'}<ImageIcon size={16} strokeWidth={1.5} />
              {:else if a.kind === 'text'}<FileText size={16} strokeWidth={1.5} />
              {:else}<FileIcon size={16} strokeWidth={1.5} />{/if}
            </span>
          {/if}
          <span class="text">
            <span class="name" title={a.name}>{a.name}</span>
            <span class="meta">
              {app.i18n.bytes(a.bytes)} · {t('att.tokens', { tokens: app.i18n.int(a.tokens) })}
            </span>
          </span>
          {#if a.delivery !== 'full'}
            <span class="warn" title={t(`att.delivery.${a.delivery}`)}>
              <TriangleAlert size={14} strokeWidth={1.75} aria-hidden="true" />
              <span class="alfa-visually-hidden">{t(`att.delivery.${a.delivery}`)}</span>
            </span>
          {/if}
          <IconButton
            size="sm"
            label={t('att.remove', { name: a.name })}
            onclick={() => void att.remove(a)}
          >
            <X size={14} strokeWidth={1.75} />
          </IconButton>
        </li>
      {/each}
    </ul>
    {#if large}
      <p class="estimate">
        {t('att.large', {
          tokens: app.i18n.int(tokens),
          share: app.i18n.percent(share),
        })}{costText}
      </p>
    {/if}
  </div>
{/if}
<p class="alfa-visually-hidden" role="status">{att.status}</p>

<style>
  .attachments {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin-bottom: var(--alfa-space-1);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-2);
    max-width: 280px;
    min-height: 40px;
    padding: 2px 2px 2px var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface);
  }
  .thumb {
    width: 32px;
    height: 32px;
    border-radius: 4px;
    object-fit: cover;
  }
  .icon {
    display: inline-flex;
    color: var(--alfa-color-text-muted);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    font-size: var(--alfa-font-size-sm);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .warn {
    display: inline-flex;
    color: var(--alfa-color-warning);
  }
  .estimate {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
