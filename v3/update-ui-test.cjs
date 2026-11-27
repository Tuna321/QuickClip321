const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync('v3/ui/app.js','utf8');
const code=source.slice(source.indexOf('let updateGeneration=0'),source.indexOf('$("exportPrefs").onclick='));
const elements=new Map(),$=id=>{if(!elements.has(id))elements.set(id,{value:'',hidden:false,disabled:false,textContent:''});return elements.get(id)};
const storage=new Map(),actions=[];
const info=(channel,v=null,switching=false)=>({channel,available:switching?null:v,latest_in_channel:v||'3.2.0',signed_available:!!v,switch_available:switching});
let status={installed:'3.2.0-nightly.1',installed_channel:'nightly',releases:[info('nightly'),info('stable','3.2.0',true)],checked_at:1,busy:false,queue_busy:false,ready:null,check_error:null};
let failInstall=false,stageVersion=null,manualChecks=0, deferred=null;
const ctx=vm.createContext({$,t:s=>s,localStorage:{getItem:k=>storage.get(k),setItem:(k,v)=>storage.set(k,v)},invoke:async(name,args)=>{
 if(name==='update_status'){if(deferred){const p=deferred;deferred=null;return p;}return structuredClone(status);}
 if(name==='check_app_updates'){manualChecks++;return status.releases.find(i=>i.channel===args.channel);}
 if(name==='stage_signed_update'){actions.push([name,args.channel]);status.ready={channel:args.channel,version:stageVersion||status.releases.find(i=>i.channel===args.channel).latest_in_channel};return status.ready;}
 if(name==='install_signed_update'){actions.push([name,args.expectedVersion]);if(failInstall||status.ready.version!==args.expectedVersion)throw Error('Queue not empty or release changed');return;}
 if(name==='snapshot')return {settings:{prefs:{update_channel:'nightly'}}};
 throw Error(name);
},confirm:()=>true,saveSection:async()=>{},expanded:false,toggleExpanded:async()=>{},show:()=>{},notice:()=>{}});
vm.runInContext(code,ctx);
const refresh=()=>vm.runInContext('refreshUpdateStatus()',ctx);
(async()=>{
 $('updateChannel').value='nightly';await refresh();
 assert.equal($('installedVersion').textContent,'3.2.0-nightly.1');assert.equal($('installedChannel').textContent,'Nightly');assert.equal($('installSigned').hidden,true);assert.equal($('updateBanner').hidden,true);assert.equal($('updateResult').textContent,'You are up to date');
 status.releases[0]=info('nightly','3.2.1-nightly.1');await refresh();
 assert.equal($('updateBanner').hidden,false);assert.match($('updateBannerText').textContent,/3.2.1-nightly.1/);assert.equal($('installSigned').textContent,'Install update');
 $('dismissUpdateBanner').onclick();await refresh();assert.equal($('updateBanner').hidden,true);assert.equal($('updateBadge').hidden,false);
 status.releases[0]=info('nightly','3.2.2-nightly.1');await refresh();assert.equal($('updateBanner').hidden,false);
 await $('checkUpdates').onclick();assert.equal(manualChecks,1);
 status.queue_busy=true;await refresh();assert.equal($('installSigned').disabled,true);await $('installSigned').onclick();assert.equal(actions.length,0);
 status.queue_busy=false;status.check_error='Offline';await refresh();assert.equal($('installSigned').disabled,true);assert.match($('updateResult').textContent,/retry later/);
 status.check_error=null;await refresh();
 status.ready={channel:'nightly',version:'3.2.1-nightly.1'};await refresh();failInstall=true;await $('installSigned').onclick();
 assert.deepEqual(actions,[['stage_signed_update','nightly'],['install_signed_update','3.2.2-nightly.1']]);assert.match($('signedStatus').textContent,/Queue not empty/);
 failInstall=false;$('updateChannel').value='stable';await $('updateChannel').onchange();assert.equal($('installedChannel').textContent,'Nightly');assert.equal($('installSigned').textContent,'Switch to Stable');assert.equal(actions.length,2);
 await $('installSigned').onclick();assert.deepEqual(actions.slice(2),[['stage_signed_update','stable'],['install_signed_update','3.2.0']]);
 status.installed='3.2.0';status.installed_channel='stable';status.releases=[info('nightly','3.2.2-nightly.1',true),info('stable')];await refresh();assert.equal($('installSigned').hidden,true);assert.equal($('updateBanner').hidden,true);
 let resolveOld;deferred=new Promise(r=>resolveOld=r);const old=refresh();status.releases[1]=info('stable','3.2.1');await refresh();resolveOld({...status,releases:[info('stable')]});await old;assert.match($('releaseVersion').textContent,/3.2.1/);
 console.log('PASS installed Stable/Nightly, same-version no action, background banner, dismiss per version, manual check, offline/queue guard, explicit switch, stale staged and stale response');
})().catch(e=>{console.error(e);process.exitCode=1;});
