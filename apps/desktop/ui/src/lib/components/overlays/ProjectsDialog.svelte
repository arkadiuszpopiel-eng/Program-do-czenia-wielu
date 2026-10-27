<!--
  Okno „Projekty”: dodawanie projektu ze wskazaniem katalogu roboczego oraz proste zarządzanie
  projektami (lista, katalog projektu, zmiana nazwy, usunięcie, nowa rozmowa w projekcie,
  przypisanie rozmowy do projektu).
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button } from '@alfa/ui-kit';
  import { untrack } from 'svelte';
  import { errorText } from '../../api/command-error';
  import type { ProjectInfo } from '../../api/types';
  import { closeProjects, projectsDialog } from '../../state/board-dialog.svelte';
  import { useApp } from '../../state/context';

  const app = useApp();
  const en = $derived(app.i18n.locale === 'en');

  let items = $state<readonly ProjectInfo[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let nameInput = $state('');
  let pickedDir = $state<string | null>(null);
  let editingName = $state<string | null>(null);
  let editingValue = $state('');

  const targetSession = $derived(
    projectsDialog.sessionId
      ? (app.sessions.list.find((s) => s.id === projectsDialog.sessionId) ?? null)
      : null,
  );

  function folderBaseName(path: string): string {
    return path.split(/[\\/]/).filter(Boolean).at(-1)?.slice(0, 80).trim() ?? '';
  }

  async function refreshAll(nextProjects?: readonly ProjectInfo[]): Promise<void> {
    items = nextProjects ?? (await app.client.sessions.projectsList());
    app.sessions.list = [...(await app.client.sessions.list())];
  }

  $effect(() => {
    const initialProj = projectsDialog.projectName;
    untrack(() => {
      loading = true;
      void app.client.sessions
        .projectsList()
        .then((res) => {
          items = res;
          const found = initialProj ? res.find((p) => p.name === initialProj) : undefined;
          if (found) {
            nameInput = found.name;
            pickedDir = found.workdir;
          }
        })
        .catch((e) => app.toasts.show({ kind: 'error', message: errorText(e) }))
        .finally(() => {
          loading = false;
        });
    });
  });

  async function pickNewProjectDir(): Promise<void> {
    try {
      const dir = await app.client.sessions.projectsPickDir();
      if (dir) {
        pickedDir = dir;
        if (!nameInput.trim()) nameInput = folderBaseName(dir);
      }
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    }
  }

  async function saveProject(andAction: 'none' | 'new-session' | 'assign'): Promise<void> {
    const cleanName = (nameInput.trim() || (pickedDir ? folderBaseName(pickedDir) : '')).trim();
    if (!cleanName || busy) return;
    busy = true;
    try {
      const updated = await app.client.sessions.projectsSave(cleanName, pickedDir);
      if (andAction === 'assign' && targetSession) {
        await app.client.sessions.setProject(targetSession.id, cleanName);
        await refreshAll();
        app.toasts.show({
          kind: 'success',
          message: en ? `Assigned to project “${cleanName}”.` : `Przypisano rozmowę do projektu „${cleanName}”.`,
        });
        closeProjects();
        return;
      }
      if (andAction === 'new-session') {
        const created = await app.client.sessions.create('coding');
        await app.client.sessions.setProject(created.id, cleanName);
        await refreshAll();
        closeProjects();
        if (app.view === 'settings') app.closeSettings();
        await app.openSession(created.id);
        return;
      }
      await refreshAll(updated);
      nameInput = '';
      pickedDir = null;
      app.toasts.show({
        kind: 'success',
        message: en ? `Saved project “${cleanName}”.` : `Zapisano projekt „${cleanName}”.`,
      });
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      busy = false;
    }
  }

  async function startSessionInProject(projectName: string): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      const created = await app.client.sessions.create('coding');
      await app.client.sessions.setProject(created.id, projectName);
      await refreshAll();
      closeProjects();
      if (app.view === 'settings') app.closeSettings();
      await app.openSession(created.id);
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      busy = false;
    }
  }

  async function assignTargetTo(projectName: string | null): Promise<void> {
    if (!targetSession || busy) return;
    busy = true;
    try {
      await app.client.sessions.setProject(targetSession.id, projectName);
      await refreshAll();
      closeProjects();
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      busy = false;
    }
  }

  async function changeProjectDir(proj: ProjectInfo): Promise<void> {
    if (busy) return;
    try {
      const dir = await app.client.sessions.projectsPickDir();
      if (!dir) return;
      busy = true;
      await refreshAll(await app.client.sessions.projectsSave(proj.name, dir));
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      busy = false;
    }
  }

  function beginRename(proj: ProjectInfo): void {
    editingName = proj.name;
    editingValue = proj.name;
  }

  async function commitRename(oldName: string): Promise<void> {
    const next = editingValue.trim();
    editingName = null;
    if (!next || next === oldName || busy) return;
    busy = true;
    try {
      await refreshAll(await app.client.sessions.projectsRename(oldName, next));
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      busy = false;
    }
  }

  async function removeProject(projectName: string): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      await refreshAll(await app.client.sessions.projectsRemove(projectName));
      app.toasts.show({
        kind: 'info',
        message: en ? `Removed project “${projectName}”.` : `Usunięto projekt „${projectName}”.`,
      });
    } catch (e) {
      app.toasts.show({ kind: 'error', message: errorText(e) });
    } finally {
      busy = false;
    }
  }
</script>

<Dialog.Root open onOpenChange={(open) => !open && closeProjects()}>
  <Dialog.Portal>
    <Dialog.Overlay class="alfa-projects-overlay" />
    <Dialog.Content class="alfa-projects" data-testid="projects-dialog">
      <Dialog.Title class="alfa-projects-title">
        {en ? 'Projects & project directories' : 'Projekty i katalogi projektów'}
      </Dialog.Title>
      <Dialog.Description class="alfa-projects-lead">
        {en
          ? 'Add a project with its working folder or manage existing projects. Conversations in a project share its folder and project memory.'
          : 'Dodaj projekt ze wskazaniem katalogu projektu lub zarządzaj istniejącymi projektami. Rozmowy w projekcie dzielą katalog roboczy i pamięć projektu.'}
      </Dialog.Description>

      {#if targetSession}
        <div class="target-bar">
          <div class="target-info">
            <span class="muted">{en ? 'Conversation:' : 'Rozmowa:'}</span>
            <strong>{targetSession.title}</strong>
            {#if targetSession.project}<span class="badge">{targetSession.project.name}</span>{/if}
          </div>
          {#if targetSession.project}
            <Button size="sm" variant="secondary" disabled={busy} onclick={() => void assignTargetTo(null)}>
              {en ? 'Remove from project' : 'Usuń z projektu'}
            </Button>
          {/if}
        </div>
      {/if}

      <section class="add-box">
        <p class="section-title">{en ? 'Add or update a project' : 'Dodaj projekt ze wskazaniem katalogu'}</p>
        <div class="dir-row">
          <Button size="sm" variant="secondary" disabled={busy} data-testid="project-pick-dir" onclick={() => void pickNewProjectDir()}>
            {en ? 'Choose project folder…' : 'Wskaż katalog projektu…'}
          </Button>
          {#if pickedDir}
            <div class="picked-dir">
              <code title={pickedDir}>{pickedDir}</code>
              <button type="button" class="clear-btn" onclick={() => (pickedDir = null)} aria-label={en ? 'Clear folder' : 'Wyczyść katalog'}>×</button>
            </div>
          {:else}
            <span class="hint">
              {en
                ? 'Choose a directory to auto-fill the project name and set the working folder.'
                : 'Wskaż folder na dysku — nazwa projektu podpowie się automatycznie.'}
            </span>
          {/if}
        </div>

        <div class="form-row">
          <input
            type="text"
            class="text-input"
            maxlength="80"
            bind:value={nameInput}
            data-testid="project-name-input"
            placeholder={en ? 'Project name (e.g. Alfa Shop)…' : 'Nazwa projektu (np. Sklep Alfa)…'}
            onkeydown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                void saveProject(targetSession ? 'assign' : 'none');
              }
            }}
          />
          <Button size="sm" variant="secondary" disabled={busy || (!nameInput.trim() && !pickedDir)} data-testid="project-save-btn" onclick={() => void saveProject('none')}>
            {en ? 'Save project' : 'Dodaj projekt'}
          </Button>
          {#if targetSession}
            <Button size="sm" variant="primary" disabled={busy || (!nameInput.trim() && !pickedDir)} data-testid="project-save-assign-btn" onclick={() => void saveProject('assign')}>
              {en ? 'Save & assign' : 'Dodaj i przypisz rozmowę'}
            </Button>
          {:else}
            <Button size="sm" variant="primary" disabled={busy || (!nameInput.trim() && !pickedDir)} data-testid="project-save-open-btn" onclick={() => void saveProject('new-session')}>
              {en ? '+ Add & start chat' : '+ Dodaj i otwórz rozmowę'}
            </Button>
          {/if}
        </div>
      </section>

      {#if loading}
        <p class="empty">…</p>
      {:else if items.length === 0}
        <p class="empty">
          {en ? 'No projects yet. Choose a project folder above to create your first project.' : 'Brak projektów. Wskaż katalog projektu powyżej, aby utworzyć pierwszy projekt.'}
        </p>
      {:else}
        <ul class="list" aria-label={en ? 'Project list' : 'Lista projektów'}>
          {#each items as proj (proj.name)}
            <li data-testid="project-row">
              <div class="row-top">
                <div class="row-title">
                  {#if editingName === proj.name}
                    <input
                      type="text"
                      class="text-input rename-input"
                      maxlength="80"
                      bind:value={editingValue}
                      onkeydown={(e) => {
                        if (e.key === 'Enter') void commitRename(proj.name);
                        if (e.key === 'Escape') {
                          e.stopPropagation();
                          editingName = null;
                        }
                      }}
                      onblur={() => void commitRename(proj.name)}
                    />
                  {:else}
                    <strong>{proj.name}</strong>
                  {/if}
                  <span class="badge">
                    {proj.session_count} {en ? (proj.session_count === 1 ? 'chat' : 'chats') : (proj.session_count === 1 ? 'rozmowa' : 'rozmów')}
                  </span>
                </div>

                <div class="actions">
                  {#if targetSession}
                    <Button size="sm" variant="primary" disabled={busy || targetSession.project?.name === proj.name} onclick={() => void assignTargetTo(proj.name)}>
                      {targetSession.project?.name === proj.name ? (en ? 'Assigned' : 'Przypisano') : (en ? 'Assign here' : 'Przypisz tutaj')}
                    </Button>
                  {/if}
                  <Button size="sm" variant="secondary" disabled={busy} onclick={() => void startSessionInProject(proj.name)}>
                    {en ? '+ New chat' : '+ Nowa rozmowa'}
                  </Button>
                  <Button size="sm" variant="secondary" disabled={busy} onclick={() => void changeProjectDir(proj)}>
                    {en ? 'Folder…' : 'Katalog…'}
                  </Button>
                  <Button size="sm" variant="ghost" disabled={busy} onclick={() => beginRename(proj)}>
                    {en ? 'Rename' : 'Zmień nazwę'}
                  </Button>
                  <Button size="sm" variant="ghost" disabled={busy} onclick={() => void removeProject(proj.name)}>
                    {en ? 'Remove' : 'Usuń'}
                  </Button>
                </div>
              </div>

              <div class="dir-line">
                <span class="muted">{en ? 'Directory:' : 'Katalog:'}</span>
                {#if proj.workdir}
                  <code title={proj.workdir}>{proj.workdir}</code>
                {:else}
                  <span class="hint">{en ? 'No directory assigned' : 'Brak wskazanego katalogu'}</span>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="footer">
        <Button variant="primary" onclick={closeProjects}>{en ? 'Close' : 'Zamknij'}</Button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.alfa-projects-overlay) { position: fixed; inset: 0; z-index: 110; background: var(--alfa-color-scrim); }
  :global(.alfa-projects) {
    position: fixed; z-index: 111; top: 50%; left: 50%; transform: translate(-50%, -50%);
    display: flex; flex-direction: column; gap: var(--alfa-space-3);
    width: min(720px, calc(var(--alfa-vw, 100vw) - 32px));
    max-height: min(calc(var(--alfa-vh, 100vh) * 0.85), 760px);
    padding: var(--alfa-space-6); border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-overlay); background: var(--alfa-color-surface);
    box-shadow: var(--alfa-shadow-3); color: var(--alfa-color-text);
  }
  :global(.alfa-projects-title) { margin: 0; font-size: var(--alfa-font-size-lg); font-weight: var(--alfa-weight-semibold); }
  :global(.alfa-projects-lead), .empty, .muted, .hint { margin: 0; color: var(--alfa-color-text-muted); font-size: var(--alfa-font-size-sm); }
  .hint { font-size: var(--alfa-font-size-xs); }
  .target-bar, .add-box {
    display: flex; flex-direction: column; gap: var(--alfa-space-2); padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border); border-radius: var(--alfa-radius-card); background: var(--alfa-color-bg);
  }
  .target-bar { flex-direction: row; align-items: center; justify-content: space-between; }
  .target-info, .dir-row, .form-row, .row-top, .row-title, .actions, .dir-line, .footer {
    display: flex; flex-wrap: wrap; align-items: center; gap: var(--alfa-space-2);
  }
  .row-top { justify-content: space-between; }
  .section-title { margin: 0; font-size: var(--alfa-font-size-sm); font-weight: var(--alfa-weight-semibold); }
  .picked-dir {
    display: flex; flex: 1; min-width: 0; align-items: center; gap: var(--alfa-space-2);
    padding: 2px var(--alfa-space-2); border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-control); font-size: var(--alfa-font-size-xs);
  }
  .picked-dir code, .dir-line code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--alfa-font-size-xs); }
  .clear-btn { margin-left: auto; border: 0; background: transparent; color: var(--alfa-color-text-muted); cursor: pointer; }
  .text-input {
    flex: 1; min-width: 180px; height: 32px; padding: 0 var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border); border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-surface); color: var(--alfa-color-text); font-size: var(--alfa-font-size-sm);
  }
  .rename-input { height: 28px; }
  .badge {
    padding: 1px var(--alfa-space-2); border-radius: var(--alfa-radius-full);
    background: var(--alfa-color-surface2); color: var(--alfa-color-text-muted); font-size: var(--alfa-font-size-xs);
  }
  .list {
    display: flex; flex: 1 1 auto; flex-direction: column; gap: var(--alfa-space-2);
    min-height: 0; margin: 0; padding: 0; overflow: auto; list-style: none;
  }
  .list > li {
    display: flex; flex-direction: column; gap: var(--alfa-space-1); padding: var(--alfa-space-3);
    border: 1px solid var(--alfa-color-border); border-radius: var(--alfa-radius-card);
  }
  .footer { justify-content: flex-end; }
</style>
