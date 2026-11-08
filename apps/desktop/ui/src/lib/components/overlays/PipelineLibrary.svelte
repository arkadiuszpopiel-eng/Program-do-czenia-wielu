<script lang="ts">
  import { Button } from '@alfa/ui-kit';
  import type { PipelineDef } from '../../api/types';
  import { chainLabel } from '../../logic/pipelines';
  interface Props {
    library: readonly PipelineDef[];
    draft: PipelineDef;
    original: string | null;
    en: boolean;
    oncreate: () => void;
    onpick: (def: PipelineDef) => void;
  }
  let { library, draft, original, en, oncreate, onpick }: Props = $props();
</script>

<nav class="side" aria-label={en ? 'Pipeline library' : 'Biblioteka pipeline’ów'}>
  <Button size="sm" variant="primary" data-testid="pipeline-new" onclick={oncreate}>
    {en ? '+ New pipeline' : '+ Nowy pipeline'}
  </Button>
  <ul class="side-list">
    {#each library as def (def.name)}
      <li>
        <button
          type="button"
          class="item"
          class:active={original === def.name ||
            (def.builtin && draft.builtin && draft.name === def.name)}
          data-testid="pipeline-item"
          onclick={() => onpick(def)}
        >
          <div class="item-top">
            <strong>{def.name}</strong>
            <span class="badge" class:custom={!def.builtin}>
              {def.builtin ? (en ? 'template' : 'wzorzec') : en ? 'custom' : 'własny'}
            </span>
          </div>
          <small>{chainLabel(def, en)}</small>
        </button>
      </li>
    {/each}
  </ul>
</nav>
