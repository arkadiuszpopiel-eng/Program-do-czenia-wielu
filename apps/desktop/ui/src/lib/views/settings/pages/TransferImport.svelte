<!--
  Import `.alfa` (PLAN §15.1, makieta 13): dry-run (podgląd różnic) → tryb dodaj / scal / zastąp →
  kolizje → snapshot i rollback. Plik z natywnego dialogu albo z listy kopii („Przywróć…”) —
  zawsze jako jednorazowy uchwyt wydany przez rdzeń (UI nigdy nie podaje ścieżki).
-->
<script lang="ts">
  import { Button, ConfirmDialog, SegmentedControl, Select, TextField } from '@alfa/ui-kit';
  import type {
    CollisionResolution,
    ImportMode,
    ImportResult,
    InspectResult,
  } from '../../../api/types-hub';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t } = app.i18n;

  let inspected = $state<Extract<InspectResult, { status: 'inspected' }> | null>(null);
  let lockedHandle = $state<string | null>(null);
  let importPassword = $state('');
  let mode = $state<string>('merge');
  let resolutions = $state<Record<string, CollisionResolution>>({});
  let result = $state<ImportResult | null>(null);
  let confirmOpen = $state(false);
  let section = $state<HTMLElement | null>(null);

  function fail(error: unknown) {
    app.toasts.show({
      kind: 'error',
      message: error instanceof Error ? error.message : String(error),
    });
  }

  /** Podgląd paczki (`handle: null` — natywny dialog; inaczej jednorazowy uchwyt z rdzenia). */
  async function inspect(handle: string | null = null) {
    let res: InspectResult;
    try {
      res = await app.client.transfer.inspect(importPassword || null, handle);
    } catch (error) {
      lockedHandle = null;
      fail(error);
      return;
    }
    result = null;
    if (res.status === 'needs_password') {
      // Hasło podane, a paczka nadal zamknięta — złe hasło (rdzeń wydał nowy uchwyt).
      if (importPassword) app.toasts.show({ kind: 'error', message: t('tr.wrongPassword') });
      lockedHandle = res.handle;
      inspected = null;
    } else if (res.status === 'inspected') {
      lockedHandle = null;
      inspected = res;
      resolutions = Object.fromEntries(
        res.items.filter((i) => i.diff === 'collision').map((i) => [i.key, 'keep_both' as const]),
      );
    }
    if (handle) section?.scrollIntoView({ block: 'start' });
  }

  /** „Wybierz plik…”: natywny dialog (nowy plik — hasło od nowa). */
  function choose() {
    importPassword = '';
    void inspect();
  }

  /** „Przywróć…” z kopii zapasowych: uchwyt kopii z listy (ważny 15 s) → podgląd od razu. */
  export async function restore(file: string) {
    importPassword = '';
    try {
      await inspect(await app.client.backups.restore(file));
    } catch (error) {
      fail(error);
    }
  }

  async function doImport() {
    if (!inspected) return;
    const handle = inspected.handle;
    // Uchwyt jest jednorazowy — po próbie importu podgląd trzeba otworzyć ponownie.
    inspected = null;
    try {
      result = await app.client.transfer.importPackage({
        handle,
        mode: mode as ImportMode,
        resolutions,
        password: importPassword || null,
      });
    } catch (error) {
      fail(error);
    }
    importPassword = '';
  }

  async function rollback() {
    if (!result) return;
    await app.client.transfer.rollback(result.snapshot_id);
    result = null;
    app.toasts.show({ kind: 'success', message: t('tr.rolledBack') });
  }
</script>

<section class="card" aria-labelledby="tr-import" bind:this={section}>
  <h3 id="tr-import">{t('tr.import')}</h3>
  <p class="desc">{t('tr.importIntro')}</p>
  <div class="actions start">
    <Button variant="secondary" onclick={choose}>{t('tr.choose')}</Button>
  </div>
  {#if lockedHandle}
    <div class="pw">
      <TextField label={t('tr.needsPassword')} type="password" bind:value={importPassword} />
      <Button variant="secondary" onclick={() => inspect(lockedHandle)}>{t('tr.unlock')}</Button>
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
