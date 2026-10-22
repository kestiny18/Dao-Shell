/* Native targets and confirmation tokens live only in their originating request/session. */
function actionReceiptText(value) {
  const lines=[`${value.name || ''} · ${localDate(value.observed_at || value.created_at)}`];
  const lifecycle=['completed','rejected','unknown','failed_before_dispatch','expired'].includes(value.lifecycle)?value.lifecycle:'unknown';
  lines.push(t(`actions.${lifecycle}`));
  const evidence=value.evidence || {};
  if(evidence.process_created) lines.push(t('actions.created'));
  if(evidence.process_exited) lines.push(t('actions.exited'));
  if(evidence.process_state==='still_running') lines.push(t('actions.running'));
  if(evidence.process_state==='unknown') lines.push(t('actions.process_unknown'));
  if(evidence.reconciliation) lines.push(t(evidence.reconciliation.process_state==='still_running'?'actions.running':'actions.process_unknown'));
  if(value.checked_at) lines.push(localDate(value.checked_at));
  if(value.persistence_warning) lines.push(t('actions.warning'));
  return lines.join('\n');
}
function applyApplicationResult(capability,value,request) {
  if(!session(request.sessionId)) return;
  const st=state(request.sessionId);
  if(capability==='app_targets') {st.actionTargets=value;return;}
  if(capability==='app_action_history') {if(value.receipts) st.actionReceipts=value.receipts;else if(value.cleared!==undefined) st.actionReceipts=(st.actionReceipts || []).filter(v=>v.lifecycle==='unknown');return;}
  if(!value.operation_id) return;
  request.seenActions ||= new Set();
  const key=`${value.operation_id}:${value.observed_at}:${value.checked_at || ''}`;
  if(request.seenActions.has(key)) return;request.seenActions.add(key);
  st.actionReceipts ||= [];const index=st.actionReceipts.findIndex(r=>r.operation_id===value.operation_id);
  if(index>=0) st.actionReceipts.splice(index,1);st.actionReceipts.unshift(value);st.actionReceipts=st.actionReceipts.slice(0,50);
  message(actionReceiptText(value),'receipt',value.lifecycle==='unknown',request.sessionId);
}
async function answerApplicationAction(approved) {
  const id=workspace.active,c=state(id).appConfirmation;
  if(!c||!activeRequest||activeRequest.sessionId!==id||activeRequest.id!==c.executionId||activeRequest.stopping||c.answering) return;
  c.answering=true;renderCurrent();
  try {await api.invoke('confirm_open',{sessionId:id,executionId:c.executionId,requestId:c.request_id,approved});}
  catch(error) {message(errorText(error),'assistant',true,id);}
  finally {state(id).appConfirmation=null;renderCurrent();}
}
function renderApplicationActions(st) {
  const target=$('application-action-panel');target.replaceChildren();if(!st)return;
  const selection=st.actionTargets;
  if(selection) {
    const card=document.createElement('details');card.className='card';card.open=true;
    const summary=document.createElement('summary');summary.textContent=t('actions.find');card.append(summary);
    if(selection.reason==='close_identity_not_atomic') {const note=document.createElement('p');note.textContent=t('actions.unavailable');card.append(note);}
    else {
      const rows=selection.local_items || selection.items || [];
      if(!rows.length) {const note=document.createElement('p');note.textContent=t('actions.no_targets');card.append(note);}
      for(const row of rows) {
        const item=document.createElement('div');item.className='file-row';
        const text=document.createElement('div');text.className='file-detail';text.textContent=`${row.index || ''} ${row.name || ''}\n${row.path || ''}${row.pid?`\nPID ${row.pid} · ${row.created || ''}`:''}${row.entry_path?`\n${row.entry_path}`:''}${row.supported===false?`\n${t('actions.target_unavailable')}`:''}`;
        const action=button(t('actions.select'),t('actions.select'),()=>run(row.kind==='launch'?'app-launch':row.kind==='close'?'app-close':'process-terminate',JSON.stringify({target_ref:row.target_ref})));
        action.disabled=busy||row.supported!==true;item.append(text,action);card.append(item);
      }
      if(selection.page?.next_cursor && selection.query) {const next=button(t('computer.next'),t('computer.next'),()=>run('app-targets',JSON.stringify({...selection.query,cursor:selection.page.next_cursor})));next.disabled=busy;card.append(next);}
    }
    target.append(card);
  }
  const confirmation=st.appConfirmation;
  if(confirmation) {
    const plan=confirmation.plan,card=document.createElement('section');card.className='confirmation application-action-preview';
    const title=document.createElement('h2');title.textContent=`${plan.kind==='terminate'?t('actions.confirm_terminate'):t('actions.confirm_launch')} · ${plan.name}`;
    const detail=document.createElement('p');detail.textContent=`${t('actions.path')}: ${plan.path || ''}${plan.entry_path?`\n${t('actions.entry')}: ${plan.entry_path}`:''}${plan.working_directory?`\n${t('actions.cwd')}: ${plan.working_directory}`:''}${plan.pid?`\nPID ${plan.pid}\n${t('actions.time')}: ${plan.created}`:''}`;
    const risk=document.createElement('p');risk.className='risk';risk.textContent=t(plan.kind==='terminate'?'actions.risk_terminate':'actions.risk_launch');
    const note=document.createElement('p');note.textContent=t('actions.pending');
    const yes=button(t(plan.kind==='terminate'?'actions.confirm_terminate':'actions.confirm_launch'),t('actions.pending'),()=>answerApplicationAction(true));
    const no=button(t('actions.cancel'),t('actions.cancel'),()=>answerApplicationAction(false));yes.disabled=no.disabled=Boolean(confirmation.answering);
    card.append(title,detail,risk,note,no,yes);target.append(card);
  }
  for(const receipt of st.actionReceipts || []) {
    const card=document.createElement('details');card.className='card';
    const title=document.createElement('summary');title.textContent=`${receipt.name || ''} · ${localDate(receipt.observed_at || receipt.created_at)}`;
    const body=document.createElement('div');body.className='application-action-preview';body.textContent=actionReceiptText(receipt);
    const check=button(t('actions.reconcile'),t('actions.reconcile'),()=>run('reconcile-action',receipt.operation_id));check.disabled=busy;
    card.append(title,body,check);target.append(card);
  }
}
$('application-find-targets').addEventListener('click',()=>run('app-targets',JSON.stringify({kind:$('application-action-kind').value,query:$('application-action-query').value.trim()})));
$('application-history').addEventListener('click',()=>run('action-receipts'));
$('application-clear-history').addEventListener('click',()=>run('clear-completed-actions'));
