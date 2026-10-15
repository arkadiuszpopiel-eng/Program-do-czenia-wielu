// Board — pinned replies (keys must match pl-board.ts).
import type { Message } from './core';
import type { plBoard } from './pl-board';

export const enBoard: Record<keyof typeof plBoard, Message> = {
  'action.board': 'Show the Board (pinned replies)',
  'board.title': 'Board',
  'board.lead':
    'Pinned agent replies from different conversations — up to 40. Pin from the message menu.',
  'board.empty': 'Nothing pinned yet. In an agent reply’s menu choose “Pin to board”.',
  'board.list': 'Pinned replies',
  'board.pin': 'Pin to board',
  'board.pinned': 'Pinned to the Board (Ctrl+K → “Show the Board”).',
  'board.open': 'Open conversation',
  'board.copy': 'Copy',
  'board.copied': 'Copied to the clipboard.',
  'board.unpin': 'Unpin',
  'msg.translateEn': 'Translate to English',
  'msg.translatePl': 'Translate to Polish',
  'msg.translatePromptEn':
    'Translate the text below into English. Reply with the translation only, keep the formatting.\n\n{text}',
  'msg.translatePromptPl':
    'Translate the text below into Polish. Reply with the translation only, keep the formatting.\n\n{text}',
  'board.close': 'Close',
};
