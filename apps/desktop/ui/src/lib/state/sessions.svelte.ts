// Lista sesji panelu lewego: grupy (przypięte, projekty, bez projektu, archiwum), sortowanie,
// „karty" jako lista ostatnio używanych (Ctrl+Tab / Ctrl+1…9 / Ctrl+W / Ctrl+Shift+T).
import type { ProjectRef, SessionSearchHit, SessionSummary } from '../api/types';

export interface SessionGroup {
  readonly id: string;
  readonly kind: 'pinned' | 'project' | 'loose' | 'archived';
  readonly project: ProjectRef | null;
  readonly sessions: readonly SessionSummary[];
}

const byUpdated = (a: SessionSummary, b: SessionSummary): number =>
  a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0;

export function groupSessions(
  list: readonly SessionSummary[],
  showArchived: boolean,
): SessionGroup[] {
  const live = list.filter((s) => !s.archived).sort(byUpdated);
  const groups: SessionGroup[] = [];
  const pinned = live.filter((s) => s.pinned);
  if (pinned.length) groups.push({ id: 'pinned', kind: 'pinned', project: null, sessions: pinned });
  const projects: Record<string, { project: ProjectRef; sessions: SessionSummary[] }> = {};
  const order: string[] = [];
  const loose: SessionSummary[] = [];
  for (const s of live) {
    if (s.pinned) continue;
    if (!s.project) {
      loose.push(s);
      continue;
    }
    const entry = projects[s.project.id];
    if (entry) entry.sessions.push(s);
    else {
      projects[s.project.id] = { project: s.project, sessions: [s] };
      order.push(s.project.id);
    }
  }
  for (const id of order) {
    const entry = projects[id];
    if (entry)
      groups.push({
        id: `p:${id}`,
        kind: 'project',
        project: entry.project,
        sessions: entry.sessions,
      });
  }
  if (loose.length) groups.push({ id: 'loose', kind: 'loose', project: null, sessions: loose });
  if (showArchived) {
    const archived = list.filter((s) => s.archived).sort(byUpdated);
    if (archived.length)
      groups.push({ id: 'archived', kind: 'archived', project: null, sessions: archived });
  }
  return groups;
}

export class SessionsState {
  list = $state<SessionSummary[]>([]);
  activeId = $state<string | null>(null);
  drafts = $state<Record<string, string>>({});
  showArchived = $state(false);
  query = $state('');
  hits = $state<SessionSearchHit[]>([]);
  renamingId = $state<string | null>(null);
  readonly active: SessionSummary | null = $derived(
    this.list.find((s) => s.id === this.activeId) ?? null,
  );
  readonly groups: SessionGroup[] = $derived(groupSessions(this.list, this.showArchived));
  /** Ostatnio używane (najnowsza na początku) i zamknięte „karty". */
  private mru: string[] = [];
  private closed: string[] = [];

  upsert(session: SessionSummary): void {
    const index = this.list.findIndex((s) => s.id === session.id);
    if (index >= 0) this.list[index] = session;
    else this.list = [session, ...this.list];
  }

  remove(id: string): void {
    this.list = this.list.filter((s) => s.id !== id);
    this.mru = this.mru.filter((x) => x !== id);
  }

  touch(id: string): void {
    this.mru = [id, ...this.mru.filter((x) => x !== id)].slice(0, 20);
  }

  /** Karta n (1…9) = n-ta ostatnio używana sesja. */
  tab(n: number): string | undefined {
    return this.recent()[n - 1];
  }

  /** Następna/poprzednia karta względem aktywnej. */
  cycle(delta: 1 | -1): string | undefined {
    const recent = this.recent();
    if (recent.length < 2) return undefined;
    const at = Math.max(0, recent.indexOf(this.activeId ?? ''));
    return recent[(at + delta + recent.length) % recent.length];
  }

  /** Ctrl+W: zamknij kartę — wróć do poprzedniej sesji. */
  close(id: string): string | undefined {
    this.closed.push(id);
    this.mru = this.mru.filter((x) => x !== id);
    return this.recent().find((x) => x !== id);
  }

  reopen(): string | undefined {
    return this.closed.pop();
  }

  private recent(): string[] {
    const alive = this.list.filter((s) => !s.archived).map((s) => s.id);
    const out = this.mru.filter((id) => alive.includes(id));
    for (const id of alive) if (!out.includes(id)) out.push(id);
    return out;
  }
}
