"use strict";
const $ = id => document.getElementById(id);
const invoke = (name,args={}) => window.__TAURI__.core.invoke(name,args);
let current = "home", initialized = false, busy = false, timer;
captureLocale();
let toolsAvailable=false,setupRunning=false;
let organizerEnabled=true,manualDestination=false,savedDefaultDestination="";
function notice(text,error=false){const toast=$("toast");toast.textContent=t(text);toast.classList.toggle("error",error);clearTimeout(timer);timer=setTimeout(()=>toast.textContent="",7000);}
function syncQueueButton(){$("quickQueue").setAttribute("aria-expanded",String(expanded&&current==="queue"));}
function show(page){current=page;syncQueueButton();document.querySelectorAll(".page").forEach(p=>p.hidden=p.id!==page);document.querySelectorAll(".nav").forEach(b=>{const active=b.dataset.page===page;b.classList.toggle("active",active);b.setAttribute("aria-current",active?"page":"false")});if(page==="diagnostics")loadDiagnostics();if(page==="shortcuts")renderShortcuts();else {if(recordingShortcut)stopRecording();if(testingShortcuts)stopShortcutTest();}$("title").textContent=t({home:"New download",queue:"Queue",history:"History",settings:"Settings",shortcuts:"Shortcuts",profiles:"Profiles",organizer:"Organizer",diagnostics:"Diagnostics"}[page]);refresh();}
document.querySelectorAll(".nav").forEach(b=>b.addEventListener("click",()=>show(b.dataset.page)));
function el(tag,cls,text){const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=tag==="button"||cls==="empty"||cls==="job-status"?t(text):text;return node;}
function action(label,cmd,id){const b=el("button","small",label);b.addEventListener("click",async()=>{b.disabled=true;try{await invoke(cmd,{id});notice(label==="Retry"?"Added to queue":"Done");await refresh();}catch(e){notice(e,true);}finally{b.disabled=false;}});return b;}
function render(container,jobs,empty){const signature=JSON.stringify(jobs);if(container.dataset.signature===signature)return;if(container.contains(document.activeElement)&&!document.activeElement.disabled){const focused=document.activeElement;focused.onblur=()=>refresh();return;}container.dataset.signature=signature;const frag=document.createDocumentFragment();if(!jobs.length)frag.append(el("div","empty",empty));for(const j of [...jobs].reverse()){const card=el("article","job");const top=el("div","job-top");const info=el("div");let name=j.files.length?j.files[0].split(/[\\/]/).pop():j.url;info.append(el("div","job-title",name),el("div","job-meta",j.preset+" · "+j.quality+" · "+j.message));top.append(info,el("span","job-status",j.status));card.append(top);if(["running","queued","paused"].includes(j.status)){const track=el("progress");track.max=100;track.value=Math.round(j.progress*100);track.setAttribute("aria-label","Download progress");card.append(track);}const buttons=el("div","job-actions");if(["running","queued","paused"].includes(j.status)){buttons.append(action("Cancel","cancel",j.id));if(j.status==="queued")buttons.append(action("Hold","pause_waiting",j.id));if(j.status==="running"&&j.preset==="original"&&!j.playlist&&!j.options?.archive&&j.options?.clip_start==null&&j.options?.clip_end==null&&!j.options?.subtitle_languages?.length&&j.message.startsWith("Downloading"))buttons.append(action("Pause","pause_waiting",j.id));if(j.status==="paused")buttons.append(action("Resume","resume_waiting",j.id));}else{if(j.files.length)buttons.append(action("Open folder","open_folder",j.id));if(["failed","cancelled"].includes(j.status))buttons.append(action("Retry single item","retry",j.id));buttons.append(action("Remove history","remove",j.id));}card.append(buttons);frag.append(card);}container.replaceChildren(frag);}
async function refresh(){if(busy)return;busy=true;try{const s=await invoke("snapshot");if(!initialized){savedDefaultDestination=s.settings.destination;$("destination").value=savedDefaultDestination;$("defaultDest").value=savedDefaultDestination;$("ytdlp").value=s.settings.yt_dlp;$("ffmpeg").value=s.settings.ffmpeg;applyPreferences(s.settings.prefs);initialized=true;}applySubmittedLinks(await invoke("take_submitted_links"));const active=s.jobs.filter(j=>["running","queued","paused"].includes(j.status));const latest=active.find(j=>j.status==="running")||active[0]||s.jobs[s.jobs.length-1];$("quickStatus").textContent=t(active.length||activePreviewId?latest?.message||"Ready":"Ready");$("quickProgress").value=active.length?Math.round(latest.progress*100):0;$("badge").textContent=active.length;toolsAvailable=await invoke("tools_ready");$("setupHint").hidden=toolsAvailable||setupRunning;if(!toolsAvailable)$("quickStatus").textContent=t(setupRunning?"Preparing media tools":"Setup required");if(current==="shortcuts")updateShortcutStatus(await invoke("hotkey_status"));if(!$("url").value.trim()&&latest&&active.length){clearTimeout(previewResetTimer);previewResetTimer=null;showActivePreview(latest);liveJob=latest.status==="running"?latest:null;}else{liveJob=null;if(!active.length&&activePreviewId&&!previewResetTimer){frameJob=null;const finished=activePreviewId;previewResetTimer=setTimeout(()=>{previewResetTimer=null;if(activePreviewId===finished&&!$("url").value.trim()){resetPreview();$("quickStatus").textContent=t("Ready");}},1500);}}if(current==="queue")render($("queueList"),active,"No active downloads.");if(current==="history")render($("historyList"),s.jobs.filter(j=>!["running","queued","paused"].includes(j.status)),"No download history yet.");}catch(e){notice(e,true);}finally{busy=false;}}
let dialogOpen=false;async function pick(target,directory){dialogOpen=true;try{const path=await invoke("plugin:dialog|open",{options:{directory,multiple:false,...(!directory?{filters:[{name:"Windows executables",extensions:["exe"]}]}:{})}});if(typeof path==="string")$(target).value=path;}catch(e){notice(e,true);}finally{dialogOpen=false;}}
$("browse").onclick=async()=>{const before=$("destination").value;await pick("destination",true);if(before!==$("destination").value){manualDestination=true;syncDestination();rememberCurrentDownload();}};function syncDestination(){$("autoFolder").hidden=!manualDestination;$("destination").title=manualDestination?"Manual destination (organizer bypassed)":organizerEnabled?"Automatic organizer enabled":"Default destination";}
$("destination").addEventListener("input",()=>{manualDestination=true;syncDestination();});$("autoFolder").onclick=()=>{$("destination").value=savedDefaultDestination;manualDestination=false;syncDestination();rememberCurrentDownload();};$("defaultBrowse").onclick=()=>pick("defaultDest",true);document.querySelectorAll(".pick").forEach(b=>b.onclick=()=>pick(b.dataset.target,false));
$("paste").onclick=async()=>{try{const t=await invoke("plugin:clipboard-manager|read_text");if(typeof t==="string"){$("url").value=t.trim();autoFilled="";$("url").dispatchEvent(new Event("input"));}}catch(e){notice(e,true);}};
$("form").onsubmit=async e=>{e.preventDefault();if(!toolsAvailable){await prepareTools();return;}const b=e.submitter||$("form").querySelector("[type=submit]");if(b.disabled)return;if(!$("url").value.trim()){notice("Paste a media link first.",true);$("url").focus();return;}if(!$("destination").value.trim()){notice("Choose a destination folder.",true);return;}const submittedUrl=$("url").value.trim();const submittedPreview=previewMeta;b.disabled=true;try{const jobId=await invoke("enqueue",{request:{url:submittedUrl,preset:$("format").value,quality:$("quality").value,playlist:!$("playlistChoice").hidden&&$("playlist").checked,organize:organizerEnabled&&!manualDestination,options:mediaOptions(),destination:$("destination").value}});rememberCurrentDownload();if(submittedPreview){queuedPreviews.set(jobId,submittedPreview);while(queuedPreviews.size>20)queuedPreviews.delete(queuedPreviews.keys().next().value);}clearSubmittedLink(submittedUrl,jobId);notice("Added to queue.");if(expanded)show("queue");else await refresh();}catch(err){notice(err,true);}finally{b.disabled=false;}};
$("settingsForm").onsubmit=async e=>{e.preventDefault();if(!$("defaultDest").value.trim()){notice("Choose a default destination.",true);return;}try{const {settings}=await invoke("snapshot");const prefs=readPreferences();prefs.rules=settings.prefs.rules;prefs.category_paths=settings.prefs.category_paths;prefs.detailed_folders=settings.prefs.detailed_folders;await savePrefs(prefs);applyTheme();syncPosition();savedDefaultDestination=$("defaultDest").value;organizerEnabled=$("smartOrganizer").checked;if(!manualDestination)$("destination").value=savedDefaultDestination;syncDestination();rememberCurrentDownload();notice("Settings saved.");await refresh();}catch(err){notice(err,true);}};
function mediaOptions(){
    return {clip_start:$("clipStart").value===""?null:Number($("clipStart").value),clip_end:$("clipEnd").value===""?null:Number($("clipEnd").value),subtitle_languages:$("subtitleLanguages").value.split(",").map(s=>s.trim()).filter(Boolean),subtitle_format:$("subtitleFormat").value,subtitle_auto:$("subtitleAuto").checked,archive:$("archiveMetadata").checked};
}
const audioFormats=["mp3","m4a","wav"];
function updateFormat(){$("quality").disabled=audioFormats.includes($("format").value);}
$("preset").onchange=()=>{const p=$("preset").value;const profile=customProfiles.find(x=>profileKey(x)===p);if(profile){$("format").value=profile.format;$("quality").value=profile.quality;$("archiveMetadata").checked=profile.archive;}else if(p!=="custom"){$("format").value=p;$("quality").value=p==="editor"?"1080":"best";$("archiveMetadata").checked=false;}updateFormat();};
$("format").onchange=()=>{$("preset").value="custom";updateFormat();};
$("quality").onchange=()=>{$("preset").value="custom";updateFormat();};
$("archiveMetadata").onchange=()=>{$("preset").value="custom";};

function syncPosition(){const custom=$("popupMode").value==="Custom";document.querySelectorAll(".custom-position").forEach(e=>e.hidden=!custom);}
let positionTimer;
function livePosition(){syncPosition();clearTimeout(positionTimer);if(!initialized)return;const x=$("popupX").value,y=$("popupY").value;if(x===""||y===""||![x,y].every(v=>Number.isFinite(Number(v))&&Number(v)>=0&&Number(v)<=100))return;
    return saveSection({popup_mode:$("popupMode").value,popup_x:Number(x)/100,popup_y:Number(y)/100}).catch(e=>notice(e,true));
}
$("popupMode").onchange=livePosition;
for(const id of ["popupX","popupY"]){$(id).oninput=()=>{clearTimeout(positionTimer);positionTimer=setTimeout(livePosition,120);};$(id).onchange=livePosition;}
const helpText={material:"Optional visual finish. Clean is the default; textures never change download behavior.",concurrent:"Number of separate downloads running at once. Start with one on a slower connection.",url:"Paste a supported media URL. Preview is optional and does not block downloading.",preset:"Choose a shortcut to set format and resolution. You can adjust either directly.",format:"MP4 prepares H.264/AAC editor media; Original preserves the available media format. Audio exports only sound.",quality:"Sets a maximum video resolution. Best uses the available highest quality.",destination:"Change selects a manual folder. Auto restores the default and organizer.",playlist:"Off downloads one item. Enable only when you want the entire playlist.",organize:"Groups downloads under the default folder using media metadata.",theme:"System follows Windows. Other palettes stay fixed.",accent:"Changes action and progress colors independently of the palette.",startupView:"Choose compact, advanced or the last view when the tray opens.",popupMode:"Tray anchors near its icon; Corner uses the bottom-right; Custom uses percentages of available screen space.",popupX:"0 is left; 100 is right. The popup remains inside the work area.",popupY:"0 is top; 100 is bottom.",fragments:"Number of parallel fragments for one download. More is not always faster.",ytdlp:"Path to a trusted yt-dlp executable.",ffmpeg:"Path to trusted FFmpeg. FFprobe should be beside it.",defaultDest:"Default output folder; manual destinations override it.",clipStart:"Optional starting time in seconds.",clipEnd:"Optional ending time in seconds, greater than start.",subtitleLanguages:"Comma-separated language codes such as en, tr.",subtitleFormat:"Subtitle file format: SRT or VTT."};
for(const [id,message] of Object.entries(helpText)){
    const label=document.querySelector('label[for="'+id+'"]')||$(id)?.closest("label");if(!label)continue;
    const b=el("button","info","i");b.type="button";b.title=message;b.setAttribute("aria-label","Help: "+message);
    b.onclick=e=>{e.preventDefault();const p=$("helpPanel");p.textContent=message;p.hidden=false;clearTimeout(helpTimer);helpTimer=setTimeout(()=>p.hidden=true,6500);};
    label.append(b);
}
let helpTimer;
function applyTheme(){const theme=$("theme").value;document.documentElement.dataset.theme=theme==="System"?(matchMedia("(prefers-color-scheme: light)").matches?"Light":"Dark"):theme;document.documentElement.dataset.accent=$("accent").value;document.documentElement.dataset.material=$("material").value;}
function readPreferences(){return {language:$("language").value,open_hotkey:$("openHotkey").value,shortcuts:shortcutBindings,always_on_top:$("alwaysOnTop").checked,last_destination:lastDownloadDestination,last_organize:lastDownloadOrganize,last_video_quality:lastVideoQuality,popup_mode:$("popupMode").value,popup_x:Number($("popupX").value)/100,popup_y:Number($("popupY").value)/100,startup_view:$("startupView").value,theme:$("theme").value,material:$("material").value,detailed_folders:$("detailedFolders").checked,accent:$("accent").value,clipboard_monitor:$("clipboardMonitor").checked,smart_organizer:$("smartOrganizer").checked,notifications:$("notifications").checked,run_on_startup:$("runOnStartup").checked,default_preset:$("preset").value.startsWith("profile:")?"custom":$("preset").value,default_format:$("format").value,default_quality:$("quality").value,fragment_concurrency:Number($("fragments").value),concurrent_downloads:Number($("concurrent").value),profiles:customProfiles,rules:savedRules,category_paths:savedPaths};}
function applyPreferences(p){if(!p)return;customProfiles=p.profiles||[];savedRules=p.rules||[];savedPaths=p.category_paths||{};renderProfiles();renderRules();renderPaths();const entries={language:p.language||"en",openHotkey:p.open_hotkey||"Ctrl+Alt+Space",popupMode:p.popup_mode,popupX:Math.round(p.popup_x*100),popupY:Math.round(p.popup_y*100),startupView:p.startup_view,theme:p.theme,material:p.material||"Flat",accent:p.accent,preset:p.default_preset==="archive"?"custom":p.default_preset,format:p.default_format,quality:p.default_quality,fragments:String(p.fragment_concurrency),concurrent:String(p.concurrent_downloads||1)};for(const [k,v] of Object.entries(entries))$(k).value=v;setLanguage($("language").value);shortcutBindings={...defaultShortcuts,open:p.open_hotkey||defaultShortcuts.open,...p.shortcuts};$("alwaysOnTop").checked=p.always_on_top!==false;lastDownloadDestination=p.last_destination||null;lastDownloadOrganize=p.last_organize!==false;lastVideoQuality=p.last_video_quality||"1080";if(lastDownloadDestination){$("destination").value=lastDownloadDestination;manualDestination=!lastDownloadOrganize||lastDownloadDestination!==savedDefaultDestination;}$("clipboardMonitor").checked=p.clipboard_monitor;$("notifications").checked=p.notifications!==false;$("runOnStartup").checked=p.run_on_startup!==false;$("archiveMetadata").checked=p.default_preset==="archive";$("smartOrganizer").checked=p.smart_organizer;$("detailedFolders").checked=Boolean(p.detailed_folders);organizerEnabled=p.smart_organizer;syncDestination();updateFormat();applyTheme();syncPosition();if(p.startup_view==="advanced"||(p.startup_view==="remember"&&localStorage.getItem("expanded")==="true"))toggleExpanded(true).catch(e=>notice(e,true));}
async function liveAppearance(){applyTheme();if(initialized)try{await saveSection({theme:$("theme").value,accent:$("accent").value,material:$("material").value});}catch(e){notice(e,true);}}
$("language").onchange=async()=>{try{await saveSection({language:$("language").value});setLanguage($("language").value);$("expand").textContent=t(expanded?"Quick view":"Advanced");delete $("queueList").dataset.signature;delete $("historyList").dataset.signature;show(current);updateSize();}catch(e){notice(e,true);}};
$("alwaysOnTop").onchange=async()=>{try{await saveSection({always_on_top:$("alwaysOnTop").checked});}catch(e){notice(e,true);}};
$("theme").onchange=liveAppearance;$("material").onchange=liveAppearance;$("accent").onchange=liveAppearance;
$("runOnStartup").onchange=async()=>{const wanted=$("runOnStartup").checked;try{await saveSection({run_on_startup:wanted});}catch(e){$("runOnStartup").checked=!wanted;notice(e,true);}};$("clipboardMonitor").onchange=async()=>{const value=$("clipboardMonitor").checked;if(!value){pendingCopied="";$("pendingLink").hidden=true;}try{await saveSection({clipboard_monitor:value});}catch(e){$("clipboardMonitor").checked=!value;notice(e,true);}};
for(const [id,key] of [["notifications","notifications"],["smartOrganizer","smart_organizer"],["detailedFolders","detailed_folders"]]){
    $(id).addEventListener("change",async()=>{const value=$(id).checked;try{await saveSection({[key]:value});if(id==="smartOrganizer"){organizerEnabled=value;syncDestination();rememberCurrentDownload();}}catch(e){$(id).checked=!value;notice(e,true);}});
}
for(const [id,key] of [["startupView","startup_view"],["fragments","fragment_concurrency"],["concurrent","concurrent_downloads"]]){
    $(id).addEventListener("change",()=>saveSection({[key]:id==="startupView"?$(id).value:Number($(id).value)}).catch(e=>notice(e,true)));
}
matchMedia("(prefers-color-scheme: light)").addEventListener("change",applyTheme);
let autoFilled="",pendingCopied="",clipboardBusy=false,clipboardAutoReady=false;
function sameLink(a,b){
    try{return new URL(a.startsWith("www.")?"https://"+a:a).href===new URL(b.startsWith("www.")?"https://"+b:b).href;}catch(_){return a.trim()===b.trim();}
}
function clearSubmittedLink(url,id){
    if(pendingCopied&&sameLink(pendingCopied,url)){pendingCopied="";$("pendingLink").hidden=true;}
    if(!$("url").value.trim()||!sameLink($("url").value.trim(),url))return;
    if(id&&previewMeta){queuedPreviews.set(id,previewMeta);while(queuedPreviews.size>20)queuedPreviews.delete(queuedPreviews.keys().next().value);}
    $("url").value="";autoFilled="";$("url").dispatchEvent(new Event("input"));
}
function applySubmittedLinks(events){for(const [id,url] of events)clearSubmittedLink(url,id);}
function acceptCopied(value){
    if(!value)return;
    const input=$("url");if(input.value===value)return;
    if(!input.value.trim()||input.value===autoFilled){input.value=value;autoFilled=value;pendingCopied="";$("pendingLink").hidden=true;input.dispatchEvent(new Event("input"));}
    else if(input.value!==value){pendingCopied=value;$("pendingLink").hidden=false;}
}
async function checkClipboard(){
    if(!clipboardAutoReady||clipboardBusy||!initialized||!$("clipboardMonitor").checked||!document.hasFocus()||dialogOpen)return;
    clipboardBusy=true;
    try{applySubmittedLinks(await invoke("take_submitted_links"));const value=await invoke("take_clipboard_link");if(value)acceptCopied(value);}catch(_){}finally{clipboardBusy=false;}
}
$("useCopied").onclick=()=>{if(pendingCopied){$("url").value="";acceptCopied(pendingCopied);}};
$("clearLink").onclick=()=>{$("url").value="";autoFilled="";$("url").dispatchEvent(new Event("input"));$("url").focus();};
$("quickQueue").onclick=async()=>{try{if(expanded&&current==="queue"){await toggleExpanded(false);return;}if(!expanded)await toggleExpanded(true);show("queue");}catch(e){notice(e,true);}};
window.addEventListener("focus",()=>{refresh();checkClipboard();if($("url").value.trim()&&$("previewText").textContent==="Loading preview...")fetchPreview();});setInterval(checkClipboard,500);

$("batchAdd").onclick=async()=>{if(!toolsAvailable){await prepareTools();return;}let b=$("batchAdd");b.disabled=true;try{let urls=await invoke("validate_links",{urls:$("batchUrls").value.split(/\r?\n/).map(s=>s.trim()).filter(Boolean)});let added=0;for(let url of urls){try{await invoke("enqueue",{request:{url,preset:$("format").value,quality:$("quality").value,playlist:false,organize:organizerEnabled&&!manualDestination,options:mediaOptions(),destination:$("destination").value}});added++;}catch(e){notice(e,true);}}if(added)rememberCurrentDownload();if(added===urls.length)$("batchUrls").value="";notice(added+" of "+urls.length+" links added.");show("queue");}catch(e){notice(e,true);}finally{b.disabled=false;}};
document.querySelectorAll(".queue-action").forEach(b=>b.onclick=async()=>{try{await invoke("queue_action",{action:b.dataset.action});await refresh();}catch(e){notice(e,true);}});
const defaultShortcuts={open:"Ctrl+Alt+Space",download:"Ctrl+Shift+D",mp3:"Ctrl+Shift+3",mp4:"Ctrl+Shift+4",folder:"Ctrl+Shift+O",submit:"Ctrl+Enter",link:"Ctrl+L",hide:"Esc"};
const shortcutActions=[["open","Open / hide QuickClip321",true],["download","Download copied link (last format)",true],["mp3","Download copied link as MP3",true],["mp4","Download copied link as MP4",true],["folder","Open downloads folder",true],["submit","Download current link",false],["link","Focus link field",false],["hide","Hide window",false]];
let shortcutBindings={...defaultShortcuts},recordingShortcut=null,recordTimer,recordReady=false,shortcutStatus={},testingShortcuts=false,testTimer;
function keyName(e){
    if(e.metaKey||e.getModifierState?.("AltGraph"))return null;
    const k=e.code.startsWith("Key")?e.code.slice(3):e.code.startsWith("Digit")?e.code.slice(5):({"Space":"Space","Enter":"Enter","Escape":"Esc","Tab":"Tab"}[e.code]||(/^F([1-9]|1[0-2])$/.test(e.code)?e.code:null));
    if(!k)return null;
    return [e.ctrlKey?"Ctrl":null,e.altKey?"Alt":null,e.shiftKey?"Shift":null,k].filter(Boolean).join("+");
}
function validBinding(key,global){
    if(key==="Off")return true;
    if(!key)return false;
    const parts=key.split("+"),last=parts.at(-1);
    if(global&&(!parts.includes("Ctrl")&&!parts.includes("Alt")||(parts.includes("Ctrl")&&parts.includes("Alt")&&last.length===1)))return false;
    return global||parts.length>1||last==="Esc"||/^F\d+$/.test(last);
}
function renderShortcuts(){
    const list=$("shortcutList");list.replaceChildren();
    for(const [id,label,global] of shortcutActions){
        const row=el("div","shortcut-row"),name=el("div","shortcut-name");name.append(el("strong","",t(label)),el("small","",t(global?"Global":"In app")));
        const status=el("small","shortcut-state");status.dataset.shortcutState=id;name.append(status);
        const key=el("button","secondary shortcut-key",t(shortcutBindings[id]));key.onclick=()=>startRecording(id);
        const off=el("button","small",t("Off"));off.onclick=()=>saveBinding(id,"Off");
        row.append(name,key,off);list.append(row);
    }
    updateShortcutStatus(shortcutStatus);
}
function updateShortcutStatus(status){
    shortcutStatus=status||{};
    for(const n of document.querySelectorAll("[data-shortcut-state]")){
        const id=n.dataset.shortcutState,s=shortcutStatus[id];
        n.textContent=s?t(s.status)+(s.hits?" | "+t("Triggered")+" "+s.hits+"x":""):t("Checking");
        n.dataset.active=String(s?.status==="Active"||s?.status==="In app");
    }
}
async function saveBinding(id,key){
    if(key!=="Off"&&Object.entries(shortcutBindings).some(([other,k])=>other!==id&&k===key)){notice("Duplicate shortcut",true);return;}
    const next={...shortcutBindings,[id]:key};
    try{await saveSection({shortcuts:next});shortcutBindings=next;renderShortcuts();}catch(e){notice(e,true);}
}
async function startRecording(id){
    await stopShortcutTest();await stopRecording();
    recordingShortcut=id;recordReady=false;
    $("shortcutCaptureHint").textContent=t("Press a new shortcut (Esc cancels)");
    try{await invoke("shortcut_capture",{active:true});}catch(e){await stopRecording();notice(e,true);return;}
    setTimeout(()=>{if(recordingShortcut===id)recordReady=true;},180);
    recordTimer=setTimeout(stopRecording,14000);
}
async function stopRecording(){
    clearTimeout(recordTimer);recordingShortcut=null;recordReady=false;$("shortcutCaptureHint").textContent="";
    try{await invoke("shortcut_capture",{active:false});}catch(_){}
}
async function stopShortcutTest(){
    clearTimeout(testTimer);testingShortcuts=false;$("testShortcuts").textContent=t("Test shortcuts");
    try{await invoke("shortcut_test",{active:false});}catch(_){}
}
$("testShortcuts").onclick=async()=>{
    if(testingShortcuts){await stopShortcutTest();return;}
    await stopRecording();
    try{await invoke("shortcut_test",{active:true});testingShortcuts=true;$("testShortcuts").textContent=t("Stop test");notice("Test mode: shortcuts will not start downloads.");testTimer=setTimeout(stopShortcutTest,29500);}
    catch(e){notice(e,true);}
};
$("resetShortcuts").onclick=async()=>{await stopRecording();try{await saveSection({shortcuts:defaultShortcuts,open_hotkey:defaultShortcuts.open});shortcutBindings={...defaultShortcuts};renderShortcuts();}catch(e){notice(e,true);}};
window.addEventListener("blur",()=>{if(recordingShortcut)stopRecording();});
document.addEventListener("keydown",e=>{
    if(e.repeat||e.isComposing)return;
    const key=keyName(e);
    if(recordingShortcut){e.preventDefault();e.stopImmediatePropagation();if(e.key==="Escape"){stopRecording();return;}if(!recordReady||!key)return;const id=recordingShortcut,global=shortcutActions.find(x=>x[0]===id)[2];if(!validBinding(key,global)){notice("Invalid shortcut",true);return;}stopRecording().then(()=>saveBinding(id,key));return;}
    if(dialogOpen||!key)return;
    const action=["submit","link","hide"].find(id=>shortcutBindings[id]===key);
    if(!action)return;
    if(shortcutActions.some(([id,_,global])=>global&&shortcutBindings[id]===key))return;
    e.preventDefault();invoke("shortcut_hit",{action}).catch(()=>{});if(testingShortcuts){notice("Shortcut recognized");return;}
    if(action==="submit")$("form").requestSubmit();else if(action==="link"){$("url").focus();$("url").select();}else invoke("hide_window").catch(e=>notice(e,true));
},true);

let expanded=false;
let customProfiles=[],savedRules=[],savedPaths={};
const categories=["video","rap","pop","rock","electronic","music","film","games","audio","beats"];
const profileKey=p=>"profile:"+p.name;
let settingsWrites=Promise.resolve();
let lastDownloadDestination=null,lastDownloadOrganize=true,lastVideoQuality="1080";
function rememberCurrentDownload(){
    if(!initialized||!clipboardAutoReady)return;
    const preset=$("format").value,quality=$("quality").value,destination=$("destination").value,organize=organizerEnabled&&!manualDestination;
    if(!destination)return;
    lastDownloadDestination=destination;lastDownloadOrganize=organize;if(!audioFormats.includes(preset))lastVideoQuality=quality;
    serializeSettings(()=>invoke("remember_download",{preset,quality,destination,organize})).catch(e=>notice(e,true));
}
for(const id of ["preset","format","quality","destination"])$(id).addEventListener("change",rememberCurrentDownload);

function serializeSettings(fn){const next=settingsWrites.then(fn);settingsWrites=next.catch(()=>{});return next;}
async function savePrefs(prefs){
    const draft={destination:$("defaultDest").value,yt_dlp:$("ytdlp").value,ffmpeg:$("ffmpeg").value,prefs};
    await serializeSettings(()=>invoke("save_settings",{settings:draft}));
}
async function saveSection(patch){
    await serializeSettings(async()=>{const {settings}=await invoke("snapshot");settings.prefs={...settings.prefs,...patch};await invoke("save_settings",{settings});});
}
function renderProfiles(){
    const select=$("preset"),previous=select.value;select.querySelectorAll("[data-custom-profile]").forEach(n=>n.remove());
    const list=$("profileList");list.replaceChildren();
    for(const p of customProfiles){
        const o=new Option(p.name,profileKey(p));o.dataset.customProfile="1";select.append(o);
        const card=el("div","config-row");card.append(el("span","",p.name+" / "+p.format.toUpperCase()+" / "+p.quality));
        const remove=el("button","small","Remove");remove.type="button";remove.onclick=async()=>{const old=customProfiles;customProfiles=old.filter(x=>x.name!==p.name);try{await saveSection({profiles:customProfiles});renderProfiles();}catch(e){customProfiles=old;notice(e,true);}};card.append(remove);list.append(card);
    }
    if([...select.options].some(x=>x.value===previous))select.value=previous;else select.value="custom";
    if(!customProfiles.length)list.append(el("p","sub","No custom presets yet."));
}
$("profileForm").onsubmit=async e=>{e.preventDefault();const name=$("profileName").value.trim();if(customProfiles.length>=50){notice("Maximum 50 custom presets.",true);return;}if(!name){notice("Enter a profile name.",true);return;}if(customProfiles.some(p=>p.name.toLowerCase()===name.toLowerCase())){notice("A profile with this name already exists.",true);return;}const old=customProfiles;customProfiles=[...old,{name,format:$("format").value,quality:$("quality").value,archive:mediaOptions().archive}];try{await saveSection({profiles:customProfiles});renderProfiles();$("preset").value="profile:"+name;$("profileName").value="";notice("Profile saved.");}catch(e){customProfiles=old;notice(e,true);}};
function renderRules(){const list=$("ruleList");list.replaceChildren();savedRules.forEach((r,i)=>{const card=el("div","config-row");card.append(el("span","",r.contains+" ? "+r.category));const b=el("button","small","Remove");b.type="button";b.onclick=async()=>{const old=savedRules;const next=old.filter((_,idx)=>idx!==i);try{await saveSection({rules:next});savedRules=next;renderRules();}catch(e){notice(e,true);}};card.append(b);list.append(card);});}
function renderPaths(){const list=$("categoryPaths");list.replaceChildren();for(const cat of categories){const label=el("label","",cat[0].toUpperCase()+cat.slice(1));label.htmlFor="path-"+cat;const row=el("div","row");const input=el("input");input.id="path-"+cat;input.value=savedPaths[cat]||"";input.placeholder="Use default folder";const browse=el("button","secondary","Browse");browse.type="button";browse.onclick=()=>pick(input.id,true);row.append(input,browse);list.append(label,row);}}
$("addRule").onclick=async()=>{const term=$("ruleTerm").value.trim();if(!term){notice("Enter a title phrase.",true);return;}if(savedRules.length>=100){notice("Maximum 100 rules.",true);return;}const next=[...savedRules,{contains:term,category:$("ruleCategory").value}];try{await saveSection({rules:next});savedRules=next;$("ruleTerm").value="";renderRules();}catch(e){notice(e,true);}};
$("organizerForm").onsubmit=async e=>{e.preventDefault();const old=savedPaths;const paths={};for(const cat of categories){const v=$("path-"+cat).value.trim();if(v){if(!/^(?:[A-Za-z]:[\\/]|\\\\)/.test(v)){notice("Choose an absolute folder for "+cat+".",true);return;}paths[cat]=v;}}savedPaths=paths;try{await saveSection({rules:savedRules,category_paths:savedPaths,detailed_folders:$("detailedFolders").checked});notice("Organizer saved.");}catch(e){savedPaths=old;notice(e,true);}};
async function prepareTools(){
    if(setupRunning)return false;
    setupRunning=true;dialogOpen=true;const d=$("firstSetup");
    $("setupTitle").textContent=t("Getting QuickClip321 ready...");
    $("setupState").textContent=t("Downloading and verifying required media tools. Please keep QuickClip321 running.");
    $("setupProgress").hidden=false;$("setupActions").hidden=true;$("setupHint").hidden=true;
    $("managedSetup").disabled=true;$("quickSetup").disabled=true;
    d.oncancel=e=>e.preventDefault();
    if(!d.open)d.showModal();
    $("setupProgress").removeAttribute("value");
    const progressTimer=setInterval(async()=>{
        try {
            const s=await invoke("setup_progress");
            if(!setupRunning)return;
            const mb=n=>(n/1048576).toFixed(1)+" MB";
            $("setupState").textContent=setupStage(s.stage)+(s.received?" - "+mb(s.received)+(s.total?" / "+mb(s.total):""):"");
            if(s.total>0){$("setupProgress").max=s.total;$("setupProgress").value=Math.min(s.received,s.total);}
            else $("setupProgress").removeAttribute("value");
        } catch(_) {}
    },400);
    try{
        const settings=await serializeSettings(()=>invoke("setup_managed_tools"));
        $("ytdlp").value=settings.yt_dlp;$("ffmpeg").value=settings.ffmpeg;
        toolsAvailable=await invoke("tools_ready");
        if(!toolsAvailable)throw Error("Setup did not finish. Please retry.");
        d.close();dialogOpen=false;notice("QuickClip321 is ready.");
        return true;
    }catch(e){
        $("setupTitle").textContent=t("Setup needs another try");
        $("setupState").textContent=String(e);
        $("setupProgress").hidden=true;$("setupActions").hidden=false;
        $("setupLater").onclick=()=>{d.close();dialogOpen=false;};
        return false;
    }finally{
        clearInterval(progressTimer);setupRunning=false;$("managedSetup").disabled=false;$("quickSetup").disabled=false;
        await refresh();
    }
}
$("setupRetry").onclick=prepareTools;
async function installManaged(){if(await confirmSetup())await prepareTools();}
function confirmSetup(){return new Promise(resolve=>{const d=$("setupConfirm");dialogOpen=true;const finish=ok=>{d.close();dialogOpen=false;resolve(ok);};$("setupYes").onclick=()=>finish(true);$("setupNo").onclick=()=>finish(false);d.oncancel=e=>{e.preventDefault();finish(false);};d.showModal();});}
$("managedSetup").onclick=installManaged;$("quickSetup").onclick=prepareTools;
$("detectTools").onclick=async()=>{try{const [y,f]=await invoke("detect_tools");if(y)$("ytdlp").value=y;if(f)$("ffmpeg").value=f;notice(y&&f?"Both tools detected. Save changes to apply.":"Some tools are missing. Select their executables manually.",!(y&&f));}catch(e){notice(e,true);}};
async function loadDiagnostics(){try{$("diagReport").textContent=await invoke("diagnostics");}catch(e){notice(e,true);}}
$("refreshDiag").onclick=loadDiagnostics;
async function toggleExpanded(next){await invoke("set_mode",{expanded:next});expanded=next;document.body.classList.toggle("expanded",expanded);localStorage.setItem("expanded",String(expanded));$("workspace").hidden=!expanded;$("expand").textContent=t(expanded?"Quick view":"Advanced");if(expanded)show("queue");syncQueueButton();}$("expand").onclick=()=>toggleExpanded(!expanded).catch(e=>notice(e,true));


window.addEventListener("blur",()=>setTimeout(()=>{if(clipboardAutoReady&&!dialogOpen&&!recordingShortcut&&!$("alwaysOnTop").checked&&!document.hasFocus())invoke("hide_window").catch(()=>{});},250));
function showThumbnail(data){const im=$("thumbnail");im.hidden=false;$("previewIcon").hidden=true;im.src=data;}
$("thumbnail").onerror=()=>{$("thumbnail").hidden=true;$("previewIcon").hidden=false;};
let previewTimer,previewRunning=false,previewPending=false,previewGeneration=0;
let previewMeta=null,activePreviewId=null;const queuedPreviews=new Map();
function updateSize(job=null){
    const node=$("sizeEstimate");
    node.hidden=!previewMeta;
    if(!previewMeta)return;
    if((job?job.playlist:$("playlist").checked)||previewMeta.playlist){node.textContent=t("Playlist size unknown");return;}
    const preset=job?job.preset:$("format").value,quality=audioFormats.includes(preset)?"best":job?job.quality:$("quality").value;
    const match=previewMeta.sizes?.find(x=>x.preset===preset&&x.quality===quality);
    if(!match){node.textContent=t("Size unknown");return;}
    const n=match.bytes;node.textContent=t("Source download estimate")+": ~"+(n>=1073741824?(n/1073741824).toFixed(2)+" GiB":(n/1048576).toFixed(1)+" MiB");
}
let frameBusy=false,lastFrame=0,frameFailures=0,frameJob=null,frameSeconds=0,lastFrameData=null,liveJob=null,previewResetTimer=null;
function resetPreview(){
    previewGeneration++;activePreviewId=null;previewMeta=null;liveJob=null;frameJob=null;lastFrameData=null;
    $("thumbnail").hidden=true;$("thumbnail").removeAttribute("src");$("previewIcon").hidden=false;
    $("previewText").textContent=t("Paste a media link to get started.");updateSize();
}
setInterval(()=>{if(liveJob&&!$("url").value.trim()&&document.hasFocus())sampleLiveFrame(liveJob);},500);
async function sampleLiveFrame(job){
    if(job.status!=="running"||job.playlist||audioFormats.includes(job.preset)||!document.hasFocus()||frameBusy||!job.message.startsWith("Downloading")||job.progress<=0)return;
    if(frameJob!==job.id){frameJob=job.id;frameFailures=0;lastFrame=0;frameSeconds=0;lastFrameData=null;}
    if(Date.now()-lastFrame<Math.min(4000,500*(1+frameFailures)))return;
    const meta=queuedPreviews.get(job.id);

    const generation=previewGeneration;
    frameBusy=true;lastFrame=Date.now();
    try{
        // Progress is approximate when separate streams are fetched. Stay behind the estimated edge.
        const seconds=meta?.duration?Math.max(0,meta.duration*job.progress-2-frameFailures*2):frameSeconds;
        const data=await invoke("live_preview_frame",{id:job.id,seconds});
        if(generation!==previewGeneration||$("url").value.trim()||frameJob!==job.id||liveJob?.id!==job.id)return;
        if(data&&/^data:image\/jpeg;base64,/.test(data)){if(lastFrameData!==data){showThumbnail(data);lastFrameData=data;}frameSeconds+=0.5;frameFailures=0;}
        else frameFailures=Math.min(frameFailures+1,4);
    }catch(_){frameFailures=Math.min(frameFailures+1,4);}
    finally{frameBusy=false;}
}
function showActivePreview(job){
    const p=queuedPreviews.get(job.id);
    if(activePreviewId===job.id)return;if(!p){activePreviewId=job.id;previewMeta=null;updateSize();$("previewText").textContent=t("Downloading");$("thumbnail").hidden=true;$("previewIcon").hidden=false;return;}
    activePreviewId=job.id;previewMeta=p;$("previewText").textContent=p.title;updateSize(job);
    if(p.thumbnail&&/^data:image\/(jpeg|png|webp);base64,/.test(p.thumbnail))showThumbnail(p.thumbnail);
}
for(const id of ["preset","format","quality","playlist"])$(id).addEventListener("change",()=>{if($("url").value.trim())updateSize();});
$("openDownloads").onclick=()=>invoke("open_downloads").catch(e=>notice(e,true));
$("quickTrim").onclick=async()=>{try{if(!expanded)await toggleExpanded(true);show("home");$("clipStart").focus();}catch(e){notice(e,true);}};

function playlistHint(raw){
    try{const u=new URL(raw.startsWith("www.")?"https://"+raw:raw);if(!["http:","https:"].includes(u.protocol))return false;
        return Boolean(u.searchParams.get("list"))||/(?:^|\/)(playlist|playlists|sets|albums|album)(?:\/|$)/i.test(u.pathname);
    }catch(_){return false;}
}
function setPlaylistVisibility(visible,count){
    $("playlistChoice").hidden=!visible;
    if(!visible)$("playlist").checked=false;
    $("playlistLabel").textContent=visible&&Number.isInteger(count)&&count>1?"Entire playlist ("+count+" items)":"Entire playlist";
}
async function fetchPreview(){
    if(!toolsAvailable)return;if(previewRunning){previewPending=true;return;}
    const url=$("url").value.trim();if(!document.hasFocus())return;if(!/^(https?:\/\/|www\.)\S+$/i.test(url))return;
    previewRunning=true;previewPending=false;const generation=previewGeneration;
    try{let p=await invoke("media_preview",{url});if($("url").value.trim()!==url||generation!==previewGeneration)return;previewMeta=p;setPlaylistVisibility(Boolean(p.playlist)||playlistHint(url),p.count);updateSize();let detail=[p.channel,p.duration?Math.round(p.duration)+"s":"",p.qualities.length?p.qualities.slice(0,4).join("p / ")+"p":"",p.playlist?(p.count||"Unknown")+" items":""].filter(Boolean).join(" | ");$("previewText").textContent=p.title+(detail?" - "+detail:"");if(p.thumbnail&&/^data:image\/(jpeg|png|webp);base64,/.test(p.thumbnail)){showThumbnail(p.thumbnail);}}
    catch(_){if($("url").value.trim()===url&&generation===previewGeneration)$("previewText").textContent=t("Preview unavailable. You can still download.");}
    finally{previewRunning=false;if(previewPending&&clipboardAutoReady)fetchPreview();}
}
let previousClipUrl="";
$("url").addEventListener("input",()=>{clearTimeout(previewResetTimer);previewResetTimer=null;liveJob=null;frameJob=null;let url=$("url").value.trim();if(url!==previousClipUrl){$("clipStart").value="";$("clipEnd").value="";previousClipUrl=url;}previewMeta=null;activePreviewId=null;updateSize();previewGeneration++;setPlaylistVisibility(playlistHint(url));$("thumbnail").hidden=true;$("thumbnail").removeAttribute("src");$("previewIcon").hidden=false;$("playlist").checked=false;clearTimeout(previewTimer);$("previewText").textContent=t(url?"Loading preview...":"Paste a media link to get started.");if(url&&clipboardAutoReady)previewTimer=setTimeout(fetchPreview,800);});
function contrast(a,b){const l=c=>{let x=c.match(/\d+(?:\.\d+)?/g).slice(0,3).map(v=>Number(v)/255).map(v=>v<=.04045?v/12.92:Math.pow((v+.055)/1.055,2.4));return .2126*x[0]+.7152*x[1]+.0722*x[2];};let x=l(a),y=l(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05);}
function themeSmoke(){const root=document.documentElement,oldTheme=$("theme").value,oldAccent=$("accent").value;const bg=new Set(),accents=new Set();let ok=true;for(let t of ["Dark","Light","Midnight","AMOLED","Forest","Sand"]){$("theme").value=t;for(let a of ["Purple","Graphite","Ocean","Emerald","Rose","Amber"]){$("accent").value=a;applyTheme();let body=getComputedStyle(document.body),button=getComputedStyle(document.querySelector(".primary")),input=getComputedStyle($("url"));bg.add(body.backgroundColor);accents.add(button.backgroundColor);ok=ok&&contrast(body.color,body.backgroundColor)>=4.5&&contrast(button.color,button.backgroundColor)>=4.5&&contrast(input.color,input.backgroundColor)>=4.5;}}$("theme").value=oldTheme;$("accent").value=oldAccent;applyTheme();return ok&&bg.size===6&&accents.size===6;}
async function smokeChecks(){
    const smoke=await invoke("smoke_active");
    const mode=smoke?await invoke("visual_page"):"";
    const firstRunFixture=smoke&&mode.startsWith("first-run:");
    if(!smoke||firstRunFixture){
        if(!toolsAvailable)await prepareTools();
        if(firstRunFixture&&toolsAvailable){
            const url=mode.slice("first-run:".length);
            if(/^http:\/\/127\.0\.0\.1:\d+\//.test(url)){
                $("url").value=url;$("url").dispatchEvent(new Event("input"));
                $("form").requestSubmit();
            }
        }
        if(!smoke){clipboardAutoReady=true;checkClipboard();}
        return;
    }
    if(mode==="ux-persist-write"||mode==="ux-persist-read"){
        const patch={language:"tr",theme:"Forest",accent:"Ocean",material:"Carbon",always_on_top:false,shortcuts:{...defaultShortcuts,mp3:"Ctrl+Shift+8"},rules:[{contains:"Persistence fixture",category:"games"}],profiles:[{name:"Persist 720p",format:"editor",quality:"720",archive:false}],category_paths:{games:"C:\\QuickClipFixture"},detailed_folders:true,notifications:false,run_on_startup:false,popup_mode:"Custom",popup_x:.2,popup_y:.3};
        if(mode==="ux-persist-write")await saveSection(patch);
        const p=(await invoke("snapshot")).settings.prefs;
        const equal=Object.entries(patch).every(([k,v])=>JSON.stringify(p[k])===JSON.stringify(v)||k==="shortcuts"&&Object.entries(v).every(([a,b])=>p[k][a]===b));
        const visible=mode==="ux-persist-write"||($("language").value==="tr"&&$("theme").value==="Forest"&&!$("alwaysOnTop").checked&&shortcutBindings.mp3==="Ctrl+Shift+8"&&savedRules[0]?.contains==="Persistence fixture");
        await invoke("persist_fixture_report",{passed:equal&&visible});return;
    }
    let persistOk=true;const restartCase=await invoke("visual_page");
    if(restartCase==="persist-read"){persistOk=$("theme").value==="Forest"&&$("accent").value==="Ocean"&&$("material").value==="Carbon"&&$("popupMode").value==="Custom"&&!$("runOnStartup").checked;}
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
    $("popupMode").value="Corner";syncPosition();const hidden=document.querySelector(".custom-position").hidden;$("popupMode").value="Custom";syncPosition();const visible=!document.querySelector(".custom-position").hidden;$("popupMode").value="Tray";syncPosition();
    let configOk=false;
    try{
        await saveSection({profiles:[{name:"Fixture 720p",format:"editor",quality:"720",archive:false}],rules:[{contains:"Fixture",category:"video"}]});
        const {settings}=await invoke("snapshot");
        customProfiles=settings.prefs.profiles;savedRules=settings.prefs.rules;renderProfiles();renderRules();
        $("preset").value="profile:Fixture 720p";$("preset").onchange();
        const selected=$("quality").value==="720"&&$("format").value==="editor";
        const report=await invoke("diagnostics");
        configOk=selected&&savedRules.length===1&&report.includes("QuickClip321")&&!report.includes(settings.destination)&&report.includes("FFprobe");
        await saveSection({profiles:[],rules:[]});customProfiles=[];savedRules=[];renderProfiles();renderRules();$("preset").value="editor";$("preset").onchange();
    }catch(e){notice(e,true);}
    let clipOk=true;
    $("url").value="https://example.com/clip-a";$("url").dispatchEvent(new Event("input"));$("clipStart").value="1";$("clipEnd").value="5";
    $("url").dispatchEvent(new Event("input"));clipOk=clipOk&&mediaOptions().clip_start===1&&mediaOptions().clip_end===5;
    $("url").value="https://example.com/clip-b";$("url").dispatchEvent(new Event("input"));clipOk=clipOk&&mediaOptions().clip_start===null&&mediaOptions().clip_end===null;
    $("clipStart").value="1";$("clipEnd").value="5";autoFilled=$("url").value;acceptCopied("https://example.com/clip-c");clipOk=clipOk&&mediaOptions().clip_start===null&&mediaOptions().clip_end===null;
    $("clipStart").value="1";$("clipEnd").value="5";$("clearLink").click();clipOk=clipOk&&mediaOptions().clip_start===null&&mediaOptions().clip_end===null;

    if(restartCase==="persist-write"){
      $("theme").value="Forest";$("accent").value="Ocean";$("material").value="Carbon";await liveAppearance();
      $("popupMode").value="Custom";$("popupX").value="20";$("popupY").value="30";await livePosition();
      $("runOnStartup").checked=false;await $("runOnStartup").onchange();
      const {settings}=await invoke("snapshot");persistOk=settings.prefs.theme==="Forest"&&settings.prefs.popup_x===.2&&!settings.prefs.run_on_startup;
    }
    let playlistOk=true;
    for(const [url,expected] of [["",false],["https://youtu.be/abc",false],["https://www.youtube.com/watch?v=abc&list=RDabc",true],["https://www.youtube.com/playlist?list=PLabc",true],["https://soundcloud.com/artist/sets/album",true],["https://example.com/video",false]]){
        $("url").value=url;$("url").dispatchEvent(new Event("input"));playlistOk=playlistOk&&$("playlistChoice").hidden===!expected&&!$("playlist").checked;
    }
    setPlaylistVisibility(true,12);$("playlist").checked=true;$("url").value="https://example.com/video";$("url").dispatchEvent(new Event("input"));playlistOk=playlistOk&&$("playlistChoice").hidden&&!$("playlist").checked;
    $("url").value="";$("url").dispatchEvent(new Event("input"));
    const oldMaterial=$("material").value;let materialOk=true;const finishes=new Set();
    for(const material of ["Flat","Glass","Carbon","Sand"]){$("material").value=material;applyTheme();finishes.add(getComputedStyle(document.querySelector(".shell")).backgroundImage);materialOk=materialOk&&document.documentElement.dataset.material===material;}
    $("material").value=oldMaterial;applyTheme();materialOk=materialOk&&finishes.size===4;
    let workflowOk=false;
    try{
        const original=organizerEnabled;manualDestination=false;organizerEnabled=true;syncDestination();
        const auto=$("autoFolder").hidden;
        $("destination").value="manual fixture";$("destination").dispatchEvent(new Event("input"));
        const manual=manualDestination&&!$("autoFolder").hidden;
        $("autoFolder").click();const reset=!manualDestination&&$("destination").value===savedDefaultDestination;
        organizerEnabled=false;syncDestination();const disabled=!organizerEnabled&&!manualDestination;organizerEnabled=original;syncDestination();
        if(expanded)await toggleExpanded(false);
        await $("quickQueue").onclick();const opened=expanded&&current==="queue"&&$("quickQueue").getAttribute("aria-expanded")==="true";
        await $("quickQueue").onclick();const closed=!expanded&&$("workspace").hidden&&$("quickQueue").getAttribute("aria-expanded")==="false";
        $("preset").value="original";$("preset").onchange();const simple=$("format").value==="original"&&$("quality").value==="best";
         $("quality").value="720";$("quality").onchange();const detail=$("preset").value==="custom"&&$("quality").value==="720";
        $("preset").value="editor";$("preset").onchange();const back=$("format").value==="editor"&&$("quality").value==="1080"&&!$("quality").disabled;
        const consent=confirmSetup();const modal=$("setupConfirm").open;$("setupNo").click();const refused=!(await consent)&&!$("setupConfirm").open;
        workflowOk=auto&&manual&&reset&&disabled&&opened&&closed&&simple&&detail&&back&&modal&&refused&&!$("hide")&&!$("organize");
    }catch(e){notice(e,true);}
    let uxOk=false;
    try{
        const before=$("theme").value;
        setLanguage("tr");const localized=document.querySelector('label[for="url"]').textContent.includes("\u011flant\u0131")&&$("theme").value===before&&document.documentElement.lang==="tr";
        setLanguage("en");const backToEnglish=document.querySelector('label[for="url"]').textContent.includes("Media link");
        await saveSection({language:"tr",open_hotkey:"Off",shortcuts:{...defaultShortcuts,mp3:"Ctrl+Shift+8"}});const {settings}=await invoke("snapshot");
        const persisted=settings.prefs.language==="tr"&&settings.prefs.open_hotkey==="Off"&&settings.prefs.shortcuts.mp3==="Ctrl+Shift+8";
        await saveSection({language:"en",open_hotkey:"Ctrl+Alt+Space",shortcuts:defaultShortcuts});
        previewMeta={title:"Fixture",playlist:false,sizes:[{preset:"editor",quality:"1080",bytes:10485760},{preset:"editor",quality:"720",bytes:5242880}]};
        $("format").value="editor";$("quality").value="1080";updateSize();const big=$("sizeEstimate").textContent.includes("10.0 MiB");
        $("quality").value="720";updateSize();const small=$("sizeEstimate").textContent.includes("5.0 MiB");
        previewMeta=null;updateSize();$("quality").value="1080";
        await $("quickTrim").onclick();const trim=expanded&&current==="home";if(expanded)await toggleExpanded(false);
        await toggleExpanded(true);show("shortcuts");const shortcutRows=$("shortcutList").children.length===8;
        await saveSection({always_on_top:false});const topOff=(await invoke("snapshot")).settings.prefs.always_on_top===false;
        await saveSection({always_on_top:true});show("home");await toggleExpanded(false);
        $("url").value="https://example.com/shortcut-test";$("url").dispatchEvent(new Event("input"));
        applySubmittedLinks([["fixture","https://example.com/shortcut-test"]]);const cleared=!$("url").value&&$("thumbnail").hidden;
        $("url").value="https://example.com/new-draft";$("url").dispatchEvent(new Event("input"));
        applySubmittedLinks([["fixture","https://example.com/shortcut-test"]]);const draftKept=$("url").value==="https://example.com/new-draft";$("clearLink").click();
        uxOk=localized&&backToEnglish&&persisted&&big&&small&&trim&&shortcutRows&&topOff&&cleared&&draftKept&&Boolean($("openDownloads"));
    }catch(e){notice(e,true);}
    await invoke("ui_ready",{passed:uxOk&&persistOk&&clipOk&&workflowOk&&playlistOk&&materialOk&&configOk&&hidden&&visible&&document.querySelectorAll(".info").length>=15&&initialized&&Boolean($("form")&&$("queueList")&&$("historyList")&&$("settingsForm")),themePassed:themeSmoke(),clipboardPassed:clipboardOk});
    const visual=await invoke("visual_page");const [page,palette,finish]=visual.split(":");if(visual){$("destination").value="C:\\Downloads\\QuickClip321";}if(["settings","shortcuts","profiles","organizer","diagnostics","queue"].includes(page)){await toggleExpanded(true);show(page);}else if(page==="quick"){$("previewText").textContent="Ready to download";}if(palette)$("theme").value=palette;if(finish)$("material").value=finish;applyTheme();if(page==="custom"){$("preset").value="custom";updateFormat();}if(page==="playlist"){$("url").value="https://www.youtube.com/playlist?list=PLfixture";setPlaylistVisibility(true,12);$("previewText").textContent="Playlist detected";}
}
refresh().then(smokeChecks).catch(e=>notice(e,true));setInterval(()=>{if(document.hasFocus())refresh();},1000);
