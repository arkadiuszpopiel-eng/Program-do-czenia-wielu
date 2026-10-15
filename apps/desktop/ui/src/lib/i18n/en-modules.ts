// Settings → Modules (keys must match pl-modules.ts).
import type { Message } from './core';
import type { plModules } from './pl-modules';

export const enModules: Record<keyof typeof plModules, Message> = {
  'modules.title': 'Modules',
  'modules.intro':
    'Alfa is made of modules: each provides one part of the program, has a memory and processor budget and its own state. Kernel modules (consents, supervision, audit, updates) are protected. Some modules can be turned off to save memory — the change takes effect after the next start.',
  'modules.summary': '{n} modules · memory budget of enabled ones: about {ram} GB',
  'modules.restart': 'Changes take effect after Alfa restarts.',
  'modules.search': 'Search modules',
  'modules.optional': 'Optional modules',
  'modules.optionalHint':
    'Conversation keeps working without them; dependent features show “module not connected”.',
  'modules.all': 'Other modules',
  'modules.protected': 'Kernel — protected',
  'modules.enabledFor': 'Module {id} enabled',
  'modules.capabilities': 'capabilities',
  'modules.ram': 'memory {mb} MB',
  'modules.vram': 'GPU {mb} MB',
  'modules.state.ready': 'running',
  'modules.state.unloaded': 'waiting for use',
  'modules.state.loading': 'starting',
  'modules.state.degraded': 'running with limitations',
  'modules.state.failed': 'error',
  'modules.state.disabled': 'disabled',
  'modules.kind.service': 'service',
  'modules.kind.tool': 'tool',
  'modules.kind.provider': 'model provider',
  'modules.kind.voice-engine': 'voice engine',
  'modules.kind.agent-pack': 'agent pack',
  'modules.kind.ui-panel': 'panel',
  'modules.lifecycle.always': 'always',
  'modules.lifecycle.lazy': 'on first use',
  'modules.lifecycle.on-demand': 'on demand',
};
