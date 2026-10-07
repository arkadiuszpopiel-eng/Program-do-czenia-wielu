<!--
  Pozycja katalogu „Modele i silniki": nazwa, rodzaj, rozmiar, licencja, stan (aria-live), uwagi
  („do potwierdzenia przez człowieka”, SHA-256 przypięty, instalacja ręczna), postęp pobierania,
  akcje (pobierz / wznów / przerwij / sprawdź / napraw / usuń / używaj do wyszukiwania) i karta zgody TOFU
  z policzonym SHA-256 każdego pliku bez przypiętej sumy.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { ModelItem } from '../../../api/types-models';
  import { groupedHash, itemActions, progressPercent } from '../../../logic/models';
  import { useApp } from '../../../state/context';

  interface Props {
    item: ModelItem;
    busy: boolean;
    ondownload: () => void;
    oncancel: () => void;
    onverify: () => void;
    onremove: () => void;
    ontrust: () => void;
    onactivate: () => void;
    onrepair: () => void;
  }

  let {
    item,
    busy,
    ondownload,
    oncancel,
    onverify,
    onremove,
    ontrust,
    onactivate,
    onrepair,
  }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const actions = $derived(itemActions(item));
  const percent = $derived(progressPercent(item));
  const titleId = $derived(`mm-${item.id}`);
  const unpinned = $derived(item.files.filter((f) => !f.pinned_sha256 && f.sha256));

  function progressLabel(): string {
    const p = item.progress;
    if (!p) return '';
    const done = app.i18n.bytes(p.done);
    return p.total
      ? t('engines.progressText', { file: p.file, done, total: app.i18n.bytes(p.total) })
      : t('engines.progressUnknown', { file: p.file, done });
  }
</script>

<li class="row" aria-labelledby={titleId}>
  <div class="head">
    <h4 id={titleId}>{item.name}</h4>
    <span class="kind">{tk(`engines.kind.${item.kind}`)}</span>
  </div>
  <p class="wk-meta">
    {t('engines.meta', { size: app.i18n.bytes(item.size_bytes), license: item.license })}
  </p>
  <ul class="badges">
    {#if item.active}<li class="badge ok">{t('engines.active')}</li>{/if}
    {#if item.pinned}<li class="badge">{t('engines.pinned')}</li>{/if}
    {#if !item.confirmed}<li class="badge warn">{t('engines.unconfirmed')}</li>{/if}
    {#if !item.downloadable}<li class="badge">{t('engines.manual')}</li>{/if}
  </ul>
  <p class="note">{app.i18n.text(item.note)}</p>
  <p
    role="status"
    aria-live="polite"
    class:wk-ok={item.state === 'installed'}
    class:wk-warn={item.state === 'needs_trust' ||
      item.state === 'corrupt' ||
      item.state === 'failed'}
  >
    {tk(`engines.state.${item.state}`)}
  </p>
  {#if item.error}<p class="wk-error">{item.error}</p>{/if}
  {#if item.progress && item.state !== 'installed'}
    <div class="bar-row">
      <progress
        class="bar"
        max="100"
        value={percent ?? undefined}
        aria-label={t('engines.progress', { name: item.name })}
        aria-valuetext={progressLabel()}
      ></progress>
      <span class="wk-meta">{progressLabel()}</span>
    </div>
  {/if}
  {#if actions.trust}
    <section class="trust" aria-labelledby={`${titleId}-trust`}>
      <h5 id={`${titleId}-trust`}>{t('engines.trust.title')}</h5>
      <p>{t('engines.trust.body')}</p>
      <p class="wk-meta">{t('engines.trust.license', { license: item.license })}</p>
      <dl>
        {#each unpinned as f (f.name)}
          <dt>{t('engines.trust.file')}: <span class="wk-code">{f.name}</span></dt>
          <dd>
            <span class="wk-meta">{t('engines.trust.hash')}</span>
            <code class="hash">{groupedHash(f.sha256 ?? '')}</code>
            <span class="wk-meta url">{f.url}</span>
          </dd>
        {/each}
      </dl>
      <div class="wk-actions">
        <Button size="sm" variant="primary" disabled={busy} onclick={ontrust}
          >{t('engines.trust.accept')}</Button
        >
        <Button size="sm" variant="ghost" disabled={busy} onclick={onremove}
          >{t('engines.trust.reject')}</Button
        >
      </div>
    </section>
  {/if}
  <p class="wk-meta path">{t('engines.target', { path: item.target })}</p>
  <div class="wk-actions">
    {#if actions.download}
      <Button size="sm" variant="secondary" disabled={busy} onclick={ondownload}
        >{t('engines.download')}</Button
      >
    {/if}
    {#if actions.resume}
      <Button size="sm" variant="secondary" disabled={busy} onclick={ondownload}
        >{t('engines.resume')}</Button
      >
    {/if}
    {#if actions.cancel}
      <Button size="sm" variant="ghost" disabled={busy} onclick={oncancel}
        >{t('engines.cancel')}</Button
      >
    {/if}
    {#if actions.activate}
      <Button size="sm" variant="primary" disabled={busy} onclick={onactivate}
        >{t('engines.activate')}</Button
      >
    {/if}
    {#if actions.verify}
      <Button size="sm" variant="ghost" disabled={busy} onclick={onverify}
        >{t('engines.verify')}</Button
      >
    {/if}
    {#if actions.repair}
      <Button size="sm" variant="ghost" disabled={busy} onclick={onrepair}
        >{t('engines.repair')}</Button
      >
    {/if}
    {#if actions.remove && !actions.trust}
      <Button size="sm" variant="ghost" disabled={busy} onclick={onremove}
        >{t('engines.remove')}</Button
      >
    {/if}
  </div>
</li>

<style>
  .row {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding-bottom: var(--alfa-space-3);
    border-bottom: 1px solid var(--alfa-color-border);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--alfa-space-2);
  }
  h4 {
    font-size: var(--alfa-font-size-sm);
  }
  h5 {
    font-size: var(--alfa-font-size-sm);
  }
  .kind {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .badge {
    padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .badge.ok {
    border-color: var(--alfa-color-success);
    color: var(--alfa-color-text);
  }
  .badge.warn {
    border-color: var(--alfa-color-warning);
    color: var(--alfa-color-text);
  }
  .note {
    margin: 0;
  }
  .path,
  .url {
    overflow-wrap: anywhere;
  }
  .bar-row {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
  }
  .bar {
    width: 100%;
    height: 8px;
    accent-color: var(--alfa-color-info);
  }
  .trust {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-warning);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
  }
  .trust p {
    margin: 0;
  }
  dl {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: 0;
  }
  dd {
    display: flex;
    flex-direction: column;
    margin: 0 0 var(--alfa-space-2);
  }
  .hash {
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-sm);
    overflow-wrap: anywhere;
    user-select: all;
  }
</style>
