<!--
  Formularz serwera MCP (dodanie z przepisu albo własny, edycja). Argumenty — jeden w linii;
  przepis z folderem pokazuje osobne pole „Folder”, które zastępuje `{folder}`. Wartości tajne nie
  wracają z rdzenia: pole puste przy edycji = wartość zapisana zostaje.
-->
<script lang="ts">
  import { Button, Checkbox, IconButton, Select, Switch, TextField } from '@alfa/ui-kit';
  import X from '@lucide/svelte/icons/x';
  import type { McpTrust } from '../../../api/types-mcp';
  import { formProblems, toInput, type McpForm } from '../../../logic/mcp';
  import { useApp } from '../../../state/context';
  import './work.css';

  interface Props {
    form: McpForm;
    /** Przepis z folderem: tekst argumentów zawiera `{folder}`. */
    folderSlot?: boolean;
    onsaved: (name: string) => void;
    oncancel: () => void;
  }

  let { form = $bindable(), folderSlot = false, onsaved, oncancel }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  let folder = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);

  const TRUST: readonly McpTrust[] = ['untrusted', 'reviewed', 'trusted'];
  const trustOptions = $derived(TRUST.map((v) => ({ value: v, label: tk(`mcp.trustShort.${v}`) })));
  // `{folder}` w argumentach zastępowany wartością pola „Folder” dopiero przy zapisie.
  const effective = $derived<McpForm>(
    folderSlot && folder.trim()
      ? { ...form, argsText: form.argsText.replace('{folder}', folder.trim()) }
      : form,
  );
  const problems = $derived(formProblems(effective));

  function addVar() {
    form.env = [...form.env, { name: '', value: '', secret: false }];
  }

  function removeVar(i: number) {
    form.env = form.env.filter((_, j) => j !== i);
  }

  async function save() {
    busy = true;
    error = null;
    try {
      const saved = await app.client.mcp.save(toInput(effective));
      onsaved(saved.name);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="wk-card wk-accent" aria-labelledby="mcp-form-title">
  <h3 id="mcp-form-title">
    {form.id ? t('mcp.form.titleEdit', { name: form.name }) : t('mcp.form.titleNew')}
  </h3>
  <div class="wk-grid">
    <TextField label={t('mcp.form.name')} bind:value={form.name} maxlength={80} />
    <TextField
      label={t('mcp.form.command')}
      hint={t('mcp.form.commandHint')}
      bind:value={form.command}
      spellcheck="false"
    />
  </div>
  {#if folderSlot}
    <TextField
      label={t('mcp.form.folder')}
      hint={t('mcp.form.folderHint')}
      placeholder="C:\Users\…\Projekty"
      bind:value={folder}
      spellcheck="false"
    />
  {/if}
  <label class="wk-field">
    <span>{t('mcp.form.args')}</span>
    <textarea rows="3" spellcheck="false" bind:value={form.argsText}></textarea>
  </label>
  <TextField label={t('mcp.form.cwd')} bind:value={form.cwd} spellcheck="false" />

  <h4>{t('mcp.form.env')}</h4>
  <p class="wk-meta">{t('mcp.form.envHint')}</p>
  {#each form.env as v, i (i)}
    <div class="env-row">
      <TextField
        aria-label={t('mcp.form.envName')}
        placeholder={t('mcp.form.envName')}
        bind:value={v.name}
        spellcheck="false"
      />
      <TextField
        aria-label={t('mcp.form.envValue')}
        placeholder={v.secret && v.value === null && form.id
          ? t('mcp.form.envSecretKept')
          : t('mcp.form.envValue')}
        type={v.secret ? 'password' : 'text'}
        autocomplete="off"
        value={v.value ?? ''}
        oninput={(e) => (v.value = (e.currentTarget as HTMLInputElement).value)}
      />
      <Checkbox
        label={t('mcp.form.envSecret')}
        checked={v.secret}
        onchange={(on) => (v.secret = on)}
      />
      <IconButton
        label={t('mcp.form.envRemove', { name: v.name || '…' })}
        onclick={() => removeVar(i)}><X size={14} /></IconButton
      >
    </div>
  {/each}
  <div class="wk-actions">
    <Button size="sm" variant="secondary" onclick={addVar}>{t('mcp.form.envAdd')}</Button>
  </div>

  <div class="wk-grid">
    <div class="wk-field">
      <span id="mcp-form-trust">{t('mcp.form.trust')}</span>
      <Select
        labelledby="mcp-form-trust"
        value={form.trust}
        options={trustOptions}
        onchange={(v) => (form.trust = v as McpTrust)}
      />
      <span class="wk-meta">{tk(`mcp.trust.${form.trust}`)}</span>
    </div>
    <TextField
      label={t('mcp.form.timeout')}
      type="number"
      min={1}
      max={600}
      value={form.timeoutSec}
      oninput={(e) => (form.timeoutSec = Number((e.currentTarget as HTMLInputElement).value) || 30)}
    />
  </div>
  <div class="wk-actions">
    <span id="mcp-form-enabled">{t('mcp.form.enabled')}</span>
    <Switch
      labelledby="mcp-form-enabled"
      checked={form.enabled}
      onchange={(on) => (form.enabled = on)}
    />
  </div>

  {#if problems.length}
    <p class="wk-meta" role="status">{problems.map((p) => tk(p)).join(' ')}</p>
  {/if}
  {#if error}<p class="wk-error" role="alert">{error}</p>{/if}
  <div class="wk-actions">
    <Button variant="primary" disabled={busy || problems.length > 0} onclick={() => void save()}
      >{t('mcp.form.save')}</Button
    >
    <Button variant="secondary" disabled={busy} onclick={oncancel}>{t('mcp.form.cancel')}</Button>
  </div>
</section>

<style>
  .env-row {
    display: grid;
    grid-template-columns: minmax(8rem, 1fr) minmax(8rem, 1.4fr) auto auto;
    align-items: center;
    gap: var(--alfa-space-2);
  }
</style>
