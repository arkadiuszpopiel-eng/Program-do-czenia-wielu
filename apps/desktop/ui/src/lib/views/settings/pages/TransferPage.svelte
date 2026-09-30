<!--
  Import i eksport `.alfa` (PLAN §15.1, makieta 13). Eksport: zakres, hasło, sekrety nigdy.
  Import: dry-run (podgląd różnic) → tryb dodaj / scal / zastąp → kolizje → snapshot i rollback.
  Operacje plikowe to intencje (natywne dialogi w rdzeniu).
-->
<script lang="ts">
  import {
    Button,
    Checkbox,
    ConfirmDialog,
    SegmentedControl,
    Select,
    Switch,
    TextField,
  } from '@alfa/ui-kit';
  import type {
    CollisionResolution,
    ImportMode,
    ImportResult,
    InspectResult,
  } from '../../../api/types-hub';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const LATER = ['memory', 'artifacts', 'logs', 'config_machine'] as const;

  let common = $state(true);
  let personas = $state(true);
  let casts = $state(true);
  let chosen = $state<string[]>(app.activeId ? [app.activeId] : []);
  let encrypt = $state(false);
  let password = $state('');
  let repeat = $state('');
  let exported = $state<string | null>(null);

  let inspected = $state<Extract<InspectResult, { status: 'inspected' }> | null>(null);
  let lockedPath = $state<string | null>(null);
  let importPassword = $state('');
  let mode = $state<string>('merge');
  let resolutions = $state<Record<string, CollisionResolution>>({});
  let result = $state<ImportResult | null>(null);
  let confirmOpen = $state(false);

  const mismatch = $derived(encrypt && repeat.length > 0 && password !== repeat);
  const canExport = $derived(!encrypt || (password.length >= 8 && password === repeat));

  async function doExport() {
    const res = await app.client.transfer.exportPackage({
      scope: {
        config_common: common,
        personas,
        casts,
        sessions: chosen,
        artifacts: false,
        logs: false,
        config_machine: false,
      },
      password: encrypt ? password : null,
    });
    password = '';
    repeat = '';
    if (res.status === 'saved') {
      exported = t('tr.exported', {
        path: res.path,
        files: t('tr.files', { n: res.files }),
        size: app.i18n.bytes(res.bytes),
      });
    }
  }

  async function inspect(path: string | null = null) {
    const res = await app.client.transfer.inspect(importPassword || null, path);
    result = null;
    if (res.status === 'needs_password') lockedPath = res.path;
    else if (res.status === 'inspected') {
      lockedPath = null;
      inspected = res;
      resolutions = Object.fromEntries(
        res.items.filter((i) => i.diff === 'collision').map((i) => [i.key, 'keep_both' as const]),
      );
    }
  }

  async function doImport() {
    if (!inspected) return;
    result = await app.client.transfer.importPackage({
      path: inspected.path,
      mode: mode as ImportMode,
      resolutions,
      password: importPassword || null,
    });
    importPassword = '';
  }

  async function rollback() {
    if (!result) return;
    await app.client.transfer.rollback(result.snapshot_id);
    result = null;
    app.toasts.show({ kind: 'success', message: t('tr.rolledBack') });
  }
</script>

<section class="card" aria-labelledby="tr-export">
  <h3 id="tr-export">{t('tr.export')}</h3>
  <p class="desc">{t('tr.exportIntro')}</p>
  <fieldset>
    <legend>{t('tr.scope')}</legend>
    <Checkbox label={t('tr.scope.config_common')} bind:checked={common} />
    <Checkbox label={t('tr.scope.personas')} bind:checked={personas} />
    <Checkbox label={t('tr.scope.casts')} bind:checked={casts} />
    {#each LATER as key (key)}
      <Checkbox label={t(`tr.scope.${key}`)} description={t('tr.f7')} disabled />
    {/each}
  </fieldset>
  <fieldset>
    <legend>{t('tr.scope.sessions')}</legend>
    <div class="sessions">
      {#each app.sessions.list.filter((s) => !s.archived) as s (s.id)}
        <Checkbox
          label={s.title}
          checked={chosen.includes(s.id)}
          onchange={(on) => (chosen = on ? [...chosen, s.id] : chosen.filter((x) => x !== s.id))}
        />
      {/each}
    </div>
  </fieldset>
  <p class="note">{t('tr.secrets')}</p>
  <div class="row">
    <span id="tr-encrypt">{t('tr.encrypt')}</span>
    <Switch bind:checked={encrypt} labelledby="tr-encrypt" />
  </div>
  {#if encrypt}
    <div class="pw">
      <TextField
        label={t('tr.password')}
        type="password"
        autocomplete="new-password"
        bind:value={password}
      />
      <TextField
        label={t('tr.passwordRepeat')}
        type="password"
        autocomplete="new-password"
        bind:value={repeat}
        error={mismatch ? t('tr.passwordMismatch') : undefined}
      />
    </div>
  {/if}
  <div class="actions">
    <Button variant="primary" disabled={!canExport} onclick={doExport}
      >{t('tr.exportButton')}</Button
    >
  </div>
  {#if exported}<p class="ok" role="status">{exported}</p>{/if}
</section>

<section class="card" aria-labelledby="tr-import">
  <h3 id="tr-import">{t('tr.import')}</h3>
  <p class="desc">{t('tr.importIntro')}</p>
  <div class="actions start">
    <Button variant="secondary" onclick={() => inspect()}>{t('tr.choose')}</Button>
  </div>
  {#if lockedPath}
    <div class="pw">
      <TextField label={t('tr.needsPassword')} type="password" bind:value={importPassword} />
      <Button variant="secondary" onclick={() => inspect(lockedPath)}>{t('tr.unlock')}</Button>
    </div>
  {/if}
  {#if inspected}
    <p class="manifest">
      {t('tr.manifest', {
        machine: inspected.manifest.source_machine,
        date: app.i18n.dateTime(inspected.manifest.created_at),
        schema: inspected.manifest.schema_version,
      })}
    </p>
    <ul class="diff">
      {#each inspected.items as item (item.key)}
        <li class="diff-row diff-{item.diff}">
          <span class="diff-label">{item.label}</span>
          <span class="badge">{t(`tr.diff.${item.diff}`)}</span>
          {#if item.diff === 'collision'}
            <Select
              size="sm"
              label={t('tr.collision', { label: item.label })}
              value={resolutions[item.key] ?? 'keep_both'}
              options={(['keep_local', 'take_imported', 'keep_both'] as const).map((r) => ({
                value: r,
                label: t(`tr.res.${r}`),
              }))}
              onchange={(v) => (resolutions[item.key] = v as CollisionResolution)}
            />
          {/if}
        </li>
      {/each}
    </ul>
    {#if inspected.warnings.length}
      <div class="warnings">
        <strong>{t('tr.warnings')}</strong>
        <ul>
          {#each inspected.warnings as w (w)}<li>{w}</li>{/each}
        </ul>
      </div>
    {/if}
    <SegmentedControl
      label={t('tr.mode')}
      variant="cards"
      bind:value={mode}
      options={(['add', 'merge', 'replace'] as const).map((m) => ({
        value: m,
        label: t(`tr.mode.${m}`),
        description: t(`tr.mode.${m}.desc`),
      }))}
    />
    <div class="actions">
      <Button
        variant="primary"
        onclick={() => (mode === 'replace' ? (confirmOpen = true) : void doImport())}
        >{t('tr.importButton')}</Button
      >
    </div>
  {/if}
  {#if result}
    <p class="ok" role="status">
      {t('tr.imported', { n: result.imported })} Snapshot: {result.snapshot_id}
    </p>
    <div class="actions start">
      <Button variant="secondary" onclick={rollback}>{t('tr.rollback')}</Button>
    </div>
  {/if}
</section>

<ConfirmDialog
  bind:open={confirmOpen}
  title={t('tr.replaceTitle')}
  description={t('tr.replaceConfirm')}
  confirmLabel={t('tr.mode.replace')}
  cancelLabel={t('common.cancel')}
  danger
  onconfirm={() => void doImport()}
/>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin-bottom: var(--alfa-space-4);
    padding: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  h3 {
    font-size: var(--alfa-font-size-md);
  }
  .desc,
  .manifest {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .note {
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
    font-size: var(--alfa-font-size-sm);
  }
  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: 0;
    padding: var(--alfa-space-2) var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
  }
  legend {
    padding: 0 var(--alfa-space-1);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .sessions {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: var(--alfa-space-1) var(--alfa-space-3);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--alfa-font-size-sm);
  }
  .pw {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-3);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--alfa-space-2);
  }
  .actions.start {
    justify-content: flex-start;
  }
  .ok {
    color: var(--alfa-color-success);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .diff {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .diff-row {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    min-height: 36px;
    border-bottom: 1px solid var(--alfa-color-border);
    font-size: var(--alfa-font-size-sm);
  }
  .diff-label {
    flex: 1;
  }
  .badge {
    padding: 1px var(--alfa-space-2);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  .diff-new .badge {
    color: var(--alfa-color-success);
  }
  .diff-changed .badge {
    color: var(--alfa-color-info);
  }
  .diff-collision .badge {
    color: var(--alfa-color-warning);
  }
  .warnings {
    font-size: var(--alfa-font-size-sm);
  }
  .warnings ul {
    margin: var(--alfa-space-1) 0 0;
    padding-left: var(--alfa-space-4);
  }
</style>
