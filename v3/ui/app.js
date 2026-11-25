"use strict";
const $ = id => document.getElementById(id);
const invoke = (name,args={}) => window.__TAURI__.core.invoke(name,args);
let current = "home", initialized = false, busy = false, timer;
function notice(text,error=false){const t=$("toast");t.textContent=String(text);t.classList.toggle("error",error);clearTimeout(timer);timer=setTimeout(()=>t.textContent="",7000);}
function show(page){current=page;document.querySelectorAll(".page").forEach(p=>p.hidden=p.id!==page);document.querySelectorAll(".nav").forEach(b=>{const active=b.dataset.page===page;b.classList.toggle("active",active);b.setAttribute("aria-current",active?"page":"false")});$("title").textContent={home:"New download",queue:"Queue",history:"History",settings:"Settings"}[page];refresh();}
document.querySelectorAll(".nav").forEach(b=>b.addEventListener("click",()=>show(b.dataset.page)));
function el(tag,cls,text){const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;}
function action(label,cmd,id){const b=el("button","small",label);b.addEventListener("click",async()=>{b.disabled=true;try{await invoke(cmd,{id});notice(label==="Retry"?"Added to queue":"Done");await refresh();}catch(e){notice(e,true);}finally{b.disabled=false;}});return b;}
function render(container,jobs,empty){const signature=JSON.stringify(jobs);if(container.dataset.signature===signature||container.contains(document.activeElement))return;container.dataset.signature=signature;const frag=document.createDocumentFragment();if(!jobs.length)frag.append(el("div","empty",empty));for(const j of [...jobs].reverse()){const card=el("article","job");const top=el("div","job-top");const info=el("div");let name=j.files.length?j.files[0].split(/[\\/]/).pop():j.url;info.append(el("div","job-title",name),el("div","job-meta",j.preset+" · "+j.quality+" · "+j.message));top.append(info,el("span","job-status",j.status));card.append(top);if(j.status==="running"||j.status==="queued"){const track=el("progress");track.max=100;track.value=Math.round(j.progress*100);track.setAttribute("aria-label","Download progress");card.append(track);}const buttons=el("div","job-actions");if(["running","queued"].includes(j.status))buttons.append(action("Cancel","cancel",j.id));else{if(j.files.length)buttons.append(action("Open folder","open_folder",j.id));if(["failed","cancelled"].includes(j.status))buttons.append(action("Retry single item","retry",j.id));buttons.append(action("Remove history","remove",j.id));}card.append(buttons);frag.append(card);}container.replaceChildren(frag);}
async function refresh(){if(busy)return;busy=true;try{const s=await invoke("snapshot");if(!initialized){$("destination").value=s.settings.destination;$("defaultDest").value=s.settings.destination;$("ytdlp").value=s.settings.yt_dlp;$("ffmpeg").value=s.settings.ffmpeg;applyPreferences(s.settings.prefs);initialized=true;}const active=s.jobs.filter(j=>["running","queued"].includes(j.status));const latest=active.find(j=>j.status==="running")||active[0]||s.jobs[s.jobs.length-1];$("quickStatus").textContent=latest?latest.message:"Ready";$("quickProgress").value=latest?Math.round(latest.progress*100):0;$("badge").textContent=active.length;$("setupHint").hidden=Boolean(s.settings.yt_dlp&&s.settings.ffmpeg);if(current==="queue")render($("queueList"),active,"No active downloads.");if(current==="history")render($("historyList"),s.jobs.filter(j=>!["running","queued"].includes(j.status)),"No download history yet.");}catch(e){notice(e,true);}finally{busy=false;}}
let dialogOpen=false;async function pick(target,directory){dialogOpen=true;try{const path=await invoke("plugin:dialog|open",{options:{directory,multiple:false,...(!directory?{filters:[{name:"Windows executables",extensions:["exe"]}]}:{})}});if(typeof path==="string")$(target).value=path;}catch(e){notice(e,true);}finally{dialogOpen=false;}}
$("browse").onclick=async()=>{const before=$("destination").value;await pick("destination",true);if(before!==$("destination").value)$("organize").checked=false;};$("destination").addEventListener("input",()=>$("organize").checked=false);$("autoFolder").onclick=()=>{$("destination").value=$("defaultDest").value;$("organize").checked=true;};$("defaultBrowse").onclick=()=>pick("defaultDest",true);document.querySelectorAll(".pick").forEach(b=>b.onclick=()=>pick(b.dataset.target,false));
$("paste").onclick=async()=>{try{const t=await invoke("plugin:clipboard-manager|read_text");if(typeof t==="string"){$("url").value=t.trim();autoFilled="";$("url").dispatchEvent(new Event("input"));}}catch(e){notice(e,true);}};
$("form").onsubmit=async e=>{e.preventDefault();const b=e.submitter;b.disabled=true;try{await invoke("enqueue",{request:{url:$("url").value.trim(),preset:$("format").value,quality:$("quality").value,playlist:$("playlist").checked,organize:$("organize").checked,options:mediaOptions(),destination:$("destination").value}});$("url").value="";autoFilled="";$("url").dispatchEvent(new Event("input"));$("playlist").checked=false;notice("Added to queue.");if(expanded)show("queue");else await refresh();}catch(err){notice(err,true);}finally{b.disabled=false;}};
$("settingsForm").onsubmit=async e=>{e.preventDefault();try{await invoke("save_settings",{settings:{destination:$("defaultDest").value,yt_dlp:$("ytdlp").value,ffmpeg:$("ffmpeg").value,prefs:readPreferences()}});applyTheme();$("organize").checked=$("smartOrganizer").checked;$("destination").value=$("defaultDest").value;notice("Settings saved.");await refresh();}catch(err){notice(err,true);}};
function mediaOptions(){
    return {clip_start:$("clipStart").value===""?null:Number($("clipStart").value),clip_end:$("clipEnd").value===""?null:Number($("clipEnd").value),subtitle_languages:$("subtitleLanguages").value.split(",").map(s=>s.trim()).filter(Boolean),subtitle_format:$("subtitleFormat").value,subtitle_auto:$("subtitleAuto").checked,archive:$("preset").value==="archive"};
}
const audioFormats=["mp3","m4a","wav"];
function updateFormat(){ $("quality").disabled=audioFormats.includes($("format").value); }
$("preset").onchange=()=>{let p=$("preset").value;if(p!=="custom"){$("format").value=p==="archive"?"original":p;$("quality").value=p==="editor"?"1080":"best";updateFormat();}};
$("format").onchange=()=>{$("preset").value="custom";updateFormat();};
$("quality").onchange=()=>{$("preset").value="custom";};
function applyTheme(){const theme=$("theme").value;document.documentElement.dataset.theme=theme==="System"?(matchMedia("(prefers-color-scheme: light)").matches?"Light":"Dark"):theme;document.documentElement.dataset.accent=$("accent").value;}
function readPreferences(){return {popup_mode:$("popupMode").value,popup_x:Number($("popupX").value)/100,popup_y:Number($("popupY").value)/100,startup_view:$("startupView").value,theme:$("theme").value,accent:$("accent").value,clipboard_monitor:$("clipboardMonitor").checked,smart_organizer:$("smartOrganizer").checked,default_preset:$("preset").value,default_format:$("format").value,default_quality:$("quality").value,fragment_concurrency:Number($("fragments").value)};}
function applyPreferences(p){if(!p)return;const entries={popupMode:p.popup_mode,popupX:Math.round(p.popup_x*100),popupY:Math.round(p.popup_y*100),startupView:p.startup_view,theme:p.theme,accent:p.accent,preset:p.default_preset,format:p.default_format,quality:p.default_quality,fragments:String(p.fragment_concurrency)};for(const [k,v] of Object.entries(entries))$(k).value=v;$("clipboardMonitor").checked=p.clipboard_monitor;$("smartOrganizer").checked=p.smart_organizer;$("organize").checked=p.smart_organizer;updateFormat();applyTheme();if(p.startup_view==="advanced"||(p.startup_view==="remember"&&localStorage.getItem("expanded")==="true"))toggleExpanded(true).catch(e=>notice(e,true));}
$("theme").onchange=applyTheme;$("accent").onchange=applyTheme;$("clipboardMonitor").onchange=()=>{if(!$("clipboardMonitor").checked){pendingCopied="";$("pendingLink").hidden=true;}};
matchMedia("(prefers-color-scheme: light)").addEventListener("change",applyTheme);
let autoFilled="",pendingCopied="",clipboardBusy=false,clipboardAutoReady=false;
function acceptCopied(value){
    if(!value)return;
    const input=$("url");
    if(!input.value.trim()||input.value===autoFilled){input.value=value;autoFilled=value;pendingCopied="";$("pendingLink").hidden=true;input.dispatchEvent(new Event("input"));}
    else if(input.value!==value){pendingCopied=value;$("pendingLink").hidden=false;}
}
async function checkClipboard(){
    if(!clipboardAutoReady||clipboardBusy||!initialized||!$("clipboardMonitor").checked||!document.hasFocus()||dialogOpen)return;
    clipboardBusy=true;
    try{const value=await invoke("take_clipboard_link");if(value)acceptCopied(value);}catch(_){}finally{clipboardBusy=false;}
}
$("useCopied").onclick=()=>{if(pendingCopied){$("url").value="";acceptCopied(pendingCopied);}};
$("clearLink").onclick=()=>{$("url").value="";autoFilled="";$("url").dispatchEvent(new Event("input"));$("url").focus();};
$("quickQueue").onclick=async()=>{try{if(!expanded)await toggleExpanded(true);show("queue");}catch(e){notice(e,true);}};
window.addEventListener("focus",checkClipboard);setInterval(checkClipboard,500);
document.addEventListener("keydown",e=>{if(e.ctrlKey&&e.key.toLowerCase()==="l"){e.preventDefault();$("url").focus();$("url").select();}});
$("batchAdd").onclick=async()=>{let b=$("batchAdd");b.disabled=true;try{let urls=await invoke("validate_links",{urls:$("batchUrls").value.split(/\r?\n/).map(s=>s.trim()).filter(Boolean)});let added=0;for(let url of urls){try{await invoke("enqueue",{request:{url,preset:$("format").value,quality:$("quality").value,playlist:false,organize:$("organize").checked,options:mediaOptions(),destination:$("destination").value}});added++;}catch(e){notice(e,true);}}if(added===urls.length)$("batchUrls").value="";notice(added+" of "+urls.length+" links added.");show("queue");}catch(e){notice(e,true);}finally{b.disabled=false;}};
document.querySelectorAll(".queue-action").forEach(b=>b.onclick=async()=>{try{await invoke("queue_action",{action:b.dataset.action});await refresh();}catch(e){notice(e,true);}});
let expanded=false;
async function toggleExpanded(next){await invoke("set_mode",{expanded:next});expanded=next;localStorage.setItem("expanded",String(expanded));$("workspace").hidden=!expanded;$("expand").textContent=expanded?"Quick view":"Advanced";if(expanded)show("queue");}$("expand").onclick=()=>toggleExpanded(!expanded).catch(e=>notice(e,true));
$("hide").onclick=()=>invoke("hide_window").catch(e=>notice(e,true));
document.addEventListener("keydown",e=>{if(e.key==="Escape"&&!dialogOpen){e.preventDefault();invoke("hide_window").catch(e=>notice(e,true));}});
window.addEventListener("blur",()=>setTimeout(()=>{if(!dialogOpen&&!expanded&&!document.hasFocus())invoke("hide_window").catch(()=>{});},250));
let previewTimer,previewRunning=false,previewPending=false;
async function fetchPreview(){
    if(previewRunning){previewPending=true;return;}
    const url=$("url").value.trim();if(!/^(https?:\/\/|www\.)\S+$/i.test(url))return;
    previewRunning=true;previewPending=false;
    try{let p=await invoke("media_preview",{url});if($("url").value.trim()!==url)return;let detail=[p.channel,p.duration?Math.round(p.duration)+"s":"",p.qualities.length?p.qualities.slice(0,4).join("p / ")+"p":"",p.playlist?(p.count||"Unknown")+" items":""].filter(Boolean).join(" | ");$("previewText").textContent=p.title+(detail?" - "+detail:"");}
    catch(_){if($("url").value.trim()===url)$("previewText").textContent="Preview unavailable. You can still download.";}
    finally{previewRunning=false;if(previewPending)fetchPreview();}
}
$("url").addEventListener("input",()=>{let url=$("url").value.trim();$("playlist").checked=false;clearTimeout(previewTimer);$("previewText").textContent=url?"Loading preview...":"Paste a media link to get started.";if(url)previewTimer=setTimeout(fetchPreview,800);});
function contrast(a,b){const l=c=>{let x=c.match(/\d+(?:\.\d+)?/g).slice(0,3).map(v=>Number(v)/255).map(v=>v<=.04045?v/12.92:Math.pow((v+.055)/1.055,2.4));return .2126*x[0]+.7152*x[1]+.0722*x[2];};let x=l(a),y=l(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05);}
function themeSmoke(){const root=document.documentElement,oldTheme=$("theme").value,oldAccent=$("accent").value;const bg=new Set(),accents=new Set();let ok=true;for(let t of ["Dark","Light","Midnight","AMOLED","Forest","Sand"]){$("theme").value=t;for(let a of ["Purple","Graphite","Ocean","Emerald","Rose","Amber"]){$("accent").value=a;applyTheme();let body=getComputedStyle(document.body),button=getComputedStyle(document.querySelector(".primary")),input=getComputedStyle($("url"));bg.add(body.backgroundColor);accents.add(button.backgroundColor);ok=ok&&contrast(body.color,body.backgroundColor)>=4.5&&contrast(button.color,button.backgroundColor)>=4.5&&contrast(input.color,input.backgroundColor)>=4.5;}}$("theme").value=oldTheme;$("accent").value=oldAccent;applyTheme();return ok&&bg.size===6&&accents.size===6;}
async function smokeChecks(){
    if(!await invoke("smoke_active")){clipboardAutoReady=true;checkClipboard();return;}
    let clipboardOk=false;
    try{
        const seeded=await invoke("take_clipboard_link");
        acceptCopied(seeded);
        const first=$("url").value==="https://example.com/fixture";
        acceptCopied("https://example.com/second");
        const replaced=$("url").value==="https://example.com/second";
        $("url").value="https://example.com/manual";autoFilled="";
        acceptCopied("https://example.com/third");
        const protectedDraft=$("url").value==="https://example.com/manual"&&!$("pendingLink").hidden;
        $("useCopied").click();
        const explicit=$("url").value==="https://example.com/third"&&$("pendingLink").hidden;
        $("url").value="";autoFilled="";pendingCopied="";$("url").dispatchEvent(new Event("input"));
        clipboardOk=first&&replaced&&protectedDraft&&explicit;
    }catch(_){}
    await invoke("ui_ready",{passed:initialized&&Boolean($("form")&&$("queueList")&&$("historyList")&&$("settingsForm")),themePassed:themeSmoke(),clipboardPassed:clipboardOk});
}
refresh().then(smokeChecks).catch(e=>notice(e,true));setInterval(refresh,1000);
