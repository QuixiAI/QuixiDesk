/* Copyright (c) 2026 QuixiAI. MIT license. */
(() => {
  "use strict";
  window.QuixiWorkspace = {
    create(c) {
      const {state,el,button,ib,icon,field,select,dialog,formDialog,toast,rpc,api,copy,navigate,header,empty,date} = c;
      const checked = value => { if(value?.error)throw Error(value.error); if(value===false||value===null)throw Error("The change could not be saved. Please retry."); return value; };
      const textMatch = (item, query, fields) => fields.some(k => String(item[k]||"").toLocaleLowerCase().includes(query));
      function startProject(p) { c.saveDraft(); state.project=p.id;state.folder=p.folders?.[0]?.path||"";navigate("new"); }
      function projectPage(host) {
        const p=state.projects.find(p=>p.id===state.id);
        if(!p){host.replaceChildren(empty("Project unavailable","Return to Projects and choose a project.",button("Projects",()=>navigate("projects"))));return;}
        const sessions=state.sessions.filter(s=>s.spaceId===p.id&&!s.isArchived);
        const filter=el("input",{type:"search",placeholder:"Find a conversation in this project","aria-label":"Search project conversations"});
        const rows=el("div",{class:"list project-conversations"});
        const renderRows=()=>{
          const q=filter.value.toLocaleLowerCase();const found=sessions.filter(s=>textMatch(s,q,["title"]));
          rows.replaceChildren(...found.map(s=>el("div",{class:"list-row"},icon(s.isStarred?"star":"file"),
            el("div",{class:"grow"},button(s.title||"Untitled",()=>navigate("chat",s.sessionId),"subtle-link"),el("small",{},s.isRunning?"Working…":date(s.lastActivityAt))),ib(`Delete conversation: ${s.title||"Untitled"}`,"trash",()=>c.deleteSession(s)),ib("Task actions","more",()=>c.sessionMenu(s)))));
          if(!found.length)rows.append(empty(q?"No conversations match":"Start something here",q?"Try a different title.":"New conversations inherit this project’s instructions and chosen folder.",button("Start task",()=>startProject(p),"primary","plus")));
        };
        filter.addEventListener("input",renderRows);
        const sidebar=el("aside",{class:"project-context card"},el("h3",{},"Project context"),
          el("p",{},p.description||"Keep related conversations and reference material together."),
          button("Edit project",()=>c.projectEditor(p),"quiet","settings"),el("hr"),
          el("h3",{},"Instructions"),el("p",{class:"project-instructions"},p.instructions||"No instructions yet. Add a goal, conventions, or background for every task."),
          el("h3",{},"Folders"),...(p.folders||[]).map(f=>el("p",{class:"path"},icon("folder",14),f.path)),
          !(p.folders?.length)?el("p",{class:"hint"},"No project folder selected. Choose a folder when a task needs local files."):null,
          button("Manage folders",()=>c.manageProjectFolders(p),"quiet","folder"),
          button("Delete project",()=>c.deleteProject(p),"quiet danger","trash"));
        const related=el("div",{class:"project-related"});
        const updateRelated=()=>{
          if(!host.isConnected||state.page!=="project"||state.id!==p.id)return;
          const ids=new Set(state.sessions.filter(s=>s.spaceId===p.id).map(s=>s.sessionId));
          const artifacts=state.artifacts.filter(a=>ids.has(a.createdBySessionId));
          const schedules=state.schedules.filter(t=>t.spaceId===p.id&&t.status!=="deleted");
          related.replaceChildren(el("h3",{},"Artifacts & schedules"),
            ...artifacts.map(a=>button(a.name||a.title,()=>openArtifact(a.id),"quiet","artifacts")),
            ...schedules.map(t=>button(t.displayName||t.name,()=>c.scheduleEditor(t),"quiet","schedule")),
            button("Add schedule",()=>c.scheduleEditor(null),"quiet","plus"));
        };
        host.replaceChildren(header(p.name,p.description||"A home for your ongoing work.",button("Start task",()=>startProject(p),"primary","plus")),
          el("div",{class:"project-layout"},el("section",{},el("div",{class:"toolbar"},filter),rows,related),sidebar));
        renderRows();updateRelated();Promise.all([c.loadArtifacts(),c.loadSchedules()]).then(updateRelated).catch(e=>toast(e.message,true));
      }
      function projectsPage(host) {
        const input=el("input",{type:"search",placeholder:"Find a project","aria-label":"Search projects"});
        const sort=select([["recent","Recently updated"],["name","Name"]],"recent",()=>draw(),{"aria-label":"Sort projects"});
        const cards=el("div",{class:"cards"});
        function draw(){const q=input.value.toLocaleLowerCase();const list=state.projects.filter(p=>textMatch(p,q,["name","description"])).sort((a,b)=>sort.value==="name"?a.name.localeCompare(b.name):(b.updatedAt||0)-(a.updatedAt||0));
          cards.replaceChildren(...list.map(p=>el("article",{class:"card project-card"},el("div",{class:"card-symbol"},icon("projects",21)),el("h3",{},p.name),el("p",{},p.description||"A place for related work."),
            el("small",{},`${p.folders?.length||0} folders · ${state.sessions.filter(s=>s.spaceId===p.id&&!s.isArchived).length} conversations`),
            el("div",{class:"card-actions"},button("Open project",()=>navigate("project",p.id)),button("Edit",()=>c.projectEditor(p),"quiet"),ib(`Delete project: ${p.name}`,"trash",()=>c.deleteProject(p))))));
          if(!list.length)cards.append(empty("No projects found",q?"Try another name.":"Create a project to keep instructions and conversations together."));}
        input.addEventListener("input",draw);host.replaceChildren(header("Projects","One place for each ongoing effort.",button("New project",()=>c.projectEditor(),"primary","plus")),el("div",{class:"toolbar"},input,sort),cards);draw();
      }
      async function openArtifact(id) {
        const a=checked(await api("/api/quixi/artifacts/"+encodeURIComponent(id)));
        const bridgeScript=`<script>(()=>{const post=(t,d)=>{try{parent.postMessage({type:t,...d},"*");}catch{}};window.quixi=Object.freeze({ask(prompt){post("quixi:artifact-ask",{prompt:String(prompt??"")});}});for(const lvl of["log","info","warn","error"]){const orig=console[lvl]?.bind(console);console[lvl]=(...args)=>{post("quixi:artifact-console",{level:lvl,text:args.map(x=>{try{return typeof x==="string"?x:JSON.stringify(x);}catch{return String(x);}}).join(" ")});orig?.(...args);};}window.addEventListener("error",e=>post("quixi:artifact-console",{level:"error",text:String(e.message||e.error||"Runtime error")}));window.addEventListener("unhandledrejection",e=>post("quixi:artifact-console",{level:"error",text:"Unhandled rejection: "+String(e.reason?.message||e.reason||"unknown")}));window.addEventListener("message",e=>{if(e.data?.type==="quixi:artifact-theme"){const t=e.data.theme;if(t==="light"||t==="dark"){document.documentElement.dataset.theme=t;document.documentElement.style.colorScheme=t;}else{delete document.documentElement.dataset.theme;document.documentElement.style.colorScheme="";}}});})();</script>`;
        const frame=el("iframe",{class:"preview-frame",sandbox:"allow-scripts allow-forms",referrerpolicy:"no-referrer",title:a.name||a.title||"Artifact preview"});
        const buildSrcdoc=()=>bridgeScript+(a.html||"");
        frame.srcdoc=buildSrcdoc();
        const frameStage=el("div",{class:"artifact-stage",dataset:{viewport:"desktop",surface:"auto"}},frame);
        const source=el("pre",{class:"code-preview artifact-source",hidden:true},a.html||"");
        const consoleList=el("div",{class:"artifact-console-list"},el("p",{class:"hint"},"No console output or runtime errors recorded."));
        const consolePane=el("div",{class:"artifact-console-pane",hidden:true});
        const consoleEntries=[];
        const status=el("p",{class:"hint",role:"status"},`Revision ${a.revision||1} · Saved ${date(a.updatedAt)}`);
        let busy=false,isPinned=Boolean(a.isStarred);
        const run=async(fn)=>{if(busy)return;busy=true;try{await fn();}catch(e){status.textContent=e.message;status.className="error-text";}finally{busy=false;}};
        const continueWithPrompt=async(seedPrompt)=>{
          await c.loadSessions();
          let s=state.sessions.find(s=>s.sessionId===a.createdBySessionId);
          d.close();
          if(s)navigate("chat",s.sessionId);else{state.project=null;state.folder="";navigate("new");}
          const prompt=document.getElementById("prompt");
          if(prompt){
            prompt.value=seedPrompt||`I want to improve the artifact “${a.name||a.title}” (id: ${a.id}). `;
            if(!s)state.attachments.push({name:(a.name||"artifact")+".html",text:a.html||""});
            c.saveDraft();c.renderChips();prompt.focus();
          }
        };
        const pinBtn=button(isPinned?"Pinned":"Pin",()=>run(async()=>{isPinned=!isPinned;checked(await rpc("CoworkArtifacts","setArtifactStarred",[id,isPinned]));pinBtn.replaceChildren(icon("star"),isPinned?"Pinned":"Pin");pinBtn.classList.toggle("active",isPinned);await c.loadArtifacts();}),"quiet"+(isPinned?" active":""),"star");
        pinBtn.setAttribute("aria-label","Toggle pin artifact");
        const reloadBtn=button("Reload",()=>{consoleEntries.length=0;renderConsole();frame.srcdoc=buildSrcdoc();},"quiet","refresh");
        const actions=el("div",{class:"artifact-actions"},
          button("Export HTML",()=>run(async()=>{const folder=await rpc("FileSystem","browseFolder",["Export artifact to folder"]);if(!folder)return;const result=checked(await rpc("CoworkArtifacts","exportArtifactHtml",[id,folder]));status.textContent="Saved to "+result.path;status.className="hint";toast("Artifact exported.");}),"primary"),
          button("Copy HTML",()=>run(async()=>{await copy(a.html||"");toast("HTML copied.");}),"quiet"),
          button("Print / PDF",()=>run(async()=>{const w=window.open("","_blank","noopener,noreferrer");if(w){w.document.open();w.document.write(a.html||"");w.document.close();w.focus();w.print();}else if(frame.contentWindow?.print){frame.contentWindow.print();}}),"quiet"),
          reloadBtn,
          pinBtn,
          button("Duplicate",()=>run(async()=>{const created=checked(await rpc("CoworkArtifacts","duplicateArtifact",[id]));await c.loadArtifacts();d.close();await openArtifact(created.id);}),"quiet"),
          button("Continue in chat",()=>run(()=>continueWithPrompt()),"quiet","arrow"));
        let viewMode="preview",vp="desktop",surfaceTheme="auto";
        const previewBtn=button("Preview",()=>{viewMode="preview";frameStage.hidden=false;source.hidden=true;consolePane.hidden=true;syncToolbar();},"quiet active");
        const sourceBtn=button("Source",()=>{viewMode="source";frameStage.hidden=true;source.hidden=false;consolePane.hidden=true;syncToolbar();},"quiet");
        const consoleBtn=button("Console",()=>{viewMode="console";frameStage.hidden=true;source.hidden=true;consolePane.hidden=false;syncToolbar();},"quiet");
        consoleBtn.setAttribute("aria-label","Artifact console");
        function renderConsole(){
          const errs=consoleEntries.filter(e=>e.level==="error");
          consoleBtn.textContent=errs.length?`Console (${errs.length} err)`:consoleEntries.length?`Console (${consoleEntries.length})`:"Console";
          consoleBtn.classList.toggle("has-error",errs.length>0);
          if(!consoleEntries.length){
            consoleList.replaceChildren(el("p",{class:"hint"},"No console output or runtime errors recorded."));
          }else{
            consoleList.replaceChildren(...consoleEntries.map(item=>el("div",{class:"artifact-console-row is-"+item.level},el("span",{class:"cordis-badge"},item.level),el("code",{},item.text))));
          }
          const fixBar=errs.length?el("div",{class:"toolbar"},button("Fix error in chat",()=>continueWithPrompt(`Please fix this runtime error in artifact “${a.name||a.title}” (id: ${a.id}):\n${errs.map(e=>e.text).join("\n")}`),"primary"),button("Clear console",()=>{consoleEntries.length=0;renderConsole();},"quiet")):el("div",{class:"toolbar"},button("Clear console",()=>{consoleEntries.length=0;renderConsole();},"quiet"));
          consolePane.replaceChildren(fixBar,consoleList);
        }
        renderConsole();
        const vpBtns=[["desktop","Desktop"],["tablet","Tablet"],["mobile","Mobile"]].map(([k,lbl])=>{
          const b=button(lbl,()=>{vp=k;frameStage.dataset.viewport=k;syncToolbar();},"quiet"+(k==="desktop"?" active":""));
          b.dataset.vp=k;
          return b;
        });
        const vpGroup=el("div",{class:"cordis-segmented artifact-vp-switch"},...vpBtns);
        const surfBtns=[["auto","Auto"],["light","Light"],["dark","Dark"]].map(([k,lbl])=>{
          const b=button(lbl,()=>{surfaceTheme=k;frameStage.dataset.surface=k;try{frame.contentWindow?.postMessage({type:"quixi:artifact-theme",theme:k},"*");}catch{}syncToolbar();},"quiet"+(k==="auto"?" active":""));
          b.dataset.surface=k;
          return b;
        });
        const surfGroup=el("div",{class:"cordis-segmented artifact-surface-switch","aria-label":"Artifact preview theme"},...surfBtns);
        function syncToolbar(){
          previewBtn.classList.toggle("active",viewMode==="preview");
          sourceBtn.classList.toggle("active",viewMode==="source");
          consoleBtn.classList.toggle("active",viewMode==="console");
          vpGroup.hidden=viewMode!=="preview";
          surfGroup.hidden=viewMode!=="preview";
          for(const b of vpBtns)b.classList.toggle("active",b.dataset.vp===vp);
          for(const b of surfBtns)b.classList.toggle("active",b.dataset.surface===surfaceTheme);
        }
        const tabs=el("div",{class:"toolbar artifact-studio-bar"},el("div",{class:"cordis-segmented"},previewBtn,sourceBtn,consoleBtn),vpGroup,surfGroup);
        if(a.versions?.length){const revisions=select([["","Earlier revisions…"],...a.versions.slice().reverse().map(v=>[String(v.revision),`Revision ${v.revision} · ${date(v.savedAt)}`])],"",null,{"aria-label":"Artifact revision"});tabs.append(revisions,button("Restore revision",()=>run(async()=>{if(!revisions.value)throw Error("Choose a revision first.");checked(await rpc("CoworkArtifacts","restoreArtifactVersion",[id,Number(revisions.value)]));await c.loadArtifacts();d.close();openArtifact(id);}),"quiet"));}
        const d=dialog(a.name||a.title||"Artifact",[actions,status,tabs,frameStage,source,consolePane],{wide:true});
        const onMsg=(ev)=>{
          if(ev.source&&frame.contentWindow&&ev.source!==frame.contentWindow)return;
          if(ev.data?.type==="quixi:artifact-ask"){
            const q=String(ev.data.prompt||"").trim().slice(0,4000);
            if(q)void continueWithPrompt(`Regarding artifact “${a.name||a.title}” (id: ${a.id}): ${q}`);
          }else if(ev.data?.type==="quixi:artifact-console"){
            if(consoleEntries.length<200){
              consoleEntries.push({level:String(ev.data.level||"log"),text:String(ev.data.text||"").slice(0,2000)});
              renderConsole();
            }
          }
        };
        window.addEventListener("message",onMsg);
        d.addEventListener("close",()=>window.removeEventListener("message",onMsg),{once:true});
        syncToolbar();
      }
      function artifactsPage(host){
        const input=el("input",{type:"search",placeholder:"Find an artifact","aria-label":"Search artifacts"});
        const sort=select([["recent","Recently updated"],["name","Name"],["starred","Pinned"]],"recent",()=>draw(),{"aria-label":"Sort artifacts"});
        const list=el("div",{class:"cards"});
        function draw(){const q=input.value.toLocaleLowerCase();const artifacts=state.artifacts.filter(a=>textMatch(a,q,["name","title","description"])&&(sort.value!=="starred"||a.isStarred)).sort((a,b)=>sort.value==="name"?String(a.name||a.title).localeCompare(b.name||b.title):(b.updatedAt||0)-(a.updatedAt||0));
          list.replaceChildren(...artifacts.map(a=>el("article",{class:"card artifact-card"},c.artifactThumbnail(a),el("h3",{},a.name||a.title),el("small",{},`Revision ${a.revision||1} · ${date(a.updatedAt)}`),el("div",{class:"card-actions"},button("Open preview",()=>openArtifact(a.id)),ib("Artifact actions","more",()=>artifactMenu(a))))));
          if(!artifacts.length)list.append(empty("Your ideas, made tangible",q?"No artifacts match this search.":"Build a useful page or tool in a conversation, then export or improve it here."));}
        input.addEventListener("input",draw);host.replaceChildren(header("Artifacts","Create, revisit, and take your work with you.",button("New artifact",()=>{navigate("new");document.getElementById("prompt").value="Help me create an interactive HTML artifact. Ask what I would like to build.";c.saveDraft();},"primary","plus")),el("div",{class:"toolbar"},input,sort),list);draw();
      }
      function artifactMenu(a){const d=dialog(a.name||a.title,[button(a.isStarred?"Unpin":"Pin",async()=>{checked(await rpc("CoworkArtifacts","setArtifactStarred",[a.id,!a.isStarred]));d.close();await c.loadArtifacts();},"quiet","star"),
        button("Rename",()=>{d.close();formDialog("Rename artifact",[{name:"name",label:"Name",value:a.name||a.title,required:true}],async v=>{if(!v.name.trim())throw Error("Enter a name.");checked(await rpc("CoworkArtifacts","setArtifactName",[a.id,v.name.trim()]));await c.loadArtifacts();});},"quiet"),
        button("Delete",()=>{d.close();c.confirmDialog("Delete artifact?",`Remove “${a.name||a.title}” and its saved revisions?`,async()=>{checked(await rpc("CoworkArtifacts","deleteArtifact",[a.id]));await c.loadArtifacts();});},"quiet danger")]);}
      function searchPage(host){
        const input=el("input",{id:"search-input",type:"search",placeholder:"Search conversations, projects, artifacts, and schedules","aria-label":"Search your work"});
        const kind=select([["all","Everything"],["conversation","Conversations"],["project","Projects"],["artifact","Artifacts"],["schedule","Schedules"]],"all",()=>search(),{"aria-label":"Search category"});
        const scope=select([["active","Active"],["all","Include archived"],["pinned","Pinned conversations"]],"active",()=>search(),{"aria-label":"Search scope"});
        const results=el("div",{class:"list",id:"search-results"});const status=el("p",{class:"result-count",role:"status"});let generation=0,timer;
        function highlighted(text,q){const span=el("span");const raw=String(text||"");const idx=raw.toLocaleLowerCase().indexOf(q.toLocaleLowerCase());if(!q||idx<0)span.textContent=raw;else span.append(raw.slice(0,idx),el("mark",{},raw.slice(idx,idx+q.length)),raw.slice(idx+q.length));return span;}
        async function search(){const ticket=++generation,q=input.value.trim();status.textContent="Searching…";try{
          const found=q?await rpc("LocalAgentModeSessions","searchSessions",[q,100]):[];
          if(ticket!==generation||!host.isConnected)return;
          const low=q.toLocaleLowerCase();let rows=[];
          for(const s of state.sessions){const hit=(found||[]).find(f=>f.sessionId===s.sessionId);if((!q||hit||textMatch(s,low,["title"]))&&(scope.value==="all"||!s.isArchived)&&(scope.value!=="pinned"||s.isStarred))rows.push({kind:"conversation",remove:()=>c.deleteSession(s),title:s.title||"Untitled",detail:hit?.snippet||date(s.lastActivityAt),icon:"file",open:()=>navigate("chat",s.sessionId),time:s.lastActivityAt});}
          if(scope.value!=="pinned"){
            for(const p of state.projects)if(!q||textMatch(p,low,["name","description","instructions"]))rows.push({kind:"project",remove:()=>c.deleteProject(p),title:p.name,detail:p.description||"Project",icon:"projects",open:()=>navigate("project",p.id),time:p.updatedAt});
            for(const a of state.artifacts)if(!q||textMatch(a,low,["name","title","description"]))rows.push({kind:"artifact",title:a.name||a.title,detail:"Artifact · Revision "+(a.revision||1),icon:"artifacts",open:()=>openArtifact(a.id),time:a.updatedAt});
            for(const t of state.schedules)if(t.status!=="deleted"&&(!q||textMatch(t,low,["displayName","name","prompt"])))rows.push({kind:"schedule",title:t.displayName||t.name,detail:t.enabled?"Scheduled task":"Paused task",icon:"schedule",open:()=>{navigate("schedules");c.scheduleEditor(t);},time:t.updatedAt});
          }
          rows=rows.filter(r=>kind.value==="all"||r.kind===kind.value).sort((a,b)=>(b.time||0)-(a.time||0));
          status.textContent=`${rows.length} result${rows.length===1?"":"s"}`;
          results.replaceChildren(...rows.map(r=>el("div",{class:"list-row search-result"},icon(r.icon),el("div",{class:"grow"},button(highlighted(r.title,q),r.open,"subtle-link"),el("p",{class:"snippet"},highlighted(r.detail,q))),el("small",{},r.kind),r.remove?ib(`Delete ${r.kind}: ${r.title}`,"trash",r.remove):null)));
          if(!rows.length)results.append(empty("No matches","Try a different phrase, category, or archive filter."));
        }catch(e){if(ticket===generation){status.textContent="Search failed";results.replaceChildren(el("p",{class:"error-text"},e.message),button("Try again",search));}}}
        input.addEventListener("input",()=>{++generation;clearTimeout(timer);timer=setTimeout(search,250);});
        input.addEventListener("keydown",e=>{if(e.key==="ArrowDown"){e.preventDefault();results.querySelector("button")?.focus();}});
        host.replaceChildren(header("Find your work","Search across your workspace. ⌘/Ctrl K to return here."),el("div",{class:"toolbar"},input,kind,scope),status,results);
        Promise.all([c.loadProjects(),c.loadArtifacts(),c.loadSchedules()]).then(search).catch(e=>{status.textContent=e.message;});search();
      }
      return {projectPage,projectsPage,openArtifact,artifactsPage,searchPage};
    }
  };
})();
