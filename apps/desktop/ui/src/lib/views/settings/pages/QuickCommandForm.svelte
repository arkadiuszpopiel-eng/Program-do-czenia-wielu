<!--
  Formularz własnego polecenia (Ustawienia → Polecenia bez modelu): nazwa, frazy (wiersz = fraza),
  dopasowanie z podpowiedzią, odpowiedź, priorytet. Błąd zapisu z rdzenia pokazuje strona.
-->
<script lang="ts">
  import { Button, Select, TextField } from '@alfa/ui-kit';
  import type { CustomCommand, QuickMatchMode } from '../../../api/types-quick';
  import { useApp } from '../../../state/context';
  import './work.css';

  interface Props {
    command: CustomCommand;
    busy: boolean;
    error: string | null;
    onsave: (command: CustomCommand) => void;
    oncancel: () => void;
  }

  let { command, busy, error, onsave, oncancel }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const MODES: readonly QuickMatchMode[] = ['exact', 'starts', 'contains', 'regex'];
  let name = $state('');
  let phrases = $state('');
  let mode = $state<QuickMatchMode>('exact');
  let reply = $state('');
  let priority = $state<string | number>('0');
  const modes = $derived(MODES.map((m) => ({ value: m, label: tk(`commands.mode.${m}`) })));

  $effect(() => {
    name = command.name;
    phrases = command.phrases.join('\n');
    mode = command.mode;
    reply = command.reply;
    priority = String(command.priority);
  });

  function save() {
    onsave({
      ...command,
      name,
      phrases: phrases.split('\n'),
      mode,
      reply,
      priority: Math.max(-10, Math.min(10, Math.round(Number(priority) || 0))),
    });
  }
</script>

<section class="wk-card" aria-labelledby="commands-form-title">
  <h3 id="commands-form-title">{t('commands.form.title')}</h3>
  <TextField label={t('commands.form.name')} bind:value={name} maxlength={60} />
  <label class="wk-field">
    <span>{t('commands.form.phrases')}</span>
    <textarea rows="3" spellcheck="false" bind:value={phrases}></textarea>
  </label>
  <div class="wk-field">
    <span id="commands-form-mode">{t('commands.form.mode')}</span>
    <Select
      labelledby="commands-form-mode"
      value={mode}
      options={modes}
      onchange={(v) => (mode = v as QuickMatchMode)}
    />
    <span class="wk-meta">{tk(`commands.form.hint.${mode}`)}</span>
  </div>
  <label class="wk-field">
    <span>{t('commands.form.reply')}</span>
    <textarea rows="3" bind:value={reply}></textarea>
  </label>
  <TextField
    label={t('commands.form.priority')}
    type="number"
    min={-10}
    max={10}
    bind:value={priority}
  />
  {#if error}<p class="wk-error" role="alert">{error}</p>{/if}
  <div class="wk-actions">
    <Button variant="primary" disabled={busy} onclick={save}>{t('commands.form.save')}</Button>
    <Button variant="secondary" disabled={busy} onclick={oncancel}
      >{t('commands.form.cancel')}</Button
    >
  </div>
</section>
