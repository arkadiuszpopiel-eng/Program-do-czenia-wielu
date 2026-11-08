<!--
  Stan w pasku pisania (właściciel 29.10: ciemna kapsuła z belki „przenieść i dopasować”): trzy
  żetony obok wyboru agentki, jak wybór trybu w Claude Code. Profil modeli z kropką połączenia —
  okienko stanu z wyborem profilu; tryb pracy — menu czterech trybów; koszt sesji — szczegóły
  kosztów. Okienka otwierają się w górę i ładują dopiero po otwarciu (budżet startu).
-->
<script lang="ts">
  import { Chip, Popover } from '@alfa/ui-kit';
  import Cloud from '@lucide/svelte/icons/cloud';
  import Coins from '@lucide/svelte/icons/coins';
  import Combine from '@lucide/svelte/icons/combine';
  import Laptop from '@lucide/svelte/icons/laptop';
  import Shield from '@lucide/svelte/icons/shield';
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import WifiOff from '@lucide/svelte/icons/wifi-off';
  import { currentProfile } from '../../state/profile';
  import { useApp } from '../../state/context';
  import Lazy from '../shell/Lazy.svelte';

  interface Props {
    /** Okienko profilu (polecenie `/model` otwiera je z pola pisania). */
    profileOpen?: boolean;
  }
  let { profileOpen = $bindable(false) }: Props = $props();

  const app = useApp();
  const { t, tk } = app.i18n;
  const loadStatus = () => import('../shell/StatusDetails.svelte');
  const loadModes = () => import('../shell/ModeMenu.svelte');
  const loadCosts = () => import('../shell/CostDetails.svelte');

  const profileId = $derived(currentProfile(app));
  const profile = $derived(t(`profile.${profileId}`));
  const ProfileIcon = $derived(
    profileId === 'local' ? Laptop : profileId === 'cloud' ? Cloud : Combine,
  );
  const offline = $derived(app.system?.online === false);
  // Tryb obowiązujący w tej rozmowie (własny albo globalny); bez rozmowy — globalny z Brokera.
  const level = $derived(app.sessions.active?.autonomy ?? app.autonomy.global ?? 'L3');
  const mode = $derived(tk(`mode.${level}.name`));
  const cost = $derived(app.costs ? app.i18n.money(app.costs.session) : '—');
  const nearLimit = $derived(
    app.costs !== null &&
      app.costs.limit.enabled &&
      app.costs.month.minor / Math.max(1, app.costs.limit.monthly.minor) >= 0.8,
  );
  let modeOpen = $state(false);
  let costOpen = $state(false);
</script>

<Popover label={t('status.title')} bind:open={profileOpen} width={320} align="end">
  {#snippet trigger(props)}
    <Chip
      {...props}
      size="sm"
      tone={offline ? 'warning' : undefined}
      label={t('titlebar.state', {
        connection: t(offline ? 'status.offline' : 'status.online'),
        profile,
        level: `${mode} (${level})`,
      })}
      title={t('titlebar.stateTip')}
    >
      {#snippet icon()}
        <span class="pair">
          {#if offline}<WifiOff size={13} strokeWidth={1.75} />{:else}<span class="dot"></span>{/if}
          <ProfileIcon size={13} strokeWidth={1.75} />
        </span>
      {/snippet}
      <span class="word name">{profile}</span>
    </Chip>
  {/snippet}
  <Lazy
    load={loadStatus}
    props={{
      onnavigate: () => (profileOpen = false),
      onmode: () => {
        profileOpen = false;
        modeOpen = true;
      },
    }}
    label={t('common.loading')}
  />
</Popover>
<Popover label={t('mode.title')} bind:open={modeOpen} width={340} align="end">
  {#snippet trigger(props)}
    <Chip
      {...props}
      size="sm"
      tone={level === 'L4' ? 'warning' : undefined}
      label={t('titlebar.mode', { mode })}
      title={t('titlebar.modeTip')}
      data-level={level}
    >
      {#snippet icon()}
        {#if level === 'L4'}<ShieldAlert size={13} strokeWidth={1.75} />{:else}<Shield
            size={13}
            strokeWidth={1.75}
          />{/if}
      {/snippet}
      <span class="word">{mode}</span>
    </Chip>
  {/snippet}
  <Lazy
    load={loadModes}
    props={{
      // Po wyborze (klawisz 1–5, Enter) fokus wraca na żeton, nie ginie na stronie.
      onclose: () => {
        modeOpen = false;
        requestAnimationFrame(() =>
          document.querySelector<HTMLElement>('.composer [data-level]')?.focus(),
        );
      },
    }}
    label={t('common.loading')}
  />
</Popover>
<Popover label={t('costs.title')} bind:open={costOpen} align="end">
  {#snippet trigger(props)}
    <Chip
      {...props}
      size="sm"
      tone={nearLimit ? 'warning' : undefined}
      label={t('titlebar.cost', { cost })}
      title={t('titlebar.costTip')}
    >
      {#snippet icon()}<Coins size={13} strokeWidth={1.75} />{/snippet}
      <span class="num">{cost}</span>
    </Chip>
  {/snippet}
  <Lazy load={loadCosts} props={{}} label={t('common.loading')} />
</Popover>

<style>
  .pair {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-success);
    box-shadow: 0 0 6px color-mix(in srgb, var(--alfa-color-success) 60%, transparent);
  }
  .num {
    font-variant-numeric: tabular-nums;
  }
  /* Wąskie pole: profil jako kropka i ikona, a przy jeszcze węższym — tryb jako sama tarcza
     (pełne nazwy zostają w opisach dla czytnika i w okienkach). Na telefonie żetony mają własny
     wiersz, więc znów pokazują słowa. */
  @container (421px <= width <= 640px) {
    .name {
      display: none;
    }
  }
  @container (421px <= width <= 520px) {
    .word {
      display: none;
    }
  }
</style>
