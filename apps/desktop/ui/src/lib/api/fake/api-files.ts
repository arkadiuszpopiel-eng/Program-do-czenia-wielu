// Atrapa: załączniki composera, eksport rozmowy i kopie zapasowe. Deterministyczna (zegar atrapy).
// Załączniki: limity jak w rdzeniu (10 plików, 25 MB pliku, 100 MB razem), rodzaj z typu MIME,
// szacunek tokenów (tekst: bajty / 4, obraz ≈ 1600); „wybór” dodaje kolejne pliki z listy,
// „wklejenie” — zrzut PNG, przeciągnięcie — pliki z `DataTransfer` przeglądarki (wskazówki).
// Kopie: katalog z „dialogu”, rotacja N najnowszych, hasło ≥ 8 znaków, test przywracania.
import type { AttachmentsApi, BackupsApi, ConversationExportApi } from '../client-files';
import type { ExportResult } from '../types-hub';
import type {
  AttachmentInfo,
  AttachmentKind,
  AttachmentRejection,
  AttachmentsAdded,
  BackupConfig,
  BackupEntry,
  BackupView,
  DroppedFileHint,
  TurnAttachment,
} from '../types-files';
import type { FakeCore } from './core';

const MB = 1024 * 1024;
export const ATTACHMENT_LIMITS = {
  maxFiles: 10,
  maxFileBytes: 25 * MB,
  maxTotalBytes: 100 * MB,
  maxImageBytes: 5 * MB,
  maxTextChars: 100_000,
} as const;
const IMAGE_TOKENS = 1_600;
const LABEL_TOKENS = 40;
const IMAGES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];
const PICKS: readonly DroppedFileHint[] = [
  { name: 'raport-q3.pdf', bytes: 1_240_000, mime: 'application/pdf' },
  { name: 'wykres-sprzedazy.png', bytes: 245_000, mime: 'image/png' },
  { name: 'notatki.md', bytes: 18_400, mime: 'text/markdown' },
];

function kindOf(mime: string): AttachmentKind {
  if (IMAGES.includes(mime)) return 'image';
  if (mime.startsWith('text/') || /json|toml|xml|yaml/.test(mime)) return 'text';
  return 'document';
}

/** Rodzaj, dostawa i tokeny — ta sama heurystyka co `app-files::attach::classify`. */
export function classifyHint(
  hint: DroppedFileHint,
): Pick<AttachmentInfo, 'kind' | 'delivery' | 'tokens'> {
  const kind = kindOf(hint.mime);
  if (kind === 'image') {
    return hint.bytes > ATTACHMENT_LIMITS.maxImageBytes
      ? { kind, delivery: 'metadata_only', tokens: LABEL_TOKENS }
      : { kind, delivery: 'full', tokens: IMAGE_TOKENS + LABEL_TOKENS };
  }
  if (kind === 'text') {
    const chars = Math.min(hint.bytes, ATTACHMENT_LIMITS.maxTextChars);
    return {
      kind,
      delivery: hint.bytes > ATTACHMENT_LIMITS.maxTextChars ? 'truncated' : 'full',
      tokens: Math.ceil(chars / 4) + LABEL_TOKENS,
    };
  }
  return { kind, delivery: 'metadata_only', tokens: LABEL_TOKENS };
}

/** Wysyłanie tury w atrapie: przygotowane załączniki → załączniki tury (znikają z composera). */
export function takeStaged(
  core: FakeCore,
  sessionId: string,
  ids: readonly string[] | undefined,
): TurnAttachment[] {
  if (!ids?.length) return [];
  const staged = core.staged.get(sessionId) ?? [];
  const chosen = staged.filter((a) => ids.includes(a.id));
  if (chosen.length !== ids.length) throw new Error('Nieznany załącznik.');
  core.staged.set(
    sessionId,
    staged.filter((a) => !ids.includes(a.id)),
  );
  return chosen.map((a, i) => ({
    name: a.name,
    mime: a.mime,
    kind: a.kind,
    bytes: a.bytes,
    artifact_id: `art-${sessionId}-${core.scheduler.now().toString(36)}-${i}`,
  }));
}

export class FakeFiles {
  private picks = 0;
  private backup: BackupView = {
    config: {
      enabled: false,
      dir: null,
      interval_hours: 24,
      keep: 7,
      include_artifacts: false,
      include_logs: false,
      skip_on_battery: true,
    },
    password_set: false,
    entries: [],
    last_run: null,
    last_error: null,
    next_due: null,
    running: false,
  };

  constructor(private readonly core: FakeCore) {}

  private staged(sessionId: string): AttachmentInfo[] {
    return this.core.staged.get(sessionId) ?? [];
  }

  private add(sessionId: string, hints: readonly DroppedFileHint[]): Promise<AttachmentsAdded> {
    const { core } = this;
    if (!core.session(sessionId)) return Promise.reject(new Error('Nieznana sesja.'));
    const list = [...this.staged(sessionId)];
    const added: AttachmentInfo[] = [];
    const rejected: AttachmentRejection[] = [];
    for (const hint of hints) {
      const total = list.reduce((sum, a) => sum + a.bytes, 0);
      const reason =
        hint.bytes === 0
          ? 'empty'
          : hint.bytes > ATTACHMENT_LIMITS.maxFileBytes
            ? 'too_large'
            : list.length >= ATTACHMENT_LIMITS.maxFiles
              ? 'too_many'
              : total + hint.bytes > ATTACHMENT_LIMITS.maxTotalBytes
                ? 'total_too_large'
                : /(^|[\\/])\.(claude|codex|ssh)([\\/]|$)/i.test(hint.name)
                  ? 'denied'
                  : null;
      if (reason) {
        rejected.push({ name: hint.name, reason });
        continue;
      }
      const info: AttachmentInfo = {
        id: core.nextId('att'),
        session_id: sessionId,
        name: hint.name,
        bytes: hint.bytes,
        mime: hint.mime || 'application/octet-stream',
        path: `C:\\Users\\Ty\\Alfa\\Sesje\\${sessionId}\\in\\${hint.name}`,
        ...classifyHint(hint),
      };
      list.push(info);
      added.push(info);
    }
    core.staged.set(sessionId, list);
    return core.reply({ added, rejected, staged: list });
  }

  attachmentsApi(): AttachmentsApi {
    return {
      pick: (sessionId) => {
        const hint = PICKS[this.picks % PICKS.length];
        this.picks++;
        return this.add(sessionId, hint ? [hint] : []);
      },
      addDropped: (sessionId, hints) => this.add(sessionId, hints),
      paste: (sessionId) => {
        const stamp = new Date(this.core.scheduler.now()).toISOString().slice(0, 19);
        const name = `wklejony-obraz-${stamp.replace(/[-:]/g, '').replace('T', '-')}.png`;
        return this.add(sessionId, [{ name, bytes: 182_000, mime: 'image/png' }]);
      },
      list: (sessionId) => this.core.reply(this.staged(sessionId)),
      remove: (sessionId, attachmentId) => {
        const left = this.staged(sessionId).filter((a) => a.id !== attachmentId);
        this.core.staged.set(sessionId, left);
        return this.core.reply(left);
      },
      watchDrag: () => () => undefined,
      previewUrl: () => null,
    };
  }

  conversationApi(): ConversationExportApi {
    return {
      exportConversation: (sessionId, format, turnId) => {
        const { core } = this;
        const session = core.session(sessionId);
        const turns = core.turnsOf(sessionId).filter((t) => !turnId || t.id === turnId);
        if (!session || turns.length === 0) {
          return Promise.reject(new Error('Brak wiadomości do eksportu.'));
        }
        const ext = format === 'markdown' ? 'md' : 'html';
        const day = new Date(core.scheduler.now()).toISOString().slice(0, 10);
        const result: ExportResult = {
          status: 'saved',
          path: `C:\\Users\\Ty\\Documents\\${session.title} ${day}.${ext}`,
          files: 1,
          bytes: turns.reduce((sum, t) => sum + t.text.length, format === 'html' ? 1_400 : 80),
        };
        return core.reply(result);
      },
    };
  }

  private view(patch: Partial<BackupView> = {}): Promise<BackupView> {
    this.backup = { ...this.backup, ...patch };
    const c = this.backup.config;
    const last = this.backup.last_run ? Date.parse(this.backup.last_run) : null;
    const due = c.enabled
      ? new Date((last ?? this.core.scheduler.now()) + (last ? c.interval_hours * 3_600_000 : 0))
      : null;
    this.backup = { ...this.backup, next_due: due ? due.toISOString() : null };
    return this.core.reply(this.backup);
  }

  backupsApi(): BackupsApi {
    return {
      status: () => this.view(),
      configure: (config: BackupConfig) => {
        const dir = this.backup.config.dir;
        const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
        return this.view({
          config: {
            ...config,
            dir,
            enabled: config.enabled && dir !== null,
            interval_hours: clamp(config.interval_hours, 1, 720),
            keep: clamp(config.keep, 1, 100),
          },
        });
      },
      chooseDir: () => this.view({ config: { ...this.backup.config, dir: 'D:\\Kopie Alfy' } }),
      setPassword: (password) => {
        if (password !== null && password.length < 8) {
          return Promise.reject(new Error('Hasło musi mieć co najmniej 8 znaków.'));
        }
        return this.view({ password_set: password !== null });
      },
      runNow: () => {
        const { dir, keep } = this.backup.config;
        if (!dir) return Promise.reject(new Error('Najpierw wybierz katalog kopii zapasowych.'));
        const at = new Date(this.core.scheduler.now() + this.backup.entries.length * 1_000);
        const stamp = at.toISOString().replace(/[-:]/g, '').replace('T', '-').replace('.', '-');
        const file = `alfa-backup-${stamp.slice(0, 19)}.alfa`;
        const entry: BackupEntry = {
          file,
          path: `${dir}\\${file}`,
          created_at: at.toISOString(),
          bytes: 48_000 + this.core.sessions.length * 12_000,
        };
        return this.view({
          entries: [entry, ...this.backup.entries].slice(0, keep),
          last_run: at.toISOString(),
          last_error: null,
        });
      },
      verify: (file) => {
        const entry = this.backup.entries.find((e) => e.file === file);
        if (!entry) return Promise.reject(new Error(`Brak kopii „${file}” w katalogu kopii.`));
        return this.core.reply({
          file,
          ok: true,
          encrypted: this.backup.password_set,
          created_at: entry.created_at,
          app_version: '0.1.0',
          items: this.core.sessions.length + 6,
          sessions: this.core.sessions.length,
          message: null,
        });
      },
    };
  }
}
