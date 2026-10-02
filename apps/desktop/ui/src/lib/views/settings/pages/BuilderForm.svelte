<!--
  Formularz szkicu agentki (Kreator): imię, charakter, kolor z palety, głos v0 (mówczyni bazowa,
  wysokość, tempo), rola (nazwa, instrukcja, polityka modelu, grupy narzędzi z polityki Kreatora,
  tylko odczyt) i limity (autonomia ≤ sufit z Brokera, zakresy zapisu, retencja pamięci).
  Każda zmiana → `onchange(szkic)`; podgląd i test na sucho liczy rdzeń.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { Checkbox, Select, TextField } from '@alfa/ui-kit';
  import type { AutonomyLevel } from '../../../api/types';
  import type { AgentDraft, BuilderPolicyView } from '../../../api/types-work';
  import { useApp } from '../../../state/context';

  interface Props {
    initial: AgentDraft;
    policy: BuilderPolicyView;
    onchange: (draft: AgentDraft) => void;
  }

  let { initial, policy, onchange }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const LEVELS: readonly AutonomyLevel[] = ['L0', 'L1', 'L2', 'L3', 'L4'];
  const start = untrack(() => initial);
  let name = $state(start.name ?? '');
  let character = $state(start.character ?? '');
  let color = $state(start.color ?? '');
  let voiceBase = $state(start.voice?.base ?? 'pl-f1');
  let pitch = $state(start.voice?.pitch ?? 1);
  let rate = $state(start.voice?.rate ?? 1);
  let roleName = $state(start.role?.name ?? '');
  let prompt = $state(start.role?.prompt ?? '');
  let modelPolicy = $state(start.role?.model_policy ?? 'conversation');
  let tools = $state<string[]>([...(start.role?.tools ?? [])]);
  let readOnly = $state(start.role?.read_only ?? false);
  let autonomy = $state<string>(start.limits.autonomy ?? 'L2');
  let fsWrite = $state(start.limits.fs_write.join('\n'));
  let retain = $state(String(start.limits.retain_days ?? 30));
  const levels = $derived(LEVELS.slice(0, LEVELS.indexOf(policy.ceiling) + 1));

  $effect(() => {
    const role = start.role;
    const next: AgentDraft = {
      ...start,
      name: name.trim() || null,
      character: character.trim() || null,
      color: color || null,
      voice: { ...(start.voice ?? emptyVoice()), base: voiceBase, pitch, rate },
      role: {
        id: role?.id ?? '',
        description: role?.description ?? '',
        untrusted_isolated: role?.untrusted_isolated ?? false,
        author: role?.author ?? false,
        name: roleName.trim(),
        prompt: prompt.trim(),
        model_policy: modelPolicy,
        tools: [...tools],
        read_only: readOnly,
      },
      limits: {
        ...start.limits,
        autonomy: autonomy as AutonomyLevel,
        fs_write: fsWrite
          .split('\n')
          .map((l) => l.trim())
          .filter(Boolean),
        retain_days: Number.parseInt(String(retain), 10) || null,
      },
    };
    // Rodzic czyta własny stan w `onchange` — bez śledzenia go w tym efekcie.
    untrack(() => onchange(next));
  });

  function emptyVoice() {
    return { base: 'pl-f1', pitch: 1, rate: 1, perceived_age: 22, timbre: '', design_prompt: '' };
  }

  function toggle(group: string, on: boolean) {
    tools = on ? [...new Set([...tools, group])] : tools.filter((g) => g !== group);
  }
</script>

<div class="wk-grid">
  <TextField label={t('builder.name')} hint={t('builder.nameHint')} bind:value={name} />
  <TextField label={t('builder.character')} bind:value={character} />
  <label class="wk-field">
    <span>{t('builder.color')}</span>
    <Select
      bind:value={color}
      label={t('builder.color')}
      options={[
        { value: '', label: t('builder.colorAuto') },
        ...policy.palette.map((c, i) => ({ value: c, label: t('builder.colorN', { n: i + 1 }) })),
      ]}
    />
  </label>
  <label class="wk-field">
    <span>{t('builder.voice')}</span>
    <Select
      bind:value={voiceBase}
      label={t('builder.voice')}
      options={policy.voices.map((v) => ({ value: v, label: v }))}
    />
  </label>
  <label class="wk-field">
    <span>{t('builder.pitch', { value: pitch.toFixed(2) })}</span>
    <input type="range" min="0.8" max="1.2" step="0.01" bind:value={pitch} />
  </label>
  <label class="wk-field">
    <span>{t('builder.rate', { value: rate.toFixed(2) })}</span>
    <input type="range" min="0.8" max="1.25" step="0.01" bind:value={rate} />
  </label>
</div>

<h4>{t('builder.role')}</h4>
<div class="wk-grid">
  <TextField label={t('builder.roleName')} bind:value={roleName} />
  <label class="wk-field">
    <span>{t('builder.modelPolicy')}</span>
    <Select
      bind:value={modelPolicy}
      label={t('builder.modelPolicy')}
      options={policy.model_policies.map((p) => ({ value: p, label: tk(`builder.policy.${p}`) }))}
    />
  </label>
</div>
<label class="wk-field">
  <span>{t('builder.prompt')}</span>
  <textarea rows="3" bind:value={prompt}></textarea>
</label>
<fieldset class="wk-field">
  <legend>{t('builder.tools')}</legend>
  <div class="wk-grid">
    {#each policy.groups as g (g)}
      <Checkbox
        label={tk(`builder.group.${g}`)}
        checked={tools.includes(g)}
        onchange={(on) => toggle(g, on)}
      />
    {/each}
  </div>
  <Checkbox label={t('builder.readOnly')} bind:checked={readOnly} />
</fieldset>

<h4>{t('builder.limits')}</h4>
<div class="wk-grid">
  <label class="wk-field">
    <span>{t('builder.autonomy', { ceiling: policy.ceiling })}</span>
    <Select
      bind:value={autonomy}
      label={t('builder.autonomy', { ceiling: policy.ceiling })}
      options={levels.map((l) => ({ value: l, label: `${l} · ${tk(`perm.${l}.name`)}` }))}
    />
  </label>
  <TextField label={t('builder.retain')} type="number" min="1" max="365" bind:value={retain} />
</div>
<label class="wk-field">
  <span>{t('builder.fsWrite')}</span>
  <textarea rows="2" spellcheck="false" bind:value={fsWrite}></textarea>
  <span class="wk-meta">{t('builder.fsWriteHint')}</span>
</label>
