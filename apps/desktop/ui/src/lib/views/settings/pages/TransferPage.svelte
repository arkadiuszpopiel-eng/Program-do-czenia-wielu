<!--
  Import i eksport `.alfa` (PLAN §15.1, makieta 13). Eksport: zakres (także artefakty wybranych
  sesji, logi i nakładka tej maszyny), hasło, sekrety nigdy. Import: `TransferImport`. Kopie
  zapasowe z harmonogramem: `BackupSection` („Przywróć…” otwiera podgląd importu kopii).
  Operacje plikowe to intencje (natywne dialogi w rdzeniu).
-->
<script lang="ts">
  import { Button, Checkbox, Switch, TextField } from '@alfa/ui-kit';
  import type { MemoryScopeInfo } from '../../../api/types-memory';
  import { useApp } from '../../../state/context';
  import BackupSection from './BackupSection.svelte';
  import TransferImport from './TransferImport.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const EXTRA = ['artifacts', 'logs', 'config_machine'] as const;

  let common = $state(true);
  let personas = $state(true);
  let casts = $state(true);
  let extra = $state<Record<(typeof EXTRA)[number], boolean>>({
    artifacts: false,
    logs: false,
    config_machine: false,
  });
  let chosen = $state<string[]>(app.activeId ? [app.activeId] : []);
  let memoryScopes = $state<readonly MemoryScopeInfo[]>([]);
  let memory = $state<string[]>([]);
  let importer = $state<ReturnType<typeof TransferImport> | null>(null);

  $effect(() => {
    void app.client.memory.scopes().then((list) => {
      memoryScopes = list.filter((s) => s.document !== null);
    });
  });
  let encrypt = $state(false);
  let password = $state('');
  let repeat = $state('');
  let exported = $state<string | null>(null);

  const mismatch = $derived(encrypt && repeat.length > 0 && password !== repeat);
  const canExport = $derived(!encrypt || (password.length >= 8 && password === repeat));

  async function doExport() {
    const res = await app.client.transfer.exportPackage({
      scope: {
        config_common: common,
        personas,
        casts,
        sessions: chosen,
        artifacts: extra.artifacts,
        logs: extra.logs,
        config_machine: extra.config_machine,
        memory,
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
</script>

<section class="card" aria-labelledby="tr-export">
  <h3 id="tr-export">{t('tr.export')}</h3>
  <p class="desc">{t('tr.exportIntro')}</p>
  <fieldset>
    <legend>{t('tr.scope')}</legend>
    <Checkbox label={t('tr.scope.config_common')} bind:checked={common} />
    <Checkbox label={t('tr.scope.personas')} bind:checked={personas} />
    <Checkbox label={t('tr.scope.casts')} bind:checked={casts} />
    {#each EXTRA as key (key)}
      <Checkbox
        label={t(`tr.scope.${key}`)}
        description={t(`tr.scope.${key}.desc`)}
        bind:checked={extra[key]}
      />
    {/each}
  </fieldset>
  <fieldset>
    <legend>{t('tr.scope.memory')}</legend>
    <p class="note">{t('tr.memoryHint')}</p>
    {#each memoryScopes as s (s.key)}
      <Checkbox
        label={`${app.i18n.tk(`memory.scopeKind.${s.scope.kind}`)}: ${s.label}`}
        checked={memory.includes(s.key)}
        onchange={(on) => (memory = on ? [...memory, s.key] : memory.filter((x) => x !== s.key))}
      />
    {:else}
      <p class="note">{t('tr.memoryEmpty')}</p>
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

<TransferImport bind:this={importer} />

<BackupSection onrestore={(file) => void importer?.restore(file)} />

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
  .desc {
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
  .ok {
    color: var(--alfa-color-success);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
</style>
