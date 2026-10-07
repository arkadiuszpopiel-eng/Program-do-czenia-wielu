<!--
  Panel Zadania: drzewo DAG schedulera (delegacja = podzadania, zależności „po: …"), stan,
  postęp i koszt; sterowanie w punktach atomowych, wstrzymanie, anulowanie z poddrzewem,
  ponowienie; nowe zadanie w bieżącej sesji (opcjonalnie po innym — krawędź DAG).
-->
<script lang="ts">
  import { Button, EmptyState, Select, Switch, TextField } from '@alfa/ui-kit';
  import ListTodo from '@lucide/svelte/icons/list-todo';
  import type { TaskInfo } from '../../api/types-tasks';
  import { attempt, load } from '../../state/attempt';
  import { useApp } from '../../state/context';
  import LoadFailed from '../shell/LoadFailed.svelte';
  import TaskNode from './TaskNode.svelte';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  let tasks = $state<TaskInfo[]>([]);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);
  let showDone = $state(true);
  let goal = $state('');
  let after = $state('');

  // Błąd listy = „Nie udało się wczytać" z „Ponów", nie mylący pusty stan „brak zadań".
  async function loadTasks() {
    loadError = null;
    const result = await load(() => app.client.tasks.list());
    if (result.status === 'ready') tasks = [...result.value];
    else if (result.status === 'failed') loadError = result.error;
    loaded = true;
  }

  $effect(() => {
    void loadTasks();
    return app.on((event) => {
      if (event.type !== 'TaskUpdated') return;
      const i = tasks.findIndex((x) => x.id === event.task.id);
      tasks = i < 0 ? [...tasks, event.task] : tasks.map((x, j) => (j === i ? event.task : x));
    });
  });

  const visible = $derived(tasks.filter((x) => showDone || x.state !== 'done'));
  const ids = $derived(new Set(visible.map((x) => x.id)));
  const roots = $derived(visible.filter((x) => !x.parent_id || !ids.has(x.parent_id)));
  const childrenOf = (id: string): readonly TaskInfo[] => visible.filter((x) => x.parent_id === id);
  const titleOf = (id: string): string => tasks.find((x) => x.id === id)?.title ?? id;
  const afterOptions = $derived([
    { value: '', label: t('tasks.afterNone') },
    ...tasks.filter((x) => x.state !== 'done').map((x) => ({ value: x.id, label: x.title })),
  ]);

  async function add() {
    const text = goal.trim();
    if (!text) return;
    // Pola czyszczone tylko po sukcesie — po błędzie cel zostaje do ponowienia.
    await attempt(app.toasts, async () => {
      const created = await app.client.tasks.create({
        session_id: sessionId,
        title: '',
        goal: text,
        agent: null,
        after: after ? [after] : [],
        parent_id: null,
      });
      goal = '';
      after = '';
      app.toasts.show({ kind: 'success', message: t('tasks.added', { title: created.title }) });
    });
  }
</script>

<div class="tasks">
  <form
    class="new"
    aria-label={t('tasks.new')}
    onsubmit={(e) => {
      e.preventDefault();
      void add();
    }}
  >
    <TextField label={t('tasks.goal')} bind:value={goal} />
    <Select label={t('tasks.after')} size="sm" bind:value={after} options={afterOptions} />
    <Button size="sm" type="submit" disabled={!goal.trim()}>{t('tasks.add')}</Button>
  </form>
  <div class="row">
    <span id="tasks-done">{t('tasks.showDone')}</span>
    <Switch bind:checked={showDone} labelledby="tasks-done" />
  </div>
  {#if loadError}
    <LoadFailed error={loadError} onretry={() => void loadTasks()} />
  {:else if loaded && roots.length === 0}
    <EmptyState title={t('panel.tasks')} description={t('tasks.empty')}>
      {#snippet icon()}<ListTodo size={20} strokeWidth={1.5} />{/snippet}
    </EmptyState>
  {:else}
    <ul class="tree" aria-label={t('tasks.list')}>
      {#each roots as task (task.id)}
        <TaskNode {task} {childrenOf} {titleOf} />
      {/each}
    </ul>
  {/if}
</div>

<style>
  .tasks {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
  }
  .new {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--alfa-font-size-sm);
  }
  .tree {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
</style>
