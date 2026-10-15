// Ikona każdej strony Ustawień (lista w nawigacji). Strona bez wpisu — np. dodana przez moduł —
// dostaje ikonę ogólną, więc nigdy nie zostaje bez znaku.
import type { Component } from 'svelte';
import ArrowLeftRight from '@lucide/svelte/icons/arrow-left-right';
import Bell from '@lucide/svelte/icons/bell';
import Blocks from '@lucide/svelte/icons/blocks';
import Brain from '@lucide/svelte/icons/brain';
import Cable from '@lucide/svelte/icons/cable';
import Stethoscope from '@lucide/svelte/icons/stethoscope';
import CalendarClock from '@lucide/svelte/icons/calendar-clock';
import Camera from '@lucide/svelte/icons/camera';
import Cpu from '@lucide/svelte/icons/cpu';
import Download from '@lucide/svelte/icons/download';
import FolderOpen from '@lucide/svelte/icons/folder-open';
import GraduationCap from '@lucide/svelte/icons/graduation-cap';
import Info from '@lucide/svelte/icons/info';
import Keyboard from '@lucide/svelte/icons/keyboard';
import KeyRound from '@lucide/svelte/icons/key-round';
import Languages from '@lucide/svelte/icons/languages';
import ListOrdered from '@lucide/svelte/icons/list-ordered';
import MessagesSquare from '@lucide/svelte/icons/messages-square';
import Mic from '@lucide/svelte/icons/mic';
import MonitorSpeaker from '@lucide/svelte/icons/monitor-speaker';
import MousePointerClick from '@lucide/svelte/icons/mouse-pointer-click';
import Network from '@lucide/svelte/icons/network';
import Palette from '@lucide/svelte/icons/palette';
import PlugZap from '@lucide/svelte/icons/plug-zap';
import Puzzle from '@lucide/svelte/icons/puzzle';
import Route from '@lucide/svelte/icons/route';
import ScrollText from '@lucide/svelte/icons/scroll-text';
import ShieldCheck from '@lucide/svelte/icons/shield-check';
import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
import Sparkles from '@lucide/svelte/icons/sparkles';
import SquareTerminal from '@lucide/svelte/icons/square-terminal';
import Users from '@lucide/svelte/icons/users';
import Wallet from '@lucide/svelte/icons/wallet';
import WandSparkles from '@lucide/svelte/icons/wand-sparkles';
import Zap from '@lucide/svelte/icons/zap';
import Wrench from '@lucide/svelte/icons/wrench';

type Icon = Component<{ size?: number | string; strokeWidth?: number | string }>;

const ICONS: Record<string, Icon> = {
  general: SlidersHorizontal,
  appearance: Palette,
  language: Languages,
  shortcuts: Keyboard,
  notifications: Bell,
  connections: PlugZap,
  providers: KeyRound,
  api: ListOrdered,
  bridges: SquareTerminal,
  models: Cpu,
  costs: Wallet,
  router: Route,
  voice: Mic,
  devices: MonitorSpeaker,
  cameras: Camera,
  agents: Users,
  builder: WandSparkles,
  skills: GraduationCap,
  plugins: Puzzle,
  mcp: Cable,
  diag: Stethoscope,
  triggers: CalendarClock,
  marshal: Network,
  permissions: ShieldCheck,
  computer: MousePointerClick,
  memory: Brain,
  sessions: MessagesSquare,
  files: FolderOpen,
  transfer: ArrowLeftRight,
  logs: ScrollText,
  improve: Sparkles,
  modules: Blocks,
  updates: Download,
  advanced: Wrench,
  commands: Zap,
  about: Info,
};

export function settingsIcon(pageId: string): Icon {
  return ICONS[pageId] ?? SlidersHorizontal;
}
