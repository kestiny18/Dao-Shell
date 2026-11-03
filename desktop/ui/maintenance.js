/* Local confirmations and native details stay in volatile session state. */
function applyMaintenanceResult(value,request) {
  const st=state(request.sessionId);
  if(["permission_required","permission_denied"].includes(value.error)){applyObservation(value,request);return;}
  if(value.source_ref) st.maintenanceSource=value.source_ref;
  request.seenMaintenance ||= new Set();
  const key=JSON.stringify(value);if(request.seenMaintenance.has(key))return;
  request.seenMaintenance.add(key);st.maintenanceResult=value;
  if(value.operation_id) message(`${value.operation || ''} · ${t(`maintenance.${['verified','failed','cancelled_before_dispatch','awaiting_user','awaiting_reboot'].includes(value.lifecycle)?value.lifecycle:'unknown'}`)}`,'receipt',value.lifecycle==='unknown',request.sessionId);
}
function maintenanceCall(tool,args) {return run('maintenance',JSON.stringify({tool,arguments:args}));}
async function answerMaintenance(approved) {
  const id=workspace.active,c=state(id).maintenanceConfirmation;
  if(!c||!activeRequest||activeRequest.sessionId!==id||activeRequest.id!==c.executionId||activeRequest.stopping||c.answering)return;
  if(approved && c.plan.package?.agreements?.length && !c.accepted)return;
  c.answering=true;renderCurrent();
  try {await api.invoke('confirm_maintenance',{sessionId:id,executionId:c.executionId,requestId:c.request_id,approved,agreementDigest:approved?c.plan.package?.agreement_digest || null:null});}
  catch(error){message(errorText(error),'assistant',true,id);}
  finally{state(id).maintenanceConfirmation=null;renderCurrent();}
}
function renderMaintenance(st) {
  const root=$('maintenance-panel');root.replaceChildren();if(!st)return;
  const controls=document.createElement('details');controls.className='card';
  const heading=document.createElement('summary');heading.textContent=t('maintenance.title');controls.append(heading);
  const consent=document.createElement('label'),check=document.createElement('input');check.type='checkbox';check.checked=Boolean(st.maintenanceConsent);check.onchange=()=>{st.maintenanceConsent=check.checked;authorize.disabled=busy||!check.checked;};
  consent.append(check,document.createTextNode(t('maintenance.source')));
  const authorize=button(t('maintenance.authorize'),t('maintenance.authorize'),()=>run('maintenance-local',JSON.stringify({kind:'source_authorize'})));authorize.disabled=busy||!check.checked;
  const query=document.createElement('input');query.maxLength=256;query.value=st.maintenanceQuery || '';query.setAttribute('aria-label',t('maintenance.search'));query.oninput=()=>{st.maintenanceQuery=query.value;};
  const find=button(t('maintenance.search'),t('maintenance.search'),()=>maintenanceCall('software_search',{query:query.value.trim(),source_ref:st.maintenanceSource}));find.disabled=busy||!st.maintenanceSource;
  controls.append(consent,authorize,query,find);
  for(const [key,action] of [['startup',()=>run('maintenance-local',JSON.stringify({kind:'startup_targets'}))],['history',()=>run('maintenance-local',JSON.stringify({kind:'history'}))],['dns',()=>maintenanceCall('network_maintenance',{kind:'flush_dns_cache'})]]){
    const b=button(t(`maintenance.${key}`),t(`maintenance.${key}`),action);b.disabled=busy;controls.append(b);
  }
  root.append(controls);
  const value=st.maintenanceResult;
  if(value){
    const result=document.createElement('section');result.className='card';
    if(value.error){const note=document.createElement('p');note.textContent=errorText(value.error);result.append(note);}
    for(const row of value.startup_checks || []){
      const note=document.createElement('p');note.textContent=t(`maintenance.startup_check_${['retained','original'].includes(row.check?.placement)?row.check.placement:'conflict_or_unknown'}`);result.append(note);
    }
    for(const row of value.items || []){
      const item=document.createElement('div');item.className='file-row';
      const text=document.createElement('div');text.className='file-detail';text.textContent=[row.name,row.package_id,row.version,row.installed_version?`${t("maintenance.installed_version")}: ${row.installed_version}`:null,row.scope,row.source,row.path].filter(Boolean).join('\n');item.append(text);
      const actions=row.startup_ref?[row.mode]:(row.supported_actions || []);
      for(const action of actions){if(!['install','upgrade','uninstall','disable','restore'].includes(action))continue;
        const b=button(t(`maintenance.${action}`),t(`maintenance.${action}`),()=>row.startup_ref?maintenanceCall('startup_change',{startup_ref:row.startup_ref,mode:action}):maintenanceCall(`software_${action}`,action==='install'?{package_ref:row.package_ref,version_ref:row.version_ref,scope:row.scope}:action==='upgrade'?{installed_ref:row.installed_ref,version_ref:row.version_ref}:{installed_ref:row.installed_ref}));b.disabled=busy;item.append(b);
      }result.append(item);
    }
    if(value.items?.length===0){const note=document.createElement('p');note.textContent=t('maintenance.empty');result.append(note);}
    for(const receipt of value.receipts || (value.operation_id?[value]:[])){
      const note=document.createElement('p');const status=['verified','failed','cancelled_before_dispatch','awaiting_user','awaiting_reboot','checked_unknown'].includes(receipt.lifecycle)?receipt.lifecycle:'unknown';note.textContent=`${receipt.operation || ''} · ${receipt.package?.package_id || ''} ${receipt.package?.version || ''}\n${t(`maintenance.${status}`)}${receipt.operation==='flush_dns_cache'?`\n${t('maintenance.dns_report')}`:''}${receipt.persistence_warning?`\n${t('maintenance.warning')}`:''}`;result.append(note);
      if(['unknown','dispatching','dispatched','awaiting_user','awaiting_reboot'].includes(receipt.lifecycle)) {
        const software=['install','upgrade','uninstall'].includes(receipt.operation);
        const check=button(t('maintenance.reconcile'),t('maintenance.reconcile'),()=>run('maintenance-local',JSON.stringify({kind:'reconcile',operation_id:receipt.operation_id,...(software?{source_ref:st.maintenanceSource}:{})})));
        check.disabled=busy || (software && !st.maintenanceSource);result.append(check);
      }
      if(receipt.last_check) {
        const check=receipt.last_check;
        if(['baseline_recorded_restart_required','new_boot_not_observed','new_boot_observed','boot_evidence_unavailable'].includes(check.boot_status)){const line=document.createElement('p');line.textContent=t(`maintenance.${check.boot_status}`);result.append(line);}
        if(check.observed_state){const observed=check.observed_state,line=document.createElement('p');line.textContent=observed.state==='present'?t('maintenance.observed_present',{version:observed.identity?.version || '?',scope:observed.identity?.scope || '?'}):observed.state==='absent_verified'?t('maintenance.observed_absent',{scope:observed.scope}):t('maintenance.observed_unknown');result.append(line);}
        if(check.provider_idle===false){const line=document.createElement('p');line.textContent=t('maintenance.provider_busy_or_unknown');result.append(line);}
      }
      if(receipt.startup_check){const checked=document.createElement('p');checked.textContent=t(`maintenance.startup_check_${['retained','original'].includes(receipt.startup_check.placement)?receipt.startup_check.placement:'conflict_or_unknown'}`);result.append(checked);}
    }
    if(value.status==='cancelled_before_dispatch'){const note=document.createElement('p');note.textContent=t('maintenance.cancelled_before_dispatch');result.append(note);}
    root.append(result);
  }
  const c=st.maintenanceConfirmation;if(!c)return;
  const plan=c.plan,card=document.createElement('section');card.className='confirmation application-action-preview';
  const title=document.createElement('h2');title.textContent=t('maintenance.title');
  const detail=document.createElement('p');detail.textContent=[plan.operation,plan.package?.name,plan.package?.identity?.source_id,plan.package?.identity?.package_id,plan.package?.identity?.version,plan.package?.identity?.scope,plan.package?.identity?.architecture,plan.package?.installation?.identity?.version?`${t("maintenance.installed_version")}: ${plan.package.installation.identity.version}`:null,plan.original,plan.target].filter(Boolean).join('\n');
  const risk=document.createElement('p');risk.className='risk';risk.textContent=t(plan.operation==='flush_dns_cache'?'maintenance.dns_risk':plan.operation?.startsWith('startup_')?'maintenance.startup_risk':'maintenance.risk');
  const expiry=document.createElement('p');expiry.textContent=t('maintenance.pending');
  card.append(title,detail,risk,expiry);
  const agreements=plan.package?.agreements || [];
  for(const agreement of agreements){const text=document.createElement('pre');text.textContent=typeof agreement==='string'?agreement:JSON.stringify(agreement);card.append(text);}
  const yes=button(t('maintenance.approve'),t('maintenance.approve'),()=>answerMaintenance(true)),no=button(t('maintenance.cancel'),t('maintenance.cancel'),()=>answerMaintenance(false));
  yes.disabled=Boolean(c.answering)||Boolean(agreements.length&&!c.accepted);no.disabled=Boolean(c.answering);
  if(agreements.length){const label=document.createElement('label'),accept=document.createElement('input');accept.type='checkbox';accept.checked=Boolean(c.accepted);accept.onchange=()=>{c.accepted=accept.checked;yes.disabled=Boolean(c.answering)||!accept.checked;};label.append(accept,document.createTextNode(t('maintenance.terms')));card.append(label);}
  card.append(no,yes);root.append(card);
}
