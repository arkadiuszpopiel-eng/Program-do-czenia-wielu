<!--
  Wprowadzenie → import `.alfa` (opcjonalnie): natywny dialog → podgląd różnic → import w trybie
  „scal". Błąd rdzenia (dialog, podgląd, import) daje toast i zostawia przycisk do ponowienia.
-->
<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { InspectResult } from '../../api/types-hub';
  import { attempt } from '../../state/attempt';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let imported = $state<Extract<InspectResult, { status: 'inspected' }> | null>(null);
  let importDone = $state(false);
  let busy = $state(false);

  async function chooseImport() {
    if (busy) return;
    busy = true;
    await attempt(app.toasts, async () => {
      const res = await app.client.transfer.inspect(null, null);
      if (res.status === 'inspected') imported = res;
    });
    busy = false;
  }

  async function runImport() {
    const target = imported;
    if (!target || busy) return;
    busy = true;
    const ok = await attempt(app.toasts, () =>
      app.client.transfer.importPackage({
        handle: target.handle,
        mode: 'merge',
        resolutions: {},
        password: null,
      }),
    );
    if (ok) {
      importDone = true;
      await attempt(app.toasts, async () => {
        app.sessions.list = [...(await app.client.sessions.list())];
      });
    } else {
      // Uchwyt podglądu jest jednorazowy — po nieudanym imporcie plik trzeba wybrać ponownie.
      imported = null;
    }
    busy = false;
  }
</script>

<h2>{t('ob.import.title')}</h2>
<p class="muted">{t('ob.import.desc')}</p>
{#if imported}
  <p class="muted small">
    {t('tr.manifest', {
      machine: imported.manifest.source_machine,
      date: app.i18n.dateTime(imported.manifest.created_at),
      schema: imported.manifest.schema_version,
    })}
  </p>
  <ul class="list">
    {#each imported.items as item (item.key)}<li>
        {item.label} — {t(`tr.diff.${item.diff}`)}
      </li>{/each}
  </ul>
  {#if importDone}
    <p class="ok" role="status">{t('tr.imported', { n: imported.items.length })}</p>
  {:else}
    <Button variant="primary" loading={busy} disabled={busy} onclick={runImport}
      >{t('tr.importButton')} ({t('tr.mode.merge')})</Button
    >
  {/if}
{:else}
  <Button variant="secondary" loading={busy} disabled={busy} onclick={chooseImport}
    >{t('tr.choose')}</Button
  >
{/if}

<style>
  h2 {
    font-size: var(--alfa-font-size-xl);
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .small {
    font-size: var(--alfa-font-size-sm);
  }
  .ok {
    color: var(--alfa-color-success);
    font-weight: var(--alfa-weight-semibold);
  }
  .list {
    margin: 0;
    padding-left: var(--alfa-space-4);
    font-size: var(--alfa-font-size-sm);
  }
</style>
