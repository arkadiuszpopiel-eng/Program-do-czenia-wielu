<!--
  Dyktowanie (F5): stan (aplikacja docelowa, aktywne/wstrzymane i dlaczego, liczba znaków),
  podgląd ostatniej frazy, start z odliczaniem (czas na przejście do okna docelowego — okno Alfy
  jest chronione), „Cofnij", skrót globalny i komendy. W Ustawieniach — profile aplikacji.
-->
<script lang="ts">
  import { Button, Checkbox, Kbd, TextField } from '@alfa/ui-kit';
  import type { VoiceFeatures } from '../../api/types-voice';
  import { appName } from '../../logic/voice-features';
  import { useApp } from '../../state/context';
  import { voiceActions } from './voice-act';

  interface Props {
    features: VoiceFeatures;
    settings?: boolean;
  }

  /** Odliczanie przed startem z przycisku (s). */
  const COUNTDOWN = 3;

  let { features, settings = false }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const act = voiceActions(app);
  const uid = $props.id();
  const d = $derived(features.dictation);
  const live = $derived(d.state === 'active' || d.state === 'paused');
  let countdown = $state<number | null>(null);
  let newApp = $state('');
  let capitalize = $state(true);
  let blockEnter = $state(true);
  let appError = $state<string | undefined>(undefined);

  $effect(() => {
    if (countdown === null) return;
    const handle = setTimeout(() => {
      if (countdown === null) return;
      if (countdown <= 1) {
        countdown = null;
        void act.dictation({ kind: 'start' });
      } else countdown -= 1;
    }, 1000);
    return () => clearTimeout(handle);
  });

  async function addProfile() {
    const name = appName(newApp);
    if (!name) {
      appError = t('vf.dict.badApp');
      return;
    }
    appError = undefined;
    const ok = await act.dictation({
      kind: 'save_profile',
      profile: { app: name, capitalize_start: capitalize, block_enter: blockEnter },
    });
    if (ok) newApp = '';
  }
</script>

<section class="vf-card" aria-labelledby="{uid}-title">
  <div class="vf-head">
    <h3 id="{uid}-title">{t('vf.dict.title')}</h3>
    <span class="vf-badge" class:on={d.state === 'active'}
      >{t(`vf.dict.state.${d.state}`, { app: d.app ?? '—' })}</span
    >
  </div>
  {#if settings}<p class="vf-muted">{t('vf.dict.desc')}</p>{/if}
  {#if d.reason}<p class="vf-warn" role="status">{app.i18n.text(d.reason)}</p>{/if}
  {#if live}
    <p>{t('vf.dict.typed', { n: d.typed_chars })}</p>
    {#if d.preview}
      <p class="vf-muted">{t('vf.dict.preview')}</p>
      <p class="vf-preview">{d.preview}</p>
    {/if}
  {/if}
  <div class="vf-row">
    {#if live}
      <Button size="sm" variant="primary" onclick={() => void act.dictation({ kind: 'stop' })}
        >{t('vf.dict.stop')}</Button
      >
      <Button
        size="sm"
        variant="secondary"
        disabled={!d.can_undo}
        onclick={() => void act.dictation({ kind: 'undo' })}>{t('vf.dict.undo')}</Button
      >
    {:else if countdown !== null}
      <p class="vf-state" role="status">{t('vf.dict.countdown', { s: countdown })}</p>
      <Button size="sm" variant="ghost" onclick={() => (countdown = null)}
        >{t('vf.speaker.cancel')}</Button
      >
    {:else}
      <Button
        size="sm"
        variant="secondary"
        disabled={d.state === 'unavailable'}
        onclick={() => (countdown = COUNTDOWN)}>{t('vf.dict.start', { s: COUNTDOWN })}</Button
      >
    {/if}
  </div>
  {#if d.shortcut}
    <p class="vf-muted">
      {t('vf.dict.shortcut', { key: '' })}<Kbd keys={d.shortcut} />
    </p>
  {/if}
  <p class="vf-muted">{t('vf.dict.commands')}</p>

  {#if settings}
    <h4 id="{uid}-profiles">{t('vf.dict.profiles')}</h4>
    <p class="vf-muted">{t('vf.dict.profilesHint')}</p>
    {#if d.profiles.length === 0}
      <p class="vf-muted">{t('vf.dict.noProfiles')}</p>
    {:else}
      <ul class="vf-list" aria-labelledby="{uid}-profiles">
        {#each d.profiles as p (p.app)}
          <li>
            <strong>{p.app}</strong>
            {#if p.capitalize_start}<span class="vf-badge">{t('vf.dict.capitalize')}</span>{/if}
            {#if p.block_enter}<span class="vf-badge">{t('vf.dict.blockEnter')}</span>{/if}
            <Button
              size="sm"
              variant="ghost"
              onclick={() => void act.dictation({ kind: 'remove_profile', app: p.app })}
              >{t('vf.dict.remove', { app: p.app })}</Button
            >
          </li>
        {/each}
      </ul>
    {/if}
    <TextField
      label={t('vf.dict.app')}
      placeholder="notepad.exe"
      bind:value={newApp}
      error={appError}
    />
    <Checkbox bind:checked={capitalize} label={t('vf.dict.capitalize')} />
    <Checkbox bind:checked={blockEnter} label={t('vf.dict.blockEnter')} />
    <div class="vf-row">
      <Button size="sm" variant="secondary" onclick={() => void addProfile()}
        >{t('vf.dict.add')}</Button
      >
    </div>
  {/if}
</section>
