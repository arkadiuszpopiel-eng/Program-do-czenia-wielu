<script lang="ts">
  import { Avatar, Button, Checkbox, Select, Switch, agentIds, personInfo } from '@alfa/ui-kit';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const en = $derived(app.i18n.locale === 'en');
  const id = $derived(app.selectedAgent);
  const prefix = $derived(`agents.${id}.`);
  const mode = $derived(app.str(`${prefix}access`, id === 'alfa' ? 'full' : 'roles'));
  const groups = $derived(
    app
      .str(`${prefix}groups`, '')
      .split(',')
      .map((g) => g.trim())
      .filter(Boolean),
  );
  const choices = [
    ['fs', 'Pliki', 'Files'],
    ['shell', 'Terminal', 'Terminal'],
    ['worktree', 'Kod i worktree', 'Code and worktree'],
    ['gui.control', 'Mysz i klawiatura', 'Mouse and keyboard'],
    ['browser', 'Przeglądarka', 'Browser'],
    ['web', 'Wyszukiwanie', 'Search'],
    ['net', 'Sieć', 'Network'],
    ['office', 'Dokumenty', 'Documents'],
    ['memory', 'Pamięć', 'Memory'],
    ['system', 'System i programy', 'System and apps'],
    ['vision', 'Obraz', 'Vision'],
    ['media', 'Multimedia', 'Media'],
    ['plugin', 'Wtyczki', 'Plugins'],
    ['mcp', 'Serwery MCP', 'MCP servers'],
    ['cli-bridge', 'Mosty CLI', 'CLI bridges'],
    ['delegate', 'Delegowanie agentkom', 'Delegate to agents'],
    ['clipboard', 'Schowek', 'Clipboard'],
  ] as const;
  const profileOptions = $derived([
    { value: 'roles', label: en ? 'By assigned roles' : 'Według przydzielonych ról' },
    { value: 'full', label: en ? 'Full access' : 'Pełny dostęp' },
    { value: 'read', label: en ? 'Read only' : 'Tylko odczyt' },
    { value: 'custom', label: en ? 'Custom access' : 'Własny dostęp' },
    { value: 'off', label: en ? 'No tools' : 'Bez narzędzi' },
  ]);
  function toggle(group: string, on: boolean) {
    const next = on ? [...new Set([...groups, group])] : groups.filter((g) => g !== group);
    void app.setSetting(`${prefix}groups`, next.join(','));
  }
</script>

<section class="access" data-testid="agent-permissions">
  <div class="tabs" role="group" aria-label={en ? 'Agent' : 'Agentka'}>
    {#each agentIds as agent (agent)}
      <button
        type="button"
        class:active={id === agent}
        aria-pressed={id === agent}
        onclick={() => (app.selectedAgent = agent)}
      >
        <Avatar {agent} size={28} decorative />
        {personInfo(agent).name}
      </button>
    {/each}
  </div>
  <h3>
    {personInfo(id).name} · {en
      ? 'Security and tool access'
      : 'Bezpieczeństwo i dostęp do narzędzi'}
  </h3>
  <p>
    {en
      ? 'Choose what this agent can use. Sędzia Cieni autonomy, the working folder and system permissions still apply. Changes take effect on the next task.'
      : 'Wybierz, z czego może korzystać ta agentka. Obowiązują poziom autonomii Sędziego Cieni, katalog roboczy i uprawnienia systemu. Zmiany działają od następnego zadania.'}
  </p>
  <label for="agent-access-profile">{en ? 'Access profile' : 'Profil dostępu'}</label>
  <Select
    id="agent-access-profile"
    value={mode}
    options={profileOptions}
    onchange={(v) => void app.setSetting(`${prefix}access`, v)}
  />
  {#if mode === 'custom'}
    <div class="groups">
      {#each choices as [key, pl, english] (key)}
        <Checkbox
          label={en ? english : pl}
          checked={groups.includes(key)}
          onchange={(on) => toggle(key, on)}
        />
      {/each}
    </div>
    <Switch
      label={en ? 'Read only for selected tools' : 'Tylko odczyt dla wybranych narzędzi'}
      checked={app.settings[`${prefix}read_only`] === true}
      onchange={(on) => void app.setSetting(`${prefix}read_only`, on)}
    />
    <label for="agent-extra-groups"
      >{en
        ? 'Tool groups (comma separated; advanced)'
        : 'Grupy narzędzi (po przecinku; zaawansowane)'}</label
    >
    <input
      id="agent-extra-groups"
      value={groups.join(', ')}
      onchange={(e) => void app.setSetting(`${prefix}groups`, e.currentTarget.value)}
    />
  {/if}
  <div class="actions">
    <Button
      variant="secondary"
      disabled={!app.activeId}
      onclick={() => {
        app.closeSettings();
        app.openPanel('agents');
      }}>{en ? 'Session roles and working folder' : 'Role w sesji i katalog roboczy'}</Button
    >
    <Button variant="ghost" onclick={() => app.openSettings('permissions')}
      >{en ? 'Sędzia Cieni and autonomy' : 'Sędzia Cieni i autonomia'}</Button
    >
    <Button variant="ghost" onclick={() => void app.resetSetting(`${prefix}access`)}
      >{en ? 'Reset profile' : 'Przywróć profil'}</Button
    >
  </div>
</section>

<style>
  /* DESIGN DECISIONS: systemowa typografia Segoe UI; istniejące tokeny kolorów Alfy;
     ruch tylko stanu kontrolek; układ kart i siatki; natywna Mica bez dodatkowego rozmycia. */
  .access {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    padding: var(--alfa-space-4);
    margin-bottom: var(--alfa-space-4);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  .tabs,
  .actions {
    display: flex;
    gap: var(--alfa-space-2);
    flex-wrap: wrap;
  }
  .tabs button {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    min-height: 44px;
    padding: 4px 12px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
  }
  .tabs .active {
    border-color: var(--alfa-color-focus);
    background: var(--alfa-color-surface2);
  }
  p {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: var(--alfa-space-3);
  }
  input {
    min-height: 36px;
    padding: 8px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
  }
  button:focus-visible,
  input:focus-visible {
    outline: 2px solid var(--alfa-color-focus);
    outline-offset: 2px;
  }
</style>
