<!-- Własne persony dziedziczą wspólny profil; zmiana ma jawny zakres także dla wbudowanych. -->
<script lang="ts">
  import { Select } from '@alfa/ui-kit';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const en = $derived(app.i18n.locale === 'en');
  const value = $derived(app.str('agents.interaction_mode', 'standard'));
  const options = $derived([
    { value: 'standard', label: en ? 'Standard' : 'Standardowy' },
    { value: 'jarvis', label: 'Jarvis' },
  ]);
  let busy = $state(false);

  async function change(next: string) {
    if (busy) return;
    busy = true;
    try {
      await app.setSetting('agents.interaction_mode', next);
    } finally {
      busy = false;
    }
  }
</script>

<div class="profile">
  <label for="builder-interaction-mode">
    {en ? 'Shared interaction profile' : 'Wspólny profil współpracy'}
  </label>
  <Select
    id="builder-interaction-mode"
    size="sm"
    {value}
    {options}
    disabled={busy}
    onchange={(next) => void change(next)}
  />
  <p class="wk-meta">
    {en
      ? 'Custom agents inherit this profile. Built-in agents using the common setting inherit it too; their individual choices take priority. Changes apply from the next response. Turn on the microphone separately in Voice.'
      : 'Własne agentki dziedziczą ten profil. Wbudowane z wyborem „Jak ustawienie wspólne” także; ich osobne wybory mają pierwszeństwo. Zmiana obowiązuje od następnej odpowiedzi. Mikrofon włącz osobno w panelu Głos.'}
  </p>
</div>

<style>
  .profile {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    max-width: 36rem;
    margin-block: var(--alfa-space-3);
  }
</style>
