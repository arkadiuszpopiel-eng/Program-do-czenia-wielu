<!-- Galeria komponentów F1 (prymitywy formularzy, banery, menu, warianty, kroki, wskaźniki). -->
<script lang="ts">
  import Banner from '../components/Banner.svelte';
  import Button from '../components/Button.svelte';
  import Checkbox from '../components/Checkbox.svelte';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import IconButton from '../components/IconButton.svelte';
  import Kbd from '../components/Kbd.svelte';
  import LevelMeter from '../components/LevelMeter.svelte';
  import Menu from '../components/Menu.svelte';
  import Popover from '../components/Popover.svelte';
  import SanitizedHtml from '../components/SanitizedHtml.svelte';
  import SegmentedControl from '../components/SegmentedControl.svelte';
  import Select from '../components/Select.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import Stepper from '../components/Stepper.svelte';
  import Switch from '../components/Switch.svelte';
  import TextField from '../components/TextField.svelte';
  import VariantSwitcher from '../components/VariantSwitcher.svelte';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import Inbox from '@lucide/svelte/icons/inbox';

  let on = $state(true);
  let theme = $state('auto');
  let level = $state('L3');
  let variant = $state(1);
  let confirm = $state(false);
  let last = $state('—');
  let meter = $state(0.42);
  const sample =
    '<p>HTML <strong>zsanitowany w Rust</strong> (ammonia) — UI tylko go wstawia.</p>' +
    '<pre><code class="language-ts">const a = 1;</code></pre>';
</script>

<div class="gallery">
  <section>
    <h3>Formularze</h3>
    <div class="row">
      <span id="g-switch">Przełącznik</span>
      <Switch bind:checked={on} labelledby="g-switch" />
      <Select
        bind:value={theme}
        label="Motyw"
        options={[
          { value: 'auto', label: 'Auto' },
          { value: 'light', label: 'Jasny' },
          { value: 'dark', label: 'Ciemny' },
        ]}
      />
      <Checkbox label="Zaszyfruj paczkę" description="hasłem, z KDF Argon2id" />
    </div>
    <TextField label="Klucz API" type="password" hint="Trafia do Menedżera poświadczeń Windows." />
    <TextField label="Kwota (zł)" type="number" error="Kwota musi być dodatnia." />
    <SegmentedControl
      label="Tryb importu"
      bind:value={level}
      options={[
        { value: 'add', label: 'Dodaj' },
        { value: 'merge', label: 'Scal' },
        { value: 'L3', label: 'Zastąp' },
      ]}
    />
  </section>
  <section>
    <h3>Banery stanów systemowych</h3>
    <Banner kind="warning" icon="offline">Brak połączenia. Działa model lokalny.</Banner>
    <Banner kind="info" icon="key" ondismiss={() => {}} dismissLabel="Ukryj">
      Brak kluczy — działa profil lokalny.
      {#snippet actions()}<Button size="sm">Dodaj klucz</Button>{/snippet}
    </Banner>
    <Banner kind="error" icon="mic">Windows blokuje dostęp do mikrofonu.</Banner>
  </section>
  <section>
    <h3>Menu, podpowiedź, warianty, kroki</h3>
    <div class="row">
      <Menu
        label="Akcje sesji"
        items={[
          { id: 'rename', label: 'Zmień nazwę', onSelect: () => (last = 'Zmień nazwę') },
          { id: 'pin', label: 'Przypnij', checked: true, onSelect: () => (last = 'Przypnij') },
          {
            id: 'del',
            label: 'Usuń',
            danger: true,
            separatorBefore: true,
            onSelect: () => (last = 'Usuń'),
          },
        ]}
      >
        {#snippet trigger(props)}
          <IconButton {...props} label="Akcje sesji"
            ><Ellipsis size={16} strokeWidth={1.5} /></IconButton
          >
        {/snippet}
      </Menu>
      <span class="muted">Wybrano: {last}</span>
      <Popover label="Szczegóły kosztów">
        {#snippet trigger(props)}<Button {...props} size="sm">Koszty ▾</Button>{/snippet}
        <p>Ta sesja: 1,08 zł · Dziś: 2,14 zł</p>
      </Popover>
      <VariantSwitcher
        index={variant}
        total={3}
        label="Wariant {variant} z 3"
        onprev={() => (variant = variant === 1 ? 3 : variant - 1)}
        onnext={() => (variant = variant === 3 ? 1 : variant + 1)}
      />
      <Kbd keys="Ctrl+Shift+F12" />
    </div>
    <Stepper
      label="Kreator"
      current={2}
      steps={[
        { id: 'a', label: 'Dostawca' },
        { id: 'b', label: 'Klucz' },
        { id: 'c', label: 'Test' },
        { id: 'd', label: 'Modele' },
      ]}
    />
  </section>
  <section>
    <h3>Ładowanie, puste stany, wskaźniki, treść z Rust</h3>
    <Skeleton lines={3} />
    <LevelMeter level={meter} label="Poziom głośności" />
    <input
      type="range"
      min="0"
      max="1"
      step="0.01"
      bind:value={meter}
      aria-label="Symulacja głośności"
    />
    <EmptyState title="Brak plików" description="Agentki nie oddały jeszcze plików w tej sesji.">
      {#snippet icon()}<Inbox size={20} strokeWidth={1.5} />{/snippet}
    </EmptyState>
    <SanitizedHtml html_sanitized={sample} />
    <Button variant="danger" onclick={() => (confirm = true)}>Usuń konto…</Button>
    <ConfirmDialog
      bind:open={confirm}
      title="Usunąć konto?"
      description="Klucz zostanie usunięty z Menedżera poświadczeń."
      confirmLabel="Usuń"
      danger
      onconfirm={() => (last = 'Usunięto konto')}
    />
  </section>
</div>

<style>
  .gallery {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--alfa-space-6);
    width: min(1200px, 95vw);
    padding: var(--alfa-space-6);
    background: var(--alfa-color-bg);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-3);
  }
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
</style>
