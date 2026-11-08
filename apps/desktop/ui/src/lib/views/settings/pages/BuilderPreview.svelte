<!--
  Podgląd persony z Kreatora (liczony w rdzeniu): odmiana imienia, znak i kolor, głos v0
  (odsłuch), rola, narzędzia, autonomia, limity, ostrzeżenia, hash; wynik testu na sucho
  (decyzje polityki dla scenariusza). „Zapisz" — tylko po zaliczonym teście tego samego hasha.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { BuilderDryRun, BuilderPreview } from '../../../api/types-work';
  import { useApp } from '../../../state/context';

  interface Props {
    preview: BuilderPreview;
    dry: BuilderDryRun | null;
    busy: boolean;
    ondry: () => void;
    onsave: () => void;
    onvoice: () => void;
  }

  let { preview, dry, busy, ondry, onsave, onvoice }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const CASES = ['nom', 'gen', 'dat', 'acc', 'ins', 'loc', 'voc'] as const;
  const current = $derived(dry !== null && dry.hash === preview.hash);
</script>

<section class="wk-card wk-accent" aria-labelledby="bd-preview">
  <h3 id="bd-preview">{t('builder.preview')}</h3>
  <div class="persona">
    <span class="glyph" aria-hidden="true">{preview.glyph}</span>
    <div>
      <strong>{preview.name}</strong>
      <p class="wk-meta">{preview.role_name} · {preview.autonomy} · {preview.color}</p>
    </div>
  </div>
  {#if preview.character}<p>{preview.character}</p>{/if}
  <table class="wk-table">
    <caption class="wk-meta">{t('builder.forms')}</caption>
    <tbody>
      {#each preview.forms as form, i (i)}
        <tr><th scope="row">{tk(`builder.case.${CASES[i] ?? 'nom'}`)}</th><td>{form}</td></tr>
      {/each}
    </tbody>
  </table>
  <dl class="wk-grid">
    <div>
      <dt class="wk-meta">{t('builder.voice')}</dt>
      <dd>{preview.voice}</dd>
    </div>
    <div>
      <dt class="wk-meta">{t('builder.tools')}</dt>
      <dd>
        {preview.tools.join(', ') || '—'}{preview.read_only ? ` (${t('builder.readOnly')})` : ''}
      </dd>
    </div>
    <div>
      <dt class="wk-meta">{t('builder.fsWrite')}</dt>
      <dd>{preview.fs_write.join(', ') || '—'}</dd>
    </div>
    <div>
      <dt class="wk-meta">{t('builder.memory')}</dt>
      <dd>
        {t('builder.memoryValue', { scope: preview.memory_scope, days: preview.retain_days })}
      </dd>
    </div>
    <div>
      <dt class="wk-meta">{t('builder.maxSteps')}</dt>
      <dd>{preview.max_steps}</dd>
    </div>
  </dl>
  <details>
    <summary>{t('builder.systemPrompt')}</summary>
    <p class="wk-code">{preview.system_prompt}</p>
  </details>
  {#each preview.warnings as w (w)}<p class="wk-warn">{w}</p>{/each}
  <p class="wk-meta">{t('skills.hash')}: <code class="wk-code">{preview.hash}</code></p>
  <div class="wk-actions">
    <Button size="sm" variant="ghost" onclick={onvoice}>{t('builder.listen')}</Button>
    <Button size="sm" variant="secondary" disabled={busy} onclick={ondry}
      >{t('builder.dryRun')}</Button
    >
    <Button size="sm" variant="primary" disabled={busy || !current || !dry?.passed} onclick={onsave}
      >{t('builder.save')}</Button
    >
  </div>
  {#if dry && current}
    <h4>{dry.passed ? t('builder.dryPassed') : t('builder.dryFailed')}</h4>
    <table class="wk-table">
      <thead>
        <tr>
          <th scope="col">{t('builder.dryTool')}</th>
          <th scope="col">{t('builder.dryExpected')}</th>
          <th scope="col">{t('builder.dryOutcome')}</th>
          <th scope="col">{t('builder.dryWhy')}</th>
        </tr>
      </thead>
      <tbody>
        {#each dry.steps as s, i (i)}
          <tr class:wk-error={s.expected !== s.outcome}>
            <td><code>{s.tool}</code></td>
            <td>{tk(`builder.dry.${s.expected}`)}</td>
            <td>{tk(`builder.dry.${s.outcome}`)}</td>
            <td>{s.why}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="wk-meta">{t('builder.dryNeeded')}</p>
  {/if}
</section>

<style>
  .persona {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
  }
  .glyph {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border: 2px solid var(--alfa-color-border-strong);
    border-radius: var(--alfa-radius-full);
    font-size: var(--alfa-font-size-lg);
  }
</style>
