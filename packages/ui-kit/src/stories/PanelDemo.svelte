<script lang="ts">
  import Panel from '../components/Panel.svelte';
  let tab = $state('agents');
  const tabs = [
    { id: 'agents', label: 'Agentki' },
    { id: 'timeline', label: 'Oś czasu' },
    { id: 'files', label: 'Pliki' },
    { id: 'memory', label: 'Pamięć' },
  ];
</script>

<div class="frame">
  <Panel title="Panel prawy" {tabs} bind:activeTab={tab} onclose={() => {}}>
    <p>Aktywna karta: <strong>{tab}</strong>. Strzałki ←/→ przełączają karty.</p>
  </Panel>
</div>

<style>
  .frame {
    width: 340px;
    height: 360px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    overflow: hidden;
  }
  p {
    font-size: var(--alfa-font-size-sm);
    color: var(--alfa-color-text-muted);
  }
</style>
