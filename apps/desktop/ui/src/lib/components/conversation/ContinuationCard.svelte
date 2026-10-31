<script lang="ts">
  import {Button,ConfirmDialog} from '@alfa/ui-kit';
  import {useApp} from '../../state/context';
  let {oncontinue}:{oncontinue:()=>void}=$props();
  const app=useApp();const en=$derived(app.i18n.locale==='en');let confirm=$state(false);
</script>
<section class="saved" role="status">
  <strong>{en?'Progress saved locally':'Postęp zapisany lokalnie'}</strong>
  <p>{en?'The task reached its allowance. Do you approve another allowance using the saved state? Completed operations will not be restarted.':'Zadanie osiągnęło limit. Czy zgadzasz się na kolejną pulę pracy od zapisanego miejsca? Ukończone operacje pozostają zachowane.'}</p>
  <div class="actions">
    <Button variant="primary" onclick={()=>confirm=true}>{en?'Extend and resume':'Przedłuż i wznów'}</Button>
    <Button variant="ghost" onclick={()=>app.openSettings('agents')}>{en?'Continuation settings':'Ustawienia przedłużania'}</Button>
  </div>
</section>
<ConfirmDialog bind:open={confirm} title={en?'Approve continuation?':'Zgoda na dalszą pracę?'} description={en?'Continue from the local checkpoint with another configured allowance. Model and tool usage may generate additional cost. Sędzia Cieni approvals are still required.':'Wznowię zapisane zadanie z kolejną pulą według ustawień. Dalsze wywołania modeli i narzędzi mogą zwiększyć koszt. Zgody Sędziego Cieni nadal obowiązują.'} confirmLabel={en?'Approve and resume':'Zgadzam się — wznów'} cancelLabel={en?'Keep paused':'Pozostaw wstrzymane'} onconfirm={oncontinue}/>
<style>
 .saved{display:grid;gap:var(--alfa-space-2);padding:var(--alfa-space-3);border:1px solid var(--alfa-color-border);border-radius:var(--alfa-radius-card);background:var(--alfa-color-surface);}
 p{color:var(--alfa-color-text-muted);font-size:var(--alfa-font-size-sm);}.actions{display:flex;gap:var(--alfa-space-2);flex-wrap:wrap;}
</style>
