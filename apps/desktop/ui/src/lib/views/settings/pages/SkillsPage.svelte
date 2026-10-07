<!--
  Ustawienia → Umiejętności: biblioteka (stan, wersja, źródło, kwarantanna), propozycje do
  przeglądu (diff + hash; instalacja tylko przejrzanej wersji), uruchomienie (zadanie agentki
  w bieżącej sesji), wyłączenie, eksport/import paczki (import z zewnątrz → kwarantanna)
  i własna propozycja z manifestu JSON. Nic nie wchodzi w życie bez decyzji użytkownika.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { SkillInfo } from '../../../api/types-work';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { attempt, load } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import SkillReview from './SkillReview.svelte';
  import SkillRun from './SkillRun.svelte';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  let skills = $state<SkillInfo[]>([]);
  let reviewing = $state<SkillInfo | null>(null);
  let running = $state<SkillInfo | null>(null);
  let manifest = $state('');
  let manifestError = $state<string | null>(null);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);
  const pending = $derived(
    skills.filter((s) => s.state === 'proposed' || s.state === 'quarantined'),
  );
  const installed = $derived(skills.filter((s) => s.state === 'installed'));
  const other = $derived(skills.filter((s) => !pending.includes(s) && !installed.includes(s)));

  async function reload() {
    const result = await load(() => app.client.skills.list());
    if (result.status === 'ready') {
      skills = [...result.value];
      loaded = true;
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void reload();
  });

  $effect(() =>
    app.on((event) => {
      if (event.type === 'SkillsChanged') void reload();
    }),
  );

  async function run(action: () => Promise<unknown>, message?: string) {
    if (!(await attempt(app.toasts, action))) return;
    if (message) app.toasts.show({ kind: 'success', message });
    await reload();
  }

  async function exportBundle() {
    await run(async () => {
      const result = await app.client.skills.exportBundle();
      if (result.status === 'saved')
        app.toasts.show({ kind: 'success', message: t('skills.exported', { path: result.path }) });
    });
  }

  async function importBundle() {
    await run(async () => {
      const result = await app.client.skills.importBundle();
      app.toasts.show({
        kind: 'info',
        message: t('skills.imported', {
          n: result.proposed.length,
          skipped: result.skipped.length,
        }),
      });
    });
  }

  async function propose() {
    manifestError = null;
    let parsed: unknown;
    try {
      parsed = JSON.parse(manifest);
    } catch {
      manifestError = t('skills.manifestInvalid');
      return;
    }
    await run(async () => {
      const s = await app.client.skills.propose(parsed);
      manifest = '';
      reviewing = s;
    }, t('skills.proposed'));
  }

  function label(s: SkillInfo): string {
    return `${s.name} ${s.version}`;
  }
</script>

<section class="wk-card" aria-labelledby="sk-title">
  <h3 id="sk-title">{t('skills.title')}</h3>
  <p>{t('skills.intro')}</p>
  <div class="wk-actions">
    <Button size="sm" variant="secondary" onclick={exportBundle}>{t('skills.export')}</Button>
    <Button size="sm" variant="secondary" onclick={importBundle}>{t('skills.import')}</Button>
  </div>
  {#if loadError}<LoadFailed error={loadError} onretry={() => void reload()} />{/if}
</section>

{#if reviewing}
  <SkillReview
    skill={reviewing}
    ondone={() => {
      reviewing = null;
      void reload();
    }}
  />
{/if}

{#if running}
  <SkillRun skill={running} ondone={() => (running = null)} />
{/if}

<section class="wk-card" aria-labelledby="sk-pending">
  <h3 id="sk-pending">{t('skills.pending')}</h3>
  {#if loaded && pending.length === 0}
    <p class="wk-meta">{t('skills.pendingEmpty')}</p>
  {:else if pending.length}
    <ul class="wk-list">
      {#each pending as s (`${s.id}@${s.version}`)}
        <li>
          <strong>{label(s)}</strong>
          <span class="wk-meta"
            >{tk(`skills.state.${s.state}`)} · {tk(`skills.origin.${s.origin}`)} · {app.i18n.dateTime(
              s.proposed_at,
            )}</span
          >
          <span>{s.description}</span>
          {#each s.findings as f (f)}<span class="wk-error">{f}</span>{/each}
          <div class="wk-actions">
            <Button
              size="sm"
              variant="primary"
              aria-label={`${t('skills.review')}: ${label(s)}`}
              onclick={() => (reviewing = s)}>{t('skills.review')}</Button
            >
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<section class="wk-card" aria-labelledby="sk-installed">
  <h3 id="sk-installed">{t('skills.installed')}</h3>
  {#if loaded && installed.length === 0}
    <p class="wk-meta">{t('skills.installedEmpty')}</p>
  {:else if installed.length}
    <ul class="wk-list">
      {#each installed as s (`${s.id}@${s.version}`)}
        <li>
          <strong>{label(s)}</strong>
          <span class="wk-meta"
            >{tk(`skills.origin.${s.origin}`)} · {s.required_tools.join(', ')}</span
          >
          <span>{s.description}</span>
          <div class="wk-actions">
            <Button
              size="sm"
              variant="secondary"
              disabled={!app.activeId}
              aria-label={`${t('skills.run')}: ${label(s)}`}
              onclick={() => (running = s)}>{t('skills.run')}</Button
            >
            <Button
              size="sm"
              variant="ghost"
              aria-label={`${t('skills.disable')}: ${label(s)}`}
              onclick={() => run(() => app.client.skills.disable(s.id), t('skills.disabled'))}
              >{t('skills.disable')}</Button
            >
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if other.length}
  <section class="wk-card" aria-labelledby="sk-other">
    <h3 id="sk-other">{t('skills.other')}</h3>
    <ul class="wk-plain wk-meta">
      {#each other as s (`${s.id}@${s.version}`)}
        <li>{label(s)} — {tk(`skills.state.${s.state}`)}</li>
      {/each}
    </ul>
  </section>
{/if}

<section class="wk-card" aria-labelledby="sk-own">
  <h3 id="sk-own">{t('skills.own')}</h3>
  <label class="wk-field">
    <span>{t('skills.manifest')}</span>
    <textarea
      rows="6"
      spellcheck="false"
      bind:value={manifest}
      aria-invalid={manifestError ? 'true' : undefined}></textarea>
    {#if manifestError}<span class="wk-error" role="alert">{manifestError}</span>{/if}
  </label>
  <div class="wk-actions">
    <Button size="sm" variant="secondary" disabled={!manifest.trim()} onclick={propose}
      >{t('skills.propose')}</Button
    >
  </div>
</section>
