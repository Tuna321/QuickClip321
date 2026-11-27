const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync('v3/ui/app.js','utf8');
const code = source.slice(source.indexOf('let signedAvailable=false'),source.indexOf('$("exportPrefs").onclick='));
const elements = new Map();
const $ = id => { if(!elements.has(id))elements.set(id,{value:'',hidden:false,disabled:false,textContent:''});return elements.get(id); };
const pending=[], actions=[];
let ready=null, failInstall=false, stageVersion="3.0.0";
const invoke=async (name,args)=>{
 if(name==='check_app_updates')return new Promise((resolve,reject)=>pending.push({args,resolve,reject}));
 if(name==='update_status')return {ready,busy:false,message:'Ready'};
 if(name==='stage_signed_update'){actions.push([name,args.channel]);ready={channel:args.channel,version:stageVersion};return ready;}
 if(name==='install_signed_update'){actions.push([name]);if(failInstall)throw Error('Queue not empty');}
 if(name==='snapshot')return {settings:{prefs:{update_channel:'stable'}}};
};
const context=vm.createContext({$,t:s=>s,invoke,saveSection:async()=>{},confirm:()=>true});
vm.runInContext(code,context);
const info=(channel,signed=true)=>({installed:'3.1.0-nightly.2',state:'Release available',latest_in_channel:channel==='stable'?'3.0.0':'3.1.0-nightly.3',signed_available:signed,switch_available:channel==='stable'});
(async()=>{
 $('updateChannel').value='stable';const first=$('checkUpdates').onclick();
 $('updateChannel').value='nightly';const second=$('checkUpdates').onclick();
 pending[1].resolve(info('nightly'));await second;
 pending[0].resolve(info('stable'));await first;
 assert.equal($('stageSigned').textContent,'Download Nightly');
 assert.equal($('stageSigned').hidden,false);
 const third=$('checkUpdates').onclick();pending[2].reject(Error('Offline'));await third;
 assert.equal($('stageSigned').hidden,true);
 $('updateChannel').value='stable';const fourth=$('checkUpdates').onclick();pending[3].resolve(info('stable'));await fourth;
 failInstall=true;await $('stageSigned').onclick();
 assert.deepEqual(actions,[['stage_signed_update','stable'],['install_signed_update']]);
 assert.equal($('installSigned').hidden,false);
 assert.match($('updateResult').textContent,/Queue not empty/);
 $('updateChannel').value='nightly';const fifth=$('checkUpdates').onclick();pending[4].resolve(info('nightly',false));await fifth;
 assert.equal($('installSigned').hidden,true);assert.equal($('stageSigned').hidden,true);
 $('updateChannel').value='stable';const sixth=$('checkUpdates').onclick();pending[5].resolve({...info('stable'),latest_in_channel:'3.0.1',switch_available:false});await sixth;
 assert.equal($('stageSigned').hidden,false);assert.equal($('installSigned').hidden,true);
 stageVersion='3.0.1';await $('stageSigned').onclick();assert.equal(ready.version,'3.0.1');assert.equal($('installSigned').hidden,false);assert.equal($('stageSigned').hidden,true);
 console.log('PASS: newer same-channel release overrides old staged version');
 console.log('PASS: stale responses, offline reset, explicit switch, busy queue recovery, wrong-channel staged release');
})().catch(e=>{console.error(e);process.exitCode=1;});

