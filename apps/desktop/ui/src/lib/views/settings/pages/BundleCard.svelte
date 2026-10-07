<!--
  Karta pakietu 1–6: ocena (6 segmentów + liczba, nie tylko kolor), nazwa, „Zalecany”, dopasowanie
  do sprzętu z uzasadnieniem, opis, wymagania, rozmiar i postęp, stan (aria-live), działania pakietu
  (pobierz / dokończ / napraw, sprawdź SHA-256), elementy z działaniami pojedynczo (pobierz, wznów,
  napraw, sprawdź; zgoda TOFU i instalacja ręczna — w katalogu poniżej) i uwagi o jakości (normy).
-->
<script lang="ts">
  import { Button, Chip } from '@alfa/ui-kit';
  import type { BundleItemView, ModelBundle } from '../../../api/types-models';
  import {
    bundleAction,
    bundleItemActions,
    bundlePercent,
    canVerify,
  } from '../../../logic/bundles';
  import { useApp } from '../../../state/context';

  interface Props {
    bundle: ModelBundle;
    busy: boolean;
    ondownload: () => void;
    onverify: () => void;
    onitem: (item: BundleItemView, action: 'download' | 'verify' | 'repair') => void;
  }

  let { bundle, busy, ondownload, onverify, onitem }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const titleId = $derived(`bundle-${bundle.id}`);
  const action = $derived(bundleAction(bundle));
  const percent = $derived(bundlePercent(bundle));
  const name = $derived(app.i18n.text(bundle.name));
  const fitTone = $derived(
    bundle.fit.kind === 'fits' ? 'success' : bundle.fit.kind === 'tight' ? 'warning' : 'error',
  );
  const showProgress = $derived(
    bundle.state === 'downloading' || bundle.state === 'partial' || bundle.state === 'needs_trust',
  );
</script>

<li class="card" class:recommended={bundle.recommended} aria-labelledby={titleId}>
  <div class="head">
    <div class="rating" role="img" aria-label={t('bundles.rating', { n: bundle.rating })}>
      <span class="score" aria-hidden="true">{bundle.rating}</span>
      <span class="meter" aria-hidden="true">
        {#each [1, 2, 3, 4, 5, 6] as step (step)}
          <span class="seg" class:on={step <= bundle.rating}></span>
        {/each}
      </span>
    </div>
    <h4 id={titleId}>{name}</h4>
    <div class="chips">
      {#if bundle.recommended}
        <Chip size="sm" tone="success">{t('bundles.recommended')}</Chip>
      {/if}
      <Chip size="sm" tone={fitTone}>{tk(`bundles.fit.${bundle.fit.kind}`)}</Chip>
    </div>
  </div>
  <p>{app.i18n.text(bundle.summary)}</p>
  <p class="wk-meta">
    {t('bundles.requirements', { text: app.i18n.text(bundle.requirements.text) })}
  </p>
  {#if bundle.fit.reason}
    <p class:wk-warn={bundle.fit.kind === 'tight'} class:wk-error={bundle.fit.kind === 'too_weak'}>
      {app.i18n.text(bundle.fit.reason)}
    </p>
  {/if}
  <p class="wk-meta">
    {t('bundles.size', { size: app.i18n.bytes(bundle.size_bytes) })}
    {#if bundle.missing_bytes > 0}
      · {t('bundles.missing', { size: app.i18n.bytes(bundle.missing_bytes) })}
    {/if}
    · {t('bundles.count', { done: bundle.installed, n: bundle.total })}
  </p>
  {#if showProgress}
    <progress
      class="bar"
      max="100"
      value={percent ?? undefined}
      aria-label={t('bundles.progress', { name })}
    ></progress>
  {/if}
  <p
    role="status"
    aria-live="polite"
    class:wk-ok={bundle.state === 'installed'}
    class:wk-warn={bundle.state === 'corrupt' || bundle.state === 'needs_trust'}
  >
    {tk(`bundles.state.${bundle.state}`)}
  </p>
  <div class="wk-actions">
    {#if action}
      <Button
        size="sm"
        variant={bundle.recommended || action === 'repair' ? 'primary' : 'secondary'}
        disabled={busy}
        onclick={ondownload}>{t(`bundles.${action}`)}</Button
      >
    {/if}
    {#if canVerify(bundle)}
      <Button size="sm" variant="ghost" disabled={busy} onclick={onverify}
        >{t('bundles.verify')}</Button
      >
    {/if}
  </div>
  <details>
    <summary>{t('bundles.items', { n: bundle.total })}</summary>
    <ul class="items">
      {#each bundle.items as item (item.id)}
        {@const can = bundleItemActions(item)}
        <li>
          <div class="item-head">
            <span class="item-name">{item.name}</span>
            <span class="wk-meta">
              {tk(`bundles.kind.${item.kind}`)} · {app.i18n.bytes(item.size_bytes)}
              {#if item.fallback}· {t('bundles.item.fallback')}{/if}
            </span>
          </div>
          <span
            class="wk-meta"
            class:wk-ok={item.state === 'installed'}
            class:wk-warn={item.state === 'corrupt' ||
              item.state === 'failed' ||
              item.state === 'needs_trust'}
          >
            {tk(`engines.state.${item.state}`)}
            {#if can.trust}· {t('bundles.item.trust')}{/if}
            {#if can.manual}· {t('bundles.item.manual')}{/if}
          </span>
          {#if can.download || can.resume || can.repair || can.verify}
            <div
              class="wk-actions"
              role="group"
              aria-label={t('bundles.item.actions', { name: item.name })}
            >
              {#if can.download}
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={busy}
                  onclick={() => onitem(item, 'download')}>{t('bundles.item.download')}</Button
                >
              {/if}
              {#if can.resume}
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={busy}
                  onclick={() => onitem(item, 'download')}>{t('bundles.item.resume')}</Button
                >
              {/if}
              {#if can.verify}
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onclick={() => onitem(item, 'verify')}>{t('bundles.item.verify')}</Button
                >
              {/if}
              {#if can.repair}
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onclick={() => onitem(item, 'repair')}>{t('bundles.item.repair')}</Button
                >
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  </details>
  <details>
    <summary>{t('bundles.quality', { n: bundle.quality.length })}</summary>
    <p class="wk-note">{t('bundles.quality.disclaimer')}</p>
    <dl class="quality">
      {#each bundle.quality as note, i (i)}
        <dt>
          {app.i18n.text(note.aspect)} <span class="standard">{note.standard}</span>
        </dt>
        <dd>{app.i18n.text(note.text)}</dd>
      {/each}
    </dl>
  </details>
</li>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
  }
  .card.recommended {
    border-color: var(--alfa-color-success);
    border-width: 2px;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-3);
  }
  h4 {
    font-size: var(--alfa-font-size-md);
  }
  .rating {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .score {
    min-width: 1.5em;
    font-size: var(--alfa-font-size-xl);
    font-weight: 600;
    line-height: 1;
    text-align: center;
  }
  .meter {
    display: inline-flex;
    gap: 2px;
  }
  .seg {
    width: 8px;
    height: 16px;
    border: 1px solid var(--alfa-color-border-strong);
    border-radius: 2px;
  }
  .seg.on {
    border-color: var(--alfa-color-text);
    background: var(--alfa-color-text);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
    margin-left: auto;
  }
  p {
    margin: 0;
  }
  .bar {
    width: 100%;
    height: 8px;
    accent-color: var(--alfa-color-info);
  }
  summary {
    cursor: pointer;
    font-weight: 600;
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: var(--alfa-space-2) 0 0;
    padding: 0;
    list-style: none;
  }
  .items > li {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding-bottom: var(--alfa-space-2);
    border-bottom: 1px solid var(--alfa-color-border);
  }
  .items > li:last-child {
    border-bottom: 0;
  }
  .item-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  .item-name {
    font-weight: 600;
  }
  .quality {
    display: grid;
    gap: var(--alfa-space-1);
    margin: var(--alfa-space-2) 0 0;
  }
  .quality dt {
    font-weight: 600;
  }
  .quality dd {
    margin: 0 0 var(--alfa-space-2);
  }
  .standard {
    margin-left: var(--alfa-space-1);
    padding: 0 var(--alfa-space-1);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    font-weight: 400;
  }
</style>
