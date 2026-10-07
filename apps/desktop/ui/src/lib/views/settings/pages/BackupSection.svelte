<!--
  Kopie zapasowe (PLAN §15.1, docs/UI.md §13.3): zaplanowany eksport `.alfa` do katalogu wybranego
  natywnym dialogiem, rotacja N najnowszych, zakres (artefakty, logi), nie na baterii, hasło kopii
  w Credential Managerze (szyfrowanie, sesje prywatne). „Sprawdź” = test przywracania (bez zapisu),
  „Przywróć…” = podgląd importu tej kopii (rdzeń wydaje jednorazowy uchwyt pliku — UI nie podaje
  ścieżki). Sekrety nigdy nie trafiają do kopii.
-->
<script lang="ts">
  import { Button, Checkbox, Select, Switch, TextField } from '@alfa/ui-kit';
  import type { BackupCheck, BackupConfig, BackupView } from '../../../api/types-files';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import { attempt, load, showError } from '../../../state/attempt';
  import { useApp } from '../../../state/context';
  import './work.css';

  interface Props {
    /** „Przywróć…”: nazwa pliku kopii z listy (uchwyt wydaje rdzeń). */
    onrestore: (file: string) => void;
  }

  let { onrestore }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const INTERVALS = [6, 12, 24, 48, 168];
  const KEEP = [3, 5, 7, 14, 30];

  let view = $state<BackupView | null>(null);
  let config = $state<BackupConfig | null>(null);
  let password = $state('');
  let busy = $state(false);
  let check = $state<BackupCheck | null>(null);
  let loadError = $state<string | null>(null);

  function show(next: BackupView) {
    view = next;
    config = { ...next.config };
  }

  /** Akcja na kopiach; `true` po sukcesie (pola czyści się tylko wtedy), błąd → toast. */
  async function act(action: () => Promise<BackupView>, done?: string): Promise<boolean> {
    const ok = await attempt(app.toasts, async () => show(await action()));
    if (ok && done) app.toasts.show({ kind: 'success', message: done });
    return ok;
  }

  async function loadStatus() {
    const result = await load(() => app.client.backups.status());
    if (result.status === 'ready') {
      show(result.value);
      loadError = null;
    } else if (result.status === 'failed') loadError = result.error;
  }

  $effect(() => {
    void loadStatus();
  });

  function options(values: readonly number[], current: number, label: (n: number) => string) {
    const all = values.includes(current) ? values : [...values, current].sort((a, b) => a - b);
    return all.map((n) => ({ value: String(n), label: label(n) }));
  }

  function patch(change: Partial<BackupConfig>) {
    if (config) config = { ...config, ...change };
  }

  async function save() {
    const current = config;
    if (current) await act(() => app.client.backups.configure(current), t('bk.saved'));
  }

  async function runNow() {
    busy = true;
    await act(() => app.client.backups.runNow(), t('bk.created'));
    busy = false;
  }

  async function setPassword(next: string | null) {
    // Po błędzie hasło zostaje w polu — użytkownik ponawia bez przepisywania.
    if (await act(() => app.client.backups.setPassword(next))) password = '';
  }

  async function verify(file: string) {
    try {
      check = await app.client.backups.verify(file);
    } catch (error) {
      showError(app.toasts, error);
    }
  }
</script>

<section class="wk-card" aria-labelledby="bk-title">
  <h3 id="bk-title">{t('bk.title')}</h3>
  <p class="wk-meta">{t('bk.intro')}</p>
  {#if loadError && !view}
    <LoadFailed error={loadError} onretry={() => void loadStatus()} />
  {:else if view && config}
    <div class="wk-actions">
      <span>{t('bk.dir')}:</span>
      <span class="wk-code">{view.config.dir ?? t('bk.noDir')}</span>
      <Button size="sm" onclick={() => void act(() => app.client.backups.chooseDir())}
        >{t('bk.chooseDir')}</Button
      >
    </div>
    <div class="row">
      <span id="bk-enabled">{t('bk.enabled')}</span>
      <Switch
        labelledby="bk-enabled"
        checked={config.enabled}
        disabled={!view.config.dir}
        onchange={(on) => patch({ enabled: on })}
      />
    </div>
    <div class="wk-grid">
      <Select
        label={t('bk.interval')}
        value={String(config.interval_hours)}
        options={options(INTERVALS, config.interval_hours, (n) => t('bk.hours', { n }))}
        onchange={(v) => patch({ interval_hours: Number(v) })}
      />
      <Select
        label={t('bk.keep')}
        value={String(config.keep)}
        options={options(KEEP, config.keep, (n) => String(n))}
        onchange={(v) => patch({ keep: Number(v) })}
      />
    </div>
    <Checkbox
      label={t('bk.artifacts')}
      checked={config.include_artifacts}
      onchange={(on) => patch({ include_artifacts: on })}
    />
    <Checkbox
      label={t('bk.logs')}
      checked={config.include_logs}
      onchange={(on) => patch({ include_logs: on })}
    />
    <Checkbox
      label={t('bk.battery')}
      checked={config.skip_on_battery}
      onchange={(on) => patch({ skip_on_battery: on })}
    />
    <div class="wk-actions">
      <Button onclick={() => void save()}>{t('bk.save')}</Button>
    </div>

    <p class="wk-meta">{t(view.password_set ? 'bk.passwordSet' : 'bk.passwordNone')}</p>
    <div class="pw">
      <TextField
        label={t('bk.password')}
        type="password"
        autocomplete="new-password"
        bind:value={password}
      />
      <Button disabled={password.length < 8} onclick={() => void setPassword(password)}
        >{t('bk.setPassword')}</Button
      >
      {#if view.password_set}
        <Button variant="ghost" onclick={() => void setPassword(null)}
          >{t('bk.clearPassword')}</Button
        >
      {/if}
    </div>

    <p>
      {view.last_run ? t('bk.last', { date: app.i18n.dateTime(view.last_run) }) : t('bk.never')}
      {#if view.next_due}· {t('bk.next', { date: app.i18n.dateTime(view.next_due) })}{/if}
    </p>
    {#if view.last_error}<p class="wk-error" role="alert">
        {t('bk.error', { error: view.last_error })}
      </p>{/if}
    <div class="wk-actions">
      <Button
        variant="primary"
        disabled={!view.config.dir || busy || view.running}
        loading={busy}
        onclick={() => void runNow()}>{busy ? t('bk.running') : t('bk.runNow')}</Button
      >
    </div>

    <h4>{t('bk.list')}</h4>
    <ul class="wk-list">
      {#each view.entries as e (e.file)}
        <li>
          <div class="entry">
            <span>{app.i18n.dateTime(e.created_at)}</span>
            <span class="wk-meta">{app.i18n.bytes(e.bytes)}</span>
            <span class="wk-actions">
              <Button
                size="sm"
                aria-label={t('bk.verifyLabel', { file: e.file })}
                onclick={() => void verify(e.file)}>{t('bk.verify')}</Button
              >
              <Button
                size="sm"
                aria-label={t('bk.restoreLabel', { file: e.file })}
                onclick={() => onrestore(e.file)}>{t('bk.restore')}</Button
              >
            </span>
          </div>
        </li>
      {:else}
        <li class="wk-meta">{t('bk.empty')}</li>
      {/each}
    </ul>
    {#if check}
      <p class={check.ok ? 'wk-ok' : 'wk-error'} role="status">
        {check.ok
          ? t('bk.verifyOk', {
              file: check.file,
              sessions: t('bk.sessions', { n: check.sessions }),
              items: t('bk.items', { n: check.items }),
            })
          : t('bk.verifyFail', { file: check.file, message: check.message ?? '' })}
      </p>
    {/if}
  {/if}
</section>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .pw {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-3);
  }
  .entry {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-3);
  }
</style>
