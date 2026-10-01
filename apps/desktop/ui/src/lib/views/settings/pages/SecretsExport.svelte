<!--
  Jawny eksport sekretów (PLAN §15.1): osobna paczka `.alfa`, zawsze szyfrowana hasłem (min. 8
  znaków). Zwykły eksport nigdy nie zawiera kluczy API. Plik wybiera natywny dialog rdzenia.
-->
<script lang="ts">
  import { Button, TextField } from '@alfa/ui-kit';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let password = $state('');
  let repeat = $state('');
  let saved = $state<string | null>(null);

  const mismatch = $derived(repeat.length > 0 && password !== repeat);
  const ready = $derived(password.length >= 8 && password === repeat);

  async function run() {
    try {
      const res = await app.client.transfer.exportSecrets(password);
      if (res.status === 'saved') saved = t('tr.secrets.saved', { path: res.path });
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      password = '';
      repeat = '';
    }
  }
</script>

<section class="card" aria-labelledby="tr-secrets">
  <h3 id="tr-secrets">{t('tr.secrets.title')}</h3>
  <p class="desc">{t('tr.secrets.desc')}</p>
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
  <div>
    <Button variant="secondary" disabled={!ready} onclick={run}>{t('tr.secrets.button')}</Button>
  </div>
  {#if saved}<p class="ok" role="status">{saved}</p>{/if}
</section>

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
  }
  .pw {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--alfa-space-3);
  }
  .ok {
    color: var(--alfa-color-success);
  }
</style>
