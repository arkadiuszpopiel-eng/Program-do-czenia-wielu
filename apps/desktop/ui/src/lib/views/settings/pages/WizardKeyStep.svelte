<!--
  Krok „Klucz" kreatora dostawcy (i wymiana klucza istniejącego konta): skąd wziąć klucz, pole
  z podglądem na życzenie i ocena wklejonego tekstu na bieżąco — sprzątanie tego, co dokleja
  schowek, klucz innego dostawcy, zdanie zamiast klucza. O tym, czy klucz działa, rozstrzyga
  dopiero test u dostawcy; tu blokujemy tylko to, co kluczem na pewno nie jest.
-->
<script lang="ts">
  import { Button, Checkbox, TextField } from '@alfa/ui-kit';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import KeyRound from '@lucide/svelte/icons/key-round';
  import type { ProviderInfo } from '../../../api/types-hub';
  import {
    KEY_HELP,
    PROVIDER_NAMES,
    blocksSubmit,
    checkKey,
    cleanKey,
  } from '../../../logic/api-key';
  import { useApp } from '../../../state/context';

  interface Props {
    provider: ProviderInfo;
    secret: string;
    label: string;
    baseUrl: string;
    saving: boolean;
    /** Odmowa rdzenia przy zapisie (np. ten sam klucz dodany drugi raz). */
    error: string;
    onsubmit: () => void;
    /** Wymiana klucza istniejącego konta: bez nazwy i adresu, z własnym tytułem. */
    rotate?: boolean;
    title?: string;
    submitLabel?: string;
  }

  let {
    provider,
    secret = $bindable(),
    label = $bindable(),
    baseUrl = $bindable(),
    saving,
    error,
    onsubmit,
    rotate = false,
    title,
    submitLabel,
  }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const uid = $props.id();

  let reveal = $state(false);
  let copied = $state(false);
  let field = $state<HTMLInputElement | null>(null);
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  const help = $derived(KEY_HELP[provider.id]);
  const check = $derived(checkKey(provider.id, secret));
  /** Schowek dokleił coś wokół klucza — mówimy o tym, zamiast po cichu zmieniać pole. */
  const tidied = $derived(!blocksSubmit(check) && secret.length > 0 && cleanKey(secret) !== secret);
  /** Własny endpoint jest obowiązkowy — bez niego rdzeń i tak odmówi. */
  const needsUrl = $derived(provider.needs_base_url && !rotate);
  const urlMissing = $derived(needsUrl && !/^https?:\/\/\S+/.test(baseUrl.trim()));
  const ready = $derived(!blocksSubmit(check) && !urlMissing && !saving);
  const providerName = (id: string) => PROVIDER_NAMES[id] ?? id;

  // Po wejściu na krok fokus trafia do pola klucza (przycisk poprzedniego kroku już nie istnieje).
  $effect(() => {
    field?.focus();
  });
  $effect(() => () => clearTimeout(copyTimer));

  async function copyUrl() {
    if (!help) return;
    try {
      await navigator.clipboard.writeText(help.url);
      copied = true;
      clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = false), 2000);
    } catch {
      // Adres jest widoczny obok — da się go zaznaczyć i skopiować ręcznie.
    }
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (ready) onsubmit();
  }
</script>

<form class="key" onsubmit={submit}>
  <h4>
    <KeyRound size={16} strokeWidth={1.5} aria-hidden="true" />
    {title ?? t('wiz.keyTitle', { name: provider.display_name })}
  </h4>

  {#if help}
    <section class="how" aria-labelledby="{uid}-how">
      <h5 id="{uid}-how">{t('wiz.howTitle')}</h5>
      <ol>
        <li>
          {t('wiz.how.open')}
          <span class="url">
            <code>{help.url}</code>
            <Button variant="secondary" size="sm" onclick={copyUrl}
              >{copied ? t('wiz.how.copied') : t('wiz.how.copy')}</Button
            >
          </span>
        </li>
        <li>{t('wiz.how.create')}</li>
        <li>{t('wiz.how.paste')}</li>
      </ol>
      {#if help.separateBilling}
        <p class="note">{t('wiz.how.billing', { name: provider.display_name })}</p>
      {/if}
    </section>
  {/if}

  <TextField
    label={t('wiz.key')}
    type={reveal ? 'text' : 'password'}
    bind:value={secret}
    bind:input={field}
    hint={t('wiz.keyHint')}
    autocomplete="off"
    spellcheck={false}
  />
  <div class="row">
    <Checkbox label={t('wiz.showKey')} bind:checked={reveal} />
  </div>

  <!-- Jeden komunikat naraz; treść nie zmienia się przy każdym znaku (czytniki ekranu). -->
  <div class="feedback" role="status">
    {#if check.kind === 'ok'}
      <p class="good">
        <CircleCheck size={14} strokeWidth={1.75} aria-hidden="true" />
        {t('wiz.check.ok')}
      </p>
    {:else if check.kind === 'not_a_key'}
      <p class="bad">
        <CircleAlert size={14} strokeWidth={1.75} aria-hidden="true" />
        {t('wiz.check.notKey')}
      </p>
    {:else if check.kind === 'cli_token'}
      <p class="bad">
        <CircleAlert size={14} strokeWidth={1.75} aria-hidden="true" />
        {t('wiz.check.cliToken')}
      </p>
    {:else if check.kind === 'other_provider'}
      <p class="warn">
        <CircleAlert size={14} strokeWidth={1.75} aria-hidden="true" />
        {t('wiz.check.other', {
          other: providerName(check.provider),
          name: provider.display_name,
        })}
      </p>
    {:else if check.kind === 'unexpected_prefix'}
      <p class="warn">
        <CircleAlert size={14} strokeWidth={1.75} aria-hidden="true" />
        {t('wiz.check.prefix', { name: provider.display_name, prefix: check.expected })}
      </p>
    {:else if check.kind === 'too_short'}
      <p class="warn">
        <CircleAlert size={14} strokeWidth={1.75} aria-hidden="true" />
        {t('wiz.check.short')}
      </p>
    {/if}
    {#if tidied}
      <p class="muted">{t('wiz.check.tidied')}</p>
    {/if}
  </div>

  {#if needsUrl}
    <TextField
      label={t('wiz.baseUrl')}
      type="url"
      bind:value={baseUrl}
      placeholder="https://"
      hint={t('wiz.baseUrlHint')}
      required
    />
  {/if}
  {#if !rotate}
    <TextField
      label={t('wiz.label')}
      bind:value={label}
      autocomplete="off"
      hint={t('wiz.labelHint')}
    />
  {/if}

  {#if error}
    <p class="fail" role="alert">{t('wiz.keyFail', { error })}</p>
  {/if}
  <div class="actions">
    <Button variant="primary" type="submit" disabled={!ready} loading={saving}
      >{submitLabel ?? t('wiz.checkKey')}</Button
    >
  </div>
</form>

<style>
  .key {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  h4 {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    margin: 0;
    font-size: var(--alfa-font-size-md);
  }
  .how {
    padding: var(--alfa-space-3) var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface2);
    font-size: var(--alfa-font-size-sm);
  }
  h5 {
    margin: 0 0 var(--alfa-space-2);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  ol {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    margin: 0;
    padding-left: 1.4em;
  }
  .url {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  code {
    font-family: var(--alfa-font-mono);
    font-size: var(--alfa-font-size-xs);
    overflow-wrap: anywhere;
    user-select: all;
  }
  .note {
    margin: var(--alfa-space-2) 0 0;
    color: var(--alfa-color-text-muted);
  }
  .row {
    margin-top: calc(-1 * var(--alfa-space-2));
  }
  .feedback {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    min-height: 20px;
    font-size: var(--alfa-font-size-sm);
  }
  .feedback p {
    display: flex;
    align-items: flex-start;
    gap: var(--alfa-space-2);
    margin: 0;
  }
  .feedback :global(svg) {
    flex: none;
    margin-top: 3px;
  }
  .good {
    color: var(--alfa-color-success);
  }
  .warn {
    color: var(--alfa-color-warning);
  }
  .bad,
  .fail {
    color: var(--alfa-color-error);
  }
  .muted {
    color: var(--alfa-color-text-muted);
  }
  .fail {
    margin: 0;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
