// Artifact canvas C2.8 (keys must match pl-canvas.ts).
import type { Message } from './core';
import type { plCanvas } from './pl-canvas';

export const enCanvas: Record<keyof typeof plCanvas, Message> = {
  'canvas.open': 'Open in canvas',
  'canvas.openNamed': 'Open in canvas: {title}',
  'canvas.openDoc': 'Open reply in canvas',
  'canvas.title': 'Artifact canvas',
  'canvas.views': 'Artifact view',
  'canvas.preview': 'Preview',
  'canvas.source': 'Source',
  'canvas.copy': 'Copy source',
  'canvas.copied': 'Artifact source copied.',
  'canvas.save': 'Save as file',
  'canvas.close': 'Close canvas (Esc)',
  'canvas.kind.html': 'HTML page',
  'canvas.kind.svg': 'SVG drawing',
  'canvas.kind.markdown': 'Markdown document',
  'canvas.frameTitle': 'Preview: {title}',
  'canvas.isolated':
    'Preview in an isolated frame: no scripts, no access to Alfa and no downloads from the network.',
  'canvas.tooLarge':
    'The artifact is {size} — more than the preview limit ({limit}). You can copy the source or save it as a file.',
  'canvas.truncated':
    'Showing the first {limit} of the source — copy it or save it as a file to get all of it.',
  'canvas.loading': 'Preparing preview…',
};
