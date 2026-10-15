import { describe, expect, it } from 'vitest';
import {
  agentName,
  b64ToBytes,
  binaryToB64,
  emptyDraft,
  isStandardAgent,
  loginBridge,
  loginProfile,
  loginSignal,
  stripAnsi,
  paramsTemplate,
  textToB64,
} from '../work';

describe('pomocniki F8', () => {
  it('kodowanie terminala: UTF-8 (polskie znaki) ↔ base64 ↔ bajty', () => {
    const b64 = textToB64('zażółć ✓');
    const bytes = b64ToBytes(b64);
    expect(new TextDecoder().decode(bytes)).toBe('zażółć ✓');
    expect(b64ToBytes(binaryToB64('\x1b[M !!'))).toEqual(
      new Uint8Array([0x1b, 0x5b, 0x4d, 0x20, 0x21, 0x21]),
    );
  });

  it('imię agentki: obsada standardowa i persony z Kreatora', () => {
    expect(agentName('delta')).toBe('Delta');
    expect(agentName('zofia')).toBe('zofia');
    expect(isStandardAgent('gama')).toBe(true);
    expect(isStandardAgent('toString')).toBe(false);
  });

  it('profil logowania mostu CLI', () => {
    expect(loginProfile('claude_code')).toBe('claude_login');
    expect(loginProfile('codex')).toBe('codex_login');
    expect(loginProfile('antigravity')).toBe('agy_login');
    expect(loginBridge('agy_login')).toBe('antigravity');
    expect(loginBridge('shell')).toBeNull();
    expect(loginProfile('gemini')).toBeNull();
  });

  it('szablon parametrów ze schematu: domyślne i wymagane', () => {
    expect(
      paramsTemplate({
        type: 'object',
        properties: {
          folder: { type: 'string' },
          limit: { type: 'integer' },
          mode: { type: 'string', default: 'szybki' },
          extra: { type: 'string' },
        },
        required: ['folder', 'limit'],
      }),
    ).toEqual({ folder: '', limit: 0, mode: 'szybki' });
    expect(paramsTemplate(null)).toEqual({});
  });

  it('pusty szkic Kreatora: autonomia L2, bez zapisu', () => {
    const d = emptyDraft();
    expect(d.limits.autonomy).toBe('L2');
    expect(d.limits.fs_write).toEqual([]);
    expect(d.role?.tools).toEqual([]);
  });

  it('wykrycie zalogowania w oknie logowania — plan bez adresu e-mail', () => {
    const esc = String.fromCharCode(27);
    const header = `${esc}[34mAntigravity CLI 1.3.5${esc}[0m\r\nala@example.com (Google AI Pro)\r\n`;
    expect(loginSignal('agy_login', header)).toEqual({ plan: 'Google AI Pro' });
    expect(JSON.stringify(loginSignal('agy_login', header))).not.toContain('@');
    expect(loginSignal('agy_login', 'Do you trust this folder?')).toBeNull();
    expect(loginSignal('claude_login', 'Login successful. Press Enter')).toEqual({ plan: null });
    expect(loginSignal('codex_login', 'Successfully logged in')).toEqual({ plan: null });
    expect(loginSignal('shell', 'Login successful')).toBeNull();
    expect(stripAnsi(`${esc}[1;34mA${esc}[0m${esc}[2KB`)).toBe('AB');
  });
});
