<script lang="ts">
  import { agents } from '@alfa/ui-kit';
  import Radar from '@lucide/svelte/icons/radar';
  import MessagesSquare from '@lucide/svelte/icons/messages-square';
  import Users from '@lucide/svelte/icons/users';
  import ListChecks from '@lucide/svelte/icons/list-checks';
  import Mic from '@lucide/svelte/icons/mic';
  import Cpu from '@lucide/svelte/icons/cpu';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import { useApp } from '../../state/context';
  import { cockpitCopy } from './copy';
  import './cockpit.css';
  const app = useApp();
  const copy = $derived(cockpitCopy(app.i18n.locale));
  const cast = $derived(app.activeId ? (app.agents[app.activeId] ?? []) : []);
  const tiles = $derived([
    {
      id: 'sessions',
      icon: MessagesSquare,
      label: copy.sessions,
      run: () => app.layout.toggleLeft(app.activeId),
    },
    { id: 'agents', icon: Users, label: copy.agents, run: () => app.openPanel('agents') },
    { id: 'tasks', icon: ListChecks, label: copy.tasks, run: () => app.openPanel('tasks') },
    { id: 'voice', icon: Mic, label: copy.voice, run: () => app.openSettings('voice') },
    { id: 'models', icon: Cpu, label: copy.models, run: () => app.openSettings('models') },
    {
      id: 'access',
      icon: ShieldCheck,
      label: copy.access,
      run: () => app.openSettings('permissions'),
    },
  ]);
</script>

<section class="cockpit" aria-label={copy.title}>
  <header class="cockpit-head">
    <div>
      <p class="cockpit-eyebrow">JARVIS <span aria-hidden="true">/</span> ALFA</p>
      <h2>{copy.heading}</h2>
      <p class="cockpit-session">
        <span>{copy.session}</span>
        {app.sessions.active?.title ?? copy.empty}
      </p>
    </div>
    <div class="cockpit-orb" aria-hidden="true"><i></i><Radar size={32} strokeWidth={1} /></div>
  </header>
  <nav class="cockpit-tiles" aria-label={copy.navigation}>
    {#each tiles as tile (tile.id)}
      <button type="button" data-cockpit-tile={tile.id} onclick={tile.run}>
        <tile.icon size={20} strokeWidth={1.5} aria-hidden="true" /><span>{tile.label}</span>
      </button>
    {/each}
  </nav>
  {#if cast.length}
    <div class="cockpit-cast" role="group" aria-label={copy.crew}>
      {#each cast as person (person.id)}
        <button
          type="button"
          onclick={() => {
            app.selectedAgent = person.id;
            app.openPanel('agents');
          }}
        >
          <span class="cockpit-state" data-status={person.status} aria-hidden="true"></span>
          {agents[person.id].name}<span class="cockpit-agent-status"
            >{app.i18n.t(`agentStatus.${person.status}`)}</span
          >
        </button>
      {/each}
    </div>
  {/if}
</section>
