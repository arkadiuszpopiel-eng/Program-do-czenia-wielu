<script lang="ts">
  import Menu from '@lucide/svelte/icons/menu';
  import Settings from '@lucide/svelte/icons/settings';
  import PanelRight from '@lucide/svelte/icons/panel-right';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Avatar from '../components/Avatar.svelte';
  import IconButton from '../components/IconButton.svelte';
  import { agentIds, type AgentId } from '../tokens';

  interface Props {
    project: string;
    session: string;
    /** Agentka, która mówi/pracuje (świeci jej awatar). */
    activeAgent?: AgentId;
    profile?: string;
    autonomy?: string;
    cost?: string;
    leftOpen?: boolean;
    rightOpen?: boolean;
    ontoggleleft?: () => void;
    ontoggleright?: () => void;
    onsettings?: () => void;
  }

  let {
    project,
    session,
    activeAgent,
    profile = 'Hybryda',
    autonomy = 'L3',
    cost = '0,00 zł',
    leftOpen = true,
    rightOpen = true,
    ontoggleleft,
    ontoggleright,
    onsettings,
  }: Props = $props();
</script>

<!-- Makieta F0. Pasek okna głównego z F1 jest w apps/desktop/ui (TitleBar z regionem przeciągania
     `data-tauri-drag-region`); Snap Layouts i przyciski natywne ocenia spike F0 (j). -->
<header class="titlebar" data-tauri-drag-region>
  <IconButton
    label={leftOpen ? 'Ukryj panel Sesje' : 'Pokaż panel Sesje'}
    size="sm"
    pressed={leftOpen}
    onclick={ontoggleleft}
    aria-keyshortcuts="Control+B"
  >
    <Menu size={16} strokeWidth={1.5} />
  </IconButton>
  <button
    type="button"
    class="crumbs"
    aria-label="Sesja: {project} › {session}. Kliknij, aby zmienić nazwę"
  >
    <span class="project">{project}</span>
    <span class="sep" aria-hidden="true">▸</span>
    <span class="session">{session}</span>
    <ChevronDown size={14} strokeWidth={1.5} aria-hidden="true" />
  </button>
  <div class="cast" role="group" aria-label="Obsada agentek">
    {#each agentIds as id (id)}
      <button type="button" class="cast-btn" aria-label="Szczegóły i rola agentki">
        <Avatar agent={id} size={24} speaking={activeAgent === id} />
      </button>
    {/each}
  </div>
  <button
    type="button"
    class="status"
    aria-label="Stan sesji: profil {profile}, autonomia {autonomy}, koszt {cost}. Kliknij, aby zobaczyć szczegóły"
  >
    <span class="dot" aria-hidden="true"></span>
    <span>{profile}</span>
    <span class="sep" aria-hidden="true">·</span>
    <span>{autonomy}</span>
    <span class="sep" aria-hidden="true">·</span>
    <span class="num">{cost}</span>
  </button>
  <IconButton
    label={rightOpen ? 'Ukryj panel prawy' : 'Pokaż panel prawy'}
    size="sm"
    pressed={rightOpen}
    onclick={ontoggleright}
    aria-keyshortcuts="Control+\\"
  >
    <PanelRight size={16} strokeWidth={1.5} />
  </IconButton>
  <IconButton label="Ustawienia" size="sm" onclick={onsettings} aria-keyshortcuts="Control+,">
    <Settings size={16} strokeWidth={1.5} />
  </IconButton>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
    height: var(--alfa-size-titlebar);
    padding: 0 var(--alfa-space-2);
    border-bottom: 1px solid var(--alfa-color-border);
    /* Tło: docelowo natywna Mica; tu jednolity kolor jako fallback. */
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
    user-select: none;
  }
  .crumbs,
  .status,
  .cast-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--alfa-space-1);
    height: 28px;
    padding: 0 var(--alfa-space-2);
    border: 0;
    border-radius: var(--alfa-radius-control);
    background: transparent;
    color: var(--alfa-color-text);
  }
  .crumbs:hover,
  .status:hover,
  .cast-btn:hover {
    background: var(--alfa-color-surface2);
  }
  .crumbs {
    min-width: 0;
    font-weight: var(--alfa-weight-semibold);
  }
  .project {
    color: var(--alfa-color-text-muted);
    font-weight: var(--alfa-weight-regular);
  }
  .sep {
    color: var(--alfa-color-text-subtle);
  }
  .cast {
    display: flex;
    margin-left: auto;
    gap: 0;
  }
  .cast-btn {
    padding: 0 2px;
  }
  .status {
    color: var(--alfa-color-text-muted);
    font-variant-numeric: tabular-nums;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-success);
  }
  @media (max-width: 720px) {
    .status,
    .project,
    .cast {
      display: none;
    }
  }
</style>
