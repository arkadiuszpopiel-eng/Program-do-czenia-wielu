<script lang="ts">
  import { CommandPalette, ConversationScreen, mock, type MicState } from '@alfa/ui-kit';

  // F0: makieta rozmowy z atrapami danych. Docelowo dane płyną z rdzenia przez IPC Tauri (zdarzenia batchowane per klatka).
  let micState = $state<MicState>('listening');
  let paletteOpen = $state(false);
  let leftOpen = $state(true);
  let rightOpen = $state(true);
  let theme = $state<'auto' | 'light' | 'dark'>('auto');

  $effect(() => {
    const root = document.documentElement;
    if (theme === 'auto') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', theme);
  });

  const commands = mock.mockCommands.map((c) => ({
    ...c,
    onSelect: () => {
      if (c.id === 'theme') theme = theme === 'dark' ? 'light' : 'dark';
      if (c.id === 'left') leftOpen = !leftOpen;
      if (c.id === 'right') rightOpen = !rightOpen;
      if (c.id === 'mic') micState = micState === 'off' ? 'listening' : 'off';
    },
  }));

  function onWindowKeydown(event: KeyboardEvent) {
    // §14.8: przeładowanie WebView wyłączone.
    if (
      event.key === 'F5' ||
      ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'r')
    ) {
      event.preventDefault();
      return;
    }
    if (event.ctrlKey && event.key.toLowerCase() === 'b') {
      event.preventDefault();
      leftOpen = !leftOpen;
    }
    if (event.ctrlKey && event.key === '\\') {
      event.preventDefault();
      rightOpen = !rightOpen;
    }
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<ConversationScreen
  project="Projekt X"
  session="Raport Q3"
  sessions={mock.mockSessions}
  messages={mock.mockMessages}
  activity={{
    agent: 'delta',
    description: 'edytuję raport.docx',
    step: 3,
    totalSteps: 7,
    elapsedSeconds: 42,
  }}
  {micState}
  bind:leftOpen
  bind:rightOpen
  toast={{ message: 'Delta: przeniesiono 14 plików · 2,1 s', actionLabel: 'Cofnij' }}
  onsubmit={() => {}}
  onstop={() => {}}
  onsettings={() => (paletteOpen = true)}
/>

<CommandPalette bind:open={paletteOpen} items={commands} />
