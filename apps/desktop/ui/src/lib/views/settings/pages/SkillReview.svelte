<!--
  Przegląd propozycji umiejętności: diff względem zainstalowanej wersji, hash treści, uwagi
  skanera (kwarantanna), wymagane narzędzia i zdolności. „Zainstaluj" / „Zwolnij z kwarantanny"
  wysyła hash przejrzanej wersji — rdzeń odmówi, jeśli treść zmieniła się od podglądu.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { SkillInfo, SkillReview } from '../../../api/types-work';
  import { useApp } from '../../../state/context';

  interface Props {
    skill: SkillInfo;
    ondone: () => void;
  }

  let { skill, ondone }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let review = $state<SkillReview | null>(null);
  let error = $state<string | null>(null);
  const quarantined = $derived(skill.state === 'quarantined');

  $effect(() => {
    review = null;
    error = null;
    app.client.skills.review(skill.id, skill.version).then(
      (r) => (review = r),
      (e: unknown) => (error = e instanceof Error ? e.message : String(e)),
    );
  });

  async function decide(kind: 'install' | 'reject') {
    const r = review;
    if (!r) return;
    try {
      if (kind === 'reject') {
        await app.client.skills.reject(r.skill.id, r.skill.version);
        app.toasts.show({ kind: 'info', message: t('skills.rejected') });
      } else if (r.skill.state === 'quarantined') {
        await app.client.skills.release(r.skill.id, r.skill.version, r.skill.hash);
        app.toasts.show({ kind: 'success', message: t('skills.released') });
      } else {
        await app.client.skills.approve(r.skill.id, r.skill.version, r.skill.hash);
        app.toasts.show({ kind: 'success', message: t('skills.installedOk') });
      }
      ondone();
    } catch (e) {
      app.toasts.show({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
  }
</script>

<section class="wk-card wk-accent" aria-labelledby="sk-review">
  <h3 id="sk-review">{t('skills.reviewTitle', { name: skill.name, version: skill.version })}</h3>
  {#if error}<p class="wk-error" role="alert">{error}</p>{/if}
  {#if review}
    {@const s = review.skill}
    <p>{s.description}</p>
    <p class="wk-meta">
      {tk(`skills.origin.${s.origin}`)} · {s.trusted ? t('skills.trusted') : t('skills.untrusted')}
      {#if review.previous_version}· {t('skills.replaces', {
          version: review.previous_version,
        })}{/if}
    </p>
    {#if quarantined}<p class="wk-warn">{t('skills.quarantineNote')}</p>{/if}
    {#each s.findings as f (f)}<p class="wk-error">{f}</p>{/each}
    <dl class="wk-grid">
      <div>
        <dt class="wk-meta">{t('skills.tools')}</dt>
        <dd>{s.required_tools.join(', ') || '—'}</dd>
      </div>
      <div>
        <dt class="wk-meta">{t('skills.capabilities')}</dt>
        <dd>{s.required_capabilities.join(', ') || '—'}</dd>
      </div>
    </dl>
    <h4>{t('skills.diff')}</h4>
    <pre class="wk-diff" aria-label={t('skills.diff')}>{#each review.diff as line, i (i)}<span
          class={line.kind}
          >{line.kind === 'added' ? '+ ' : line.kind === 'removed' ? '− ' : '  '}{line.text}
</span>{/each}</pre>
    <p class="wk-meta">{t('skills.hash')}: <code class="wk-code">{s.hash}</code></p>
    <div class="wk-actions">
      <Button size="sm" variant="primary" onclick={() => decide('install')}
        >{quarantined ? t('skills.release') : t('skills.install')}</Button
      >
      <Button size="sm" variant="secondary" onclick={() => decide('reject')}
        >{t('skills.reject')}</Button
      >
      <Button size="sm" variant="ghost" onclick={ondone}>{t('common.cancel')}</Button>
    </div>
  {:else if !error}
    <p class="wk-meta">{t('common.loading')}</p>
  {/if}
</section>
