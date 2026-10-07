<!-- Panel Agentki (makieta 6): cztery persony, role w sesji, stan (mówi / pracuje), szablony obsad. -->
<script lang="ts">
  import {
    Avatar,
    Button,
    Checkbox,
    Chip,
    Select,
    agentIds,
    agents,
    type AgentId,
  } from '@alfa/ui-kit';
  import type { CastTemplateId } from '../../api/types';
  import { attempt } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import WorkdirCard from './WorkdirCard.svelte';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const ROLES = [
    'conductor',
    'speaker',
    'thinker',
    'operator',
    'coder',
    'critic',
    'researcher',
    'keeper',
    'writer',
  ];
  const TEMPLATES: readonly CastTemplateId[] = ['standard', 'solo', 'coding', 'research'];
  let template = $state<string>('standard');
  let editing = $state<AgentId | null>(null);
  /** Po odrzuconej zmianie ról pola wyboru odtwarzają stan z rdzenia (same przełączyły się już). */
  let rolesRev = $state(0);

  const states = $derived(app.agents[sessionId] ?? []);
  const stateOf = (id: AgentId) => states.find((a) => a.id === id);

  async function applyTemplate() {
    const ok = await attempt(app.toasts, () =>
      app.client.agents.applyCast(sessionId, template as CastTemplateId),
    );
    if (ok) app.toasts.show({ kind: 'success', message: t('agents.applied') });
  }

  async function toggleRole(agent: AgentId, role: string, on: boolean) {
    const current = stateOf(agent)?.role_ids ?? [];
    const next = on ? [...current, role] : current.filter((r) => r !== role);
    if (!(await attempt(app.toasts, () => app.client.agents.setRoles(sessionId, agent, next)))) {
      rolesRev += 1;
    }
  }
</script>

<div class="agents">
  <WorkdirCard {sessionId} />
  <div class="template">
    <label for="cast-template" class="label">{t('agents.template')}</label>
    <div class="row">
      <Select
        id="cast-template"
        bind:value={template}
        options={TEMPLATES.map((id) => ({ value: id, label: t(`agents.template.${id}`) }))}
        size="sm"
      />
      <Button size="sm" variant="secondary" onclick={applyTemplate}>{t('agents.apply')}</Button>
    </div>
    <p class="hint">{t('agents.conductorHint')}</p>
  </div>
  <ul class="list">
    {#each agentIds as id (id)}
      {@const st = stateOf(id)}
      <li class="card" style:--accent="var(--alfa-agent-{id})">
        <div class="top">
          <Avatar
            agent={id}
            size={32}
            speaking={st?.status === 'speaking'}
            working={st?.status === 'working'}
          />
          <div class="who">
            <span class="name">{agents[id].name}</span>
            <span class="persona">{t(`persona.${id}`)}</span>
          </div>
          <span class="status" class:live={st && st.status !== 'idle'}
            >{t(`agentStatus.${st?.status ?? 'idle'}`)}</span
          >
        </div>
        {#if st?.activity}<p class="activity">{t('agents.now', { activity: st.activity })}</p>{/if}
        <div class="roles" aria-label={t('agents.roles')}>
          {#each st?.role_ids ?? [] as role (role)}
            <Chip size="sm" agent={id}>{app.i18n.tk(`role.${role}`)}</Chip>
          {:else}
            <span class="hint">{t('agents.noRole')}</span>
          {/each}
        </div>
        <div class="actions">
          <Button
            size="sm"
            variant="ghost"
            aria-expanded={editing === id}
            aria-label={t('agents.editRoles', { name: agents[id].name })}
            onclick={() => (editing = editing === id ? null : id)}
          >
            {t('agents.editRolesShort')}
          </Button>
        </div>
        {#if editing === id}
          <fieldset class="role-editor">
            <legend class="alfa-visually-hidden"
              >{t('agents.editRoles', { name: agents[id].name })}</legend
            >
            {#key rolesRev}
              {#each ROLES as role (role)}
                <Checkbox
                  label={app.i18n.tk(`role.${role}`)}
                  checked={(st?.role_ids ?? []).includes(role)}
                  onchange={(on) => void toggleRole(id, role, on)}
                />
              {/each}
            {/key}
          </fieldset>
        {/if}
        <p class="hint">{t('agents.hint', { name: agents[id].name })}</p>
      </li>
    {/each}
  </ul>
</div>

<style>
  .agents {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-4);
  }
  .label {
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .template .row {
    display: flex;
    gap: var(--alfa-space-2);
    margin-top: var(--alfa-space-1);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border);
    border-left: 3px solid var(--accent);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
  }
  .who {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: var(--alfa-weight-semibold);
  }
  .persona,
  .hint,
  .activity {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .status {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .status.live {
    color: var(--accent);
    font-weight: var(--alfa-weight-semibold);
  }
  .roles {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
  .role-editor {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--alfa-space-1) var(--alfa-space-3);
    margin: 0;
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control);
  }
</style>
