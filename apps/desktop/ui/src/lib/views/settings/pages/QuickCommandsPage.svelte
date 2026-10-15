<!--
  Ustawienia → Polecenia bez modelu: tester („co stanie się z tą wiadomością”), własne polecenia
  (fraza → stała odpowiedź; dodawanie, edycja, włączanie, usuwanie z potwierdzeniem) i katalog
  wbudowanych (rachunki, literowanie, losowanie, data, quiz). Przełącznik włączenia — pod spodem,
  z definicji strony.
-->
<script lang="ts">
  import { Button, Switch, TextField } from '@alfa/ui-kit';
  import { errorText } from '../../../api/command-error';
  import type { CustomCommand, QuickCommandsView, QuickTest } from '../../../api/types-quick';
  import LoadFailed from '../../../components/shell/LoadFailed.svelte';
  import Loading from '../../../components/shell/Loading.svelte';
  import { useApp } from '../../../state/context';
  import QuickCommandForm from './QuickCommandForm.svelte';
  import './work.css';

  const app = useApp();
  const { t, tk } = app.i18n;
  const BLANK: CustomCommand = {
    id: '',
    name: '',
    phrases: [],
    mode: 'exact',
    reply: '',
    enabled: true,
    priority: 0,
  };
  let view = $state<QuickCommandsView | null>(null);
  let loadError = $state<string | null>(null);
  let editing = $state<{ index: number; command: CustomCommand } | null>(null);
  let formError = $state<string | null>(null);
  let confirm = $state<string | null>(null);
  let busy = $state(false);
  let message = $state('');
  let tested = $state<QuickTest | null>(null);

  async function load() {
    try {
      view = await app.client.localCommands.get();
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  $effect(() => {
    const text = message.trim();
    if (!text) {
      tested = null;
      return;
    }
    const timer = setTimeout(() => {
      app.client.localCommands.test(text).then(
        (r) => (tested = r),
        () => (tested = null),
      );
    }, 250);
    return () => clearTimeout(timer);
  });

  async function store(list: readonly CustomCommand[]): Promise<boolean> {
    busy = true;
    try {
      view = await app.client.localCommands.set(list);
      if (message.trim()) tested = await app.client.localCommands.test(message.trim());
      return true;
    } catch (e) {
      if (editing) formError = errorText(e);
      else app.toasts.show({ kind: 'error', message: errorText(e) });
      return false;
    } finally {
      busy = false;
    }
  }

  async function save(command: CustomCommand) {
    if (!view || !editing) return;
    const list = [...view.commands];
    const at = command.id ? list.findIndex((c) => c.id === command.id) : -1;
    if (at < 0) list.push(command);
    else list[at] = command;
    formError = null;
    if (await store(list)) editing = null;
  }

  async function toggle(index: number, on: boolean) {
    if (!view) return;
    await store(view.commands.map((c, i) => (i === index ? { ...c, enabled: on } : c)));
  }

  async function remove(id: string) {
    if (!view) return;
    confirm = null;
    await store(view.commands.filter((c) => c.id !== id));
  }

  function testLine(r: QuickTest): string {
    if (r.source === 'custom') return t('commands.test.custom', { name: r.name ?? '' });
    if (r.source === 'builtin') return t('commands.test.builtin', { name: r.name ?? '' });
    return tk(`commands.test.${r.source}`);
  }
</script>

<section class="wk-card" aria-labelledby="commands-title">
  <h3 id="commands-title">{t('commands.title')}</h3>
  <p>{t('commands.intro')}</p>
  {#if view && !view.enabled}<p class="wk-warn" role="status">{t('commands.off')}</p>{/if}
  {#if loadError}<LoadFailed error={loadError} onretry={() => void load()} />
  {:else if !view}<Loading />{/if}
</section>

{#if view}
  <section class="wk-card" aria-labelledby="commands-test">
    <h3 id="commands-test">{t('commands.test.title')}</h3>
    <TextField label={t('commands.test.input')} bind:value={message} spellcheck="false" />
    {#if tested}
      <div role="status" data-testid="commands-test-result">
        <p class:wk-ok={tested.source !== 'model' && tested.source !== 'disabled'}>
          {testLine(tested)}
        </p>
        {#if tested.reply}<p class="wk-code">{tested.reply}</p>{/if}
      </div>
    {/if}
  </section>

  <section class="wk-card" aria-labelledby="commands-own">
    <h3 id="commands-own">{t('commands.own.title')}</h3>
    <p class="wk-meta">{t('commands.own.hint')}</p>
    {#if view.commands.length === 0}<p class="wk-meta">{t('commands.own.empty')}</p>{/if}
    <ul class="wk-list">
      {#each view.commands as c, i (c.id)}
        <li>
          <div class="wk-actions">
            <strong>{c.name}</strong>
            <span class="wk-meta">{tk(`commands.mode.${c.mode}`)}</span>
            {#if c.priority !== 0}
              <span class="wk-meta">{t('commands.own.priority', { n: c.priority })}</span>
            {/if}
            <Switch
              label={t('commands.own.enabled', { name: c.name })}
              checked={c.enabled}
              disabled={busy}
              onchange={(on) => void toggle(i, on)}
            />
          </div>
          <span class="wk-meta">{t('commands.own.phrases')}: {c.phrases.join(' · ')}</span>
          <span class="wk-code">{c.reply}</span>
          <div class="wk-actions">
            {#if confirm === c.id}
              <span class="wk-warn">{t('commands.own.confirm', { name: c.name })}</span>
              <Button size="sm" variant="primary" disabled={busy} onclick={() => void remove(c.id)}
                >{t('commands.own.confirmYes')}</Button
              >
              <Button size="sm" variant="secondary" onclick={() => (confirm = null)}
                >{t('commands.own.cancel')}</Button
              >
            {:else}
              <Button
                size="sm"
                variant="secondary"
                disabled={busy || editing !== null}
                onclick={() => {
                  formError = null;
                  editing = { index: i, command: c };
                }}>{t('commands.own.edit')}</Button
              >
              <Button
                size="sm"
                variant="ghost"
                disabled={busy || editing !== null}
                onclick={() => (confirm = c.id)}
                >{t('commands.own.remove')}</Button
              >
            {/if}
          </div>
        </li>
      {/each}
    </ul>
    {#if !editing}
      <div class="wk-actions">
        <Button
          variant="secondary"
          disabled={busy || view.commands.length >= 50}
          onclick={() => {
            formError = null;
            editing = { index: -1, command: BLANK };
          }}>{t('commands.own.add')}</Button
        >
      </div>
    {/if}
  </section>

  {#if editing}
    <QuickCommandForm
      command={editing.command}
      {busy}
      error={formError}
      onsave={(c) => void save(c)}
      oncancel={() => (editing = null)}
    />
  {/if}

  <section class="wk-card" aria-labelledby="commands-builtin">
    <h3 id="commands-builtin">{t('commands.builtin.title')}</h3>
    <ul class="wk-list">
      {#each view.builtins as b (b.title)}
        <li>
          <strong>{b.title}</strong>
          <span class="wk-meta">{b.examples.map((e) => `„${e}”`).join(' · ')}</span>
        </li>
      {/each}
    </ul>
  </section>
{/if}
