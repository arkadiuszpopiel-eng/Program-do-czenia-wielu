<!--
  Paleta poleceń Ctrl+K (ładowana leniwie, wstępnie pobierana w bezczynności → otwarcie ≤ 50 ms):
  polecenia (ze skrótami), sesje i ustawienia; dopasowanie rozmyte bez polskich znaków.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { CommandPalette, type CommandItem } from '@alfa/ui-kit';
  import type { SettingsPageDef } from '../../api/types-system';
  import { bestScore } from '../../logic/fuzzy';
  import { COMPOSER_LOCAL, SHORTCUTS } from '../../logic/shortcut-registry';
  import { EXTRA_COMMANDS, runCommand } from '../../state/commands';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let schema = $state<readonly SettingsPageDef[]>([]);
  let fetching = false;

  // Schemat ustawień pobierany od razu (moduł palety ładuje się w bezczynności), żeby lista
  // pozycji była gotowa przed pierwszym otwarciem — otwarcie to wtedy tylko `show()`.
  // Błąd nie wycieka jako nieobsłużone odrzucenie: paleta działa bez pozycji ustawień (błąd
  // z „Ponów" pokazuje widok Ustawień), a przy kolejnym otwarciu próbujemy ponownie.
  async function fetchSchema() {
    if (fetching) return;
    fetching = true;
    try {
      schema = await app.client.settings.schema();
    } catch {
      // Paleta zostaje bez pozycji ustawień — patrz komentarz wyżej.
    } finally {
      fetching = false;
    }
  }

  $effect(() => {
    void app.palette.open;
    if (untrack(() => schema.length === 0)) void fetchSchema();
  });

  function label(id: string): string {
    const goto = /^session\.goto(\d)$/.exec(id);
    return goto
      ? t('shortcut.session.goto', { n: Number(goto[1]) })
      : app.i18n.tk(id.startsWith('action.') ? id : `shortcut.${id}`);
  }

  const items = $derived.by((): CommandItem[] => {
    const sessions: CommandItem[] = app.sessions.list
      .filter((s) => !s.archived)
      .map((s) => ({
        id: `session:${s.id}`,
        label: s.title,
        group: t('palette.group.sessions'),
        keywords: s.project ? [s.project.name] : [],
        onSelect: () => void app.openSession(s.id),
      }));
    if (app.palette.mode === 'sessions') return sessions;
    const actions: CommandItem[] = [
      ...SHORTCUTS.filter(
        (d) => d.scope === 'app' && !COMPOSER_LOCAL.has(d.id) && !/goto[2-9]/.test(d.id),
      ).map((d) => d.id),
      ...EXTRA_COMMANDS,
    ].map((id) => ({
      id: `cmd:${id}`,
      label: label(id),
      group: t('palette.group.actions'),
      shortcut: app.bindings[id]?.[0],
      onSelect: () => runCommand(app, id),
    }));
    const settings: CommandItem[] = schema.flatMap((page) => [
      {
        id: `page:${page.id}`,
        label: app.i18n.text(page.label),
        group: t('palette.group.settings'),
        keywords: [t('settings.title')],
        onSelect: () => app.openSettings(page.id),
      },
      ...page.settings.map((s) => ({
        id: `setting:${s.key}`,
        label: `${app.i18n.text(s.label)}`,
        group: t('palette.group.settings'),
        keywords: [app.i18n.text(page.label), app.i18n.text(s.description), s.key],
        onSelect: () => app.openSettings(page.id),
      })),
    ]);
    return [...actions, ...sessions, ...settings];
  });
</script>

<CommandPalette
  bind:open={() => app.palette.open, (open) => (app.palette.open = open)}
  {items}
  hotkey={false}
  placeholder={t(
    app.palette.mode === 'sessions' ? 'palette.placeholderSessions' : 'palette.placeholder',
  )}
  labels={{ title: t('palette.title'), search: t('palette.search'), empty: t('palette.empty') }}
  filter={(search, itemLabel, keywords) => bestScore(search, itemLabel, keywords)}
/>
