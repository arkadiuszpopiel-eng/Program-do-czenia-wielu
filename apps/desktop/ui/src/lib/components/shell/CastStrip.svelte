<!--
  Obsada w pasku tytułu: cztery agentki w jednej kapsule. Stan widać bez najeżdżania: gotowa —
  przygaszona, pracuje — pierścień i kropka, mówi — pulsujący pierścień, czeka na zatwierdzenie —
  bursztynowy znacznik. Podpowiedź podaje imię, rolę, stan i bieżącą czynność; klik otwiera panel
  Agentki.
-->
<script lang="ts">
  import { Avatar, Menu, agentIds, personInfo, type AgentId } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';
  import { agentNavigation } from '../../state/agent-navigation.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const states = $derived(app.activeId ? (app.agents[app.activeId] ?? []) : []);
  const en = $derived(app.i18n.locale === 'en');
  function items(id: AgentId) {
    const name = personInfo(id).name;
    const configure = () => {
      app.selectedAgent = id;
      app.openSettings('agents');
    };
    return [
      {
        id: 'settings',
        label: en ? `Settings for ${name}` : `Ustawienia agentki ${name}`,
        onSelect: configure,
      },
      {
        id: 'roles',
        label: en ? `Session roles · ${name}` : `Role w sesji · ${name}`,
        disabled: !app.activeId,
        onSelect: () => {
          app.selectedAgent = id;
          app.closeSettings();
          app.openPanel('agents');
        },
      },
      {
        id: 'chat',
        label: en ? `Write to ${name}` : `Napisz do ${name}`,
        disabled: !app.activeId,
        onSelect: () => {
          app.closeSettings();
          const draft = app.sessions.drafts[app.activeId ?? ''] ?? '';
          app.setDraft(`@${name} ${draft.replace(/^@(Alfa|Beta|Gama|Delta)\s*/i, '')}`);
          requestAnimationFrame(() => document.getElementById('alfa-composer')?.focus());
        },
      },
      {
        id: 'voice',
        label: en ? `Voice · ${name}` : `Głos · ${name}`,
        onSelect: () => {
          app.selectedAgent = id;
          agentNavigation.voice = id;
          app.openSettings('voice');
        },
      },
      {
        id: 'permissions',
        label: en ? 'Shadow Judge and autonomy' : 'Sędzia Cieni i autonomia',
        separatorBefore: true,
        onSelect: () => app.openSettings('permissions'),
      },
    ];
  }

  function describe(id: (typeof agentIds)[number]) {
    const state = states.find((a) => a.id === id);
    // Mowę z głośnika widać od razu po pomiarze poziomu, także gdy rdzeń nie zgłosił jeszcze stanu.
    const status = app.speechAgent === id ? 'speaking' : (state?.status ?? 'idle');
    const role = (state?.role_ids ?? []).map((r) => app.i18n.tk(`role.${r}`)).join(', ') || '—';
    const label = t('titlebar.agent', {
      name: personInfo(id).name,
      role,
      status: t(`agentStatus.${status}`),
    });
    return { status, label: state?.activity ? `${label} — ${state.activity}` : label };
  }
</script>

<div class="cast" role="group" aria-label={t('titlebar.cast')}>
  {#each agentIds as id (id)}
    {@const info = describe(id)}
    <Menu items={items(id)} label={info.label} align="start">
      {#snippet trigger(props)}
        <button
          type="button"
          class="cast-btn {info.status}"
          aria-label={info.label}
          title={info.label}
          {...props}
        >
          <Avatar
            agent={id}
            size={28}
            speaking={info.status === 'speaking'}
            working={info.status === 'working' || info.status === 'waiting_approval'}
            decorative
          />
          {#if info.status !== 'idle'}<span class="flag" aria-hidden="true"></span>{/if}
        </button>
      {/snippet}
    </Menu>
  {/each}
</div>

<style>
  .cast {
    display: flex;
    align-items: center;
    gap: 3px;
    height: var(--bar-h, 32px);
    padding: 0 3px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-bg);
  }
  .cast-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: var(--alfa-radius-full);
    background: transparent;
    transition: transform var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .cast-btn.speaking {
    z-index: 2;
  }
  .cast-btn[aria-expanded='true'] {
    background: var(--alfa-color-surface2);
  }
  /* Gotowa agentka jest spokojniejsza, żeby pracująca od razu rzucała się w oczy. */
  .cast-btn.idle :global(.avatar) {
    opacity: 0.78;
  }
  .cast-btn:hover :global(.avatar),
  .cast-btn:focus-visible :global(.avatar) {
    opacity: 1;
  }
  .cast-btn:hover {
    transform: scale(1.08);
  }
  .cast-btn:focus-visible {
    outline: var(--alfa-size-focus-ring) solid var(--alfa-color-focus);
    outline-offset: 1px;
  }
  .flag {
    position: absolute;
    right: -1px;
    bottom: -1px;
    width: 9px;
    height: 9px;
    border: 2px solid var(--alfa-color-bg);
    border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-info);
  }
  .speaking .flag {
    background: var(--alfa-color-success);
  }
  .waiting_approval .flag {
    background: var(--alfa-color-warning);
  }
  @media (max-width: 720px) {
    .cast {
      display: none;
    }
  }
  @media (forced-colors: active) {
    .flag {
      background: Highlight;
    }
  }
</style>
