// Katalog dostawców w atrapie — odwzorowanie `providers-catalog/*.toml` (stan z 30.09.2026).
// W buildzie z Tauri katalog czyta rdzeń (accounts-hub); UI dostaje go komendą `accounts_catalog`.
import type { ProviderInfo } from '../types-hub';

type Row = [
  id: string,
  name: string,
  kind: ProviderInfo['kind'],
  compat: ProviderInfo['compat'],
  privacy: string,
  jurisdiction: string,
  compliance: ProviderInfo['compliance_status'],
];

const ROWS: readonly Row[] = [
  ['anthropic', 'Anthropic (Claude)', 'chat', 'native', 'unknown', 'unknown', 'unverified'],
  ['openai', 'OpenAI', 'multi', 'native', 'unknown', 'unknown', 'unverified'],
  ['google', 'Google (Gemini)', 'multi', 'openai', 'google-paid-eea-no-train', 'unknown', 'gray'],
  ['xai', 'xAI (Grok)', 'multi', 'openai', 'xai-retention-30d', 'unknown', 'unverified'],
  ['deepseek', 'DeepSeek', 'chat', 'openai', 'cn-may-train', 'CN', 'unverified'],
  ['kimi', 'Kimi (Moonshot)', 'chat', 'openai', 'cn-may-train', 'CN', 'unverified'],
  ['qwen', 'Qwen (Alibaba DashScope)', 'multi', 'openai', 'sg', 'SG|EU', 'unverified'],
  ['zai', 'Z.ai (GLM)', 'chat', 'openai', 'sg', 'SG', 'unverified'],
  ['minimax', 'MiniMax', 'multi', 'openai', 'unknown', 'unknown', 'unverified'],
  ['openrouter', 'OpenRouter', 'chat', 'openai', 'unknown', 'unknown', 'unverified'],
  ['mistral', 'Mistral', 'chat', 'openai', 'unknown', 'unknown', 'unverified'],
  ['elevenlabs', 'ElevenLabs', 'multi', 'native', 'unknown', 'unknown', 'unverified'],
  ['cartesia', 'Cartesia', 'tts', 'native', 'unknown', 'unknown', 'unverified'],
  ['azure-speech', 'Azure Speech', 'tts', 'native', 'unknown', 'unknown', 'unverified'],
  ['soniox', 'Soniox', 'stt', 'native', 'unknown', 'unknown', 'unverified'],
  [
    'custom-openai-compatible',
    'Własny endpoint (zgodny z OpenAI)',
    'chat',
    'openai',
    'unknown',
    'unknown',
    'unverified',
  ],
  [
    'custom-anthropic-compatible',
    'Własny endpoint (zgodny z Anthropic)',
    'chat',
    'anthropic',
    'unknown',
    'unknown',
    'unverified',
  ],
];

// Jak w rdzeniu: adresu wymaga każdy wpis bez znanego `base_url` w katalogu — dziś wszystkie
// poza tymi trzema (test na laptopie 2026-10-07: rozjazd atrapy z rdzeniem ukrył błąd kreatora).
const KNOWN_ENDPOINTS: ReadonlySet<string> = new Set(['anthropic', 'openai', 'xai']);

export const FAKE_CATALOG: readonly ProviderInfo[] = ROWS.map(
  ([id, display_name, kind, compat, privacy_tag, jurisdiction, compliance_status]) => ({
    id,
    display_name,
    kind,
    auth: 'api_key',
    compat,
    privacy_tag,
    jurisdiction,
    compliance_status,
    terms_url: id === 'google' ? 'https://geminicli.com/docs/resources/tos-privacy/' : null,
    needs_base_url: !KNOWN_ENDPOINTS.has(id),
  }),
);
