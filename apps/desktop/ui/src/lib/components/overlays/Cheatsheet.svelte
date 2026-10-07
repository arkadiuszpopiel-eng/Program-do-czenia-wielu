<!-- Ściągawka skrótów (Ctrl+/): wszystkie skróty z PLAN §14.8 w grupach, z bieżącymi przypisaniami. -->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Kbd } from '@alfa/ui-kit';
  import { SHORTCUTS, SHORTCUT_GROUPS } from '../../logic/shortcut-registry';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  const byGroup = $derived(
    SHORTCUT_GROUPS.map((group) => ({
      group,
      items: SHORTCUTS.filter((d) => d.group === group && !/goto[2-9]/.test(d.id)),
    })),
  );

  function label(id: string): string {
    return id === 'session.goto1'
      ? t('shortcut.session.goto', { n: '1…9' })
      : app.i18n.tk(`shortcut.${id}`);
  }

  function chords(id: string): string[] {
    if (id === 'session.goto1') return ['Ctrl+1…9'];
    return app.bindings[id] ?? [];
  }
</script>

<Dialog.Root bind:open={() => app.cheatsheetOpen, (open) => (app.cheatsheetOpen = open)}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-sheet-overlay" />
    <Dialog.Content class="alfa-sheet">
      <Dialog.Title class="alfa-sheet-title">{t('cheatsheet.title')}</Dialog.Title>
      <Dialog.Description class="alfa-sheet-desc">{t('cheatsheet.customize')}</Dialog.Description>
      <!-- tabindex: przewijana treść dostępna z klawiatury (strzałki / PgDn). -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="grid" tabindex="0">
        {#each byGroup as { group, items } (group)}
          <section>
            <h3>{app.i18n.tk(`cheatsheet.group.${group}`)}</h3>
            <dl>
              {#each items as def (def.id)}
                <dt>{label(def.id)}</dt>
                <dd>
                  {#each chords(def.id) as chord (chord)}<Kbd keys={chord} />{:else}<span
                      class="off">{t('cheatsheet.disabled')}</span
                    >{/each}
                </dd>
              {/each}
            </dl>
          </section>
        {/each}
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.alfa-sheet-overlay) {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: var(--alfa-color-scrim);
  }
  :global(.alfa-sheet) {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 101;
    display: flex;
    flex-direction: column;
    width: min(880px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    padding: var(--alfa-space-6);
    transform: translate(-50%, -50%);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay);
    background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3);
    color: var(--alfa-color-text);
  }
  :global(.alfa-sheet-title) {
    margin: 0;
    font-size: var(--alfa-font-size-xl);
    font-weight: var(--alfa-weight-semibold);
  }
  :global(.alfa-sheet-desc) {
    margin: var(--alfa-space-1) 0 var(--alfa-space-4);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .grid {
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: var(--alfa-space-6);
  }
  h3 {
    margin-bottom: var(--alfa-space-2);
    font-size: var(--alfa-font-size-sm);
    color: var(--alfa-color-text-muted);
  }
  dl {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--alfa-space-1) var(--alfa-space-3);
    margin: 0;
    font-size: var(--alfa-font-size-sm);
  }
  dd {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--alfa-space-1);
    margin: 0;
  }
  .off {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
</style>
