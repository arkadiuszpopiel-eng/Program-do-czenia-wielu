// English dictionary: Settings → “Models and engines” → bundles 1–6, “Repair” of a single element,
// the selection guide and quality notes per standards.
import type { Message } from './core';
import type { plBundles } from './pl-bundles';

export const enBundles: Record<keyof typeof plBundles, Message> = {
  'bundles.title': 'Bundles — from 6 (reference) to 1 (minimal)',
  'bundles.intro':
    'A bundle is a complete set of models and engines for this computer: 6 is the full local Alfa (voice conversation, agents with tools, voice verification), 1 is text chat only. Alfa picks the engine builds itself (NVIDIA GPU — CUDA, AMD or Intel GPU — Vulkan, no GPU — CPU). You can also download, check or repair each element on its own.',
  'bundles.loading': 'Loading bundles…',
  'bundles.machine': 'This computer: {gpu} · RAM {ram} · CPU: {cores}',
  'bundles.machine.gpu': '{name} ({vram})',
  'bundles.machine.noGpu': 'no GPU',
  'bundles.cores': { one: '{n} core', other: '{n} cores' },
  'bundles.guide.title': 'How to choose a bundle — recommendations',
  'bundles.guide.recommended':
    'Start with the bundle marked “Recommended” — the highest rating that runs on this computer without compromises.',
  'bundles.guide.tight':
    '“Tight” means it works, but slower (e.g. part of the model runs on the CPU). For fluent voice conversation pick a bundle that fits fully.',
  'bundles.guide.agents':
    'Agents with tools (files, commands, windows) need Bielik 4.5B — bundles 5–6 — or a cloud model.',
  'bundles.guide.shared':
    'Bundles do not exclude each other: shared elements are downloaded once. You can start with a smaller one and add a larger one later.',
  'bundles.guide.repair':
    'Every file is checked with SHA-256. When an element gets damaged or installs badly, use “Repair” on that element — the rest of the bundle stays as it is.',
  'bundles.guide.standards':
    'Quality notes refer to international standards (ISO/IEC 25010 and 25059, ITU-T P.800/P.808 and G.114, ISO/IEC 19795-1, WER) as recommendations or measurement methods — they are not certificates.',
  'bundles.rating': 'Rating {n} of 6',
  'bundles.recommended': 'Recommended for this computer',
  'bundles.fit.fits': 'Fits this computer',
  'bundles.fit.tight': 'Tight — works with a compromise',
  'bundles.fit.too_weak': 'Hardware too weak',
  'bundles.requirements': 'Requirements: {text}',
  'bundles.size': 'Size: {size}',
  'bundles.missing': 'to download: {size}',
  'bundles.count': {
    one: '{done} of {n} element installed',
    other: '{done} of {n} elements installed',
  },
  'bundles.progress': 'Progress of bundle {name}',
  'bundles.state.not_installed': 'Not downloaded',
  'bundles.state.partial': 'Partly installed',
  'bundles.state.installed': 'Installed',
  'bundles.state.corrupt': 'An element is damaged or failed — repair it',
  'bundles.state.downloading': 'Downloading in the background…',
  'bundles.state.needs_trust':
    'Waiting for your consent: approve files without a pinned SHA-256 in the catalog below',
  'bundles.download': 'Download bundle',
  'bundles.resume': 'Finish downloading',
  'bundles.repair': 'Repair bundle',
  'bundles.verify': 'Check files (SHA-256)',
  'bundles.started':
    'Downloading bundle “{name}” in the background — progress shows next to the elements.',
  'bundles.verified': 'Checked bundle “{name}”: {state}.',
  'bundles.weak.title': 'Bundle “{name}” is too demanding for this computer',
  'bundles.weak.body':
    '{reason} You can download it, but some engines will not start or will run very slowly. Recommended here: {recommended}.',
  'bundles.weak.none': 'none — consider a cloud model',
  'bundles.weak.confirm': 'Download anyway',
  'bundles.items': 'Bundle elements ({n})',
  'bundles.kind.llm': 'chat model',
  'bundles.kind.stt': 'speech recognition',
  'bundles.kind.tts': 'voice (speech synthesis)',
  'bundles.kind.vad': 'voice activity detection',
  'bundles.kind.wake': 'wake word',
  'bundles.kind.speaker': 'voice verification',
  'bundles.kind.embed': 'semantic search',
  'bundles.kind.sidecar': 'engine',
  'bundles.item.fallback': 'fallback: CPU',
  'bundles.item.manual': 'manual install — see the catalog below',
  'bundles.item.trust': 'approve in the catalog below',
  'bundles.item.download': 'Download',
  'bundles.item.resume': 'Resume',
  'bundles.item.repair': 'Repair',
  'bundles.item.verify': 'Check',
  'bundles.item.actions': 'Actions: {name}',
  'bundles.quality': 'Quality and standards ({n})',
  'bundles.quality.disclaimer':
    'Standards are cited as recommendations or measurement methods, without claiming certification. Values “to be measured” become known after measuring on this computer.',
  'engines.repair': 'Repair',
  'engines.repairing': 'Repairing “{name}”: removed the files and downloading again.',
  'engines.repairConfirm.title': 'Repair “{name}”?',
  'engines.repairConfirm.body':
    'Alfa will delete this element’s files (including partial downloads) and download it again — {size}. Other elements stay unchanged.',
};
