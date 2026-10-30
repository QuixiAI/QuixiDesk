const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {JSDOM}=require('jsdom');
const source=fs.readFileSync(path.join(__dirname,'../../resources/ui/schedule-workflows.js'),'utf8');
function fixture({task=null, saveError=null, preview=async()=>({runs:[Date.now()+60000,Date.now()+120000,Date.now()+180000],timezone:'Local'})}={}) {
 const dom=new JSDOM('<body></body>',{runScripts:'outside-only',url:'https://app.localhost'}), w=dom.window,calls=[];
 const el=(tag,attrs={},...children)=>{const n=w.document.createElement(tag);for(const[k,v]of Object.entries(attrs)){if(v==null||v===false)continue;if(k.startsWith('on'))n.addEventListener(k.slice(2).toLowerCase(),v);else if(k==='class')n.className=v;else if(k==='value')n.value=v;else if(k==='selected')n.selected=v;else n.setAttribute(k,v===true?'':v);}for(const c of children.flat(Infinity)){if(c!=null)n.append(c instanceof w.Node?c:String(c));}return n;};
 const button=(text,fn,cls)=>el('button',{type:'button',class:cls,onClick:fn},text);
 const select=(rows,value,fn,attrs={})=>{const n=el('select',{...attrs,onChange:e=>fn?.(e.target.value)});for(const[v,label]of rows)n.append(el('option',{value:v,selected:v===value},label));return n;};
 const field=(text,n,hint)=>{n.id='f'+Math.random();return el('div',{},el('label',{for:n.id},text),n,hint?el('small',{},hint):null);};
 const dialog=(title,body)=>{const d=el('dialog',{'aria-label':title},el('div',{class:'dialog-body'},body));d.close=()=>{d.dispatchEvent(new w.Event('close'));d.remove();};w.document.body.append(d);return d;};
 const ctx={el,button,select,field,dialog,state:{project:null,projects:[{id:'p',name:'Club',folders:[{path:'/club'}]}],providers:{activeProvider:'custom',providers:[{id:'custom',name:'Custom',models:[{id:'a',label:'Model A'}]}]}},currentFolder:()=>'',loadSchedules:async()=>calls.push({member:'reload'}),api:async(url,opts)=>({exists:true,path:opts.path}),rpc:async(service,member,args)=>{calls.push({service,member,args});if(member==='previewSchedule')return preview(args[0]);if(member==='browseFolder')return '/chosen';if(member==='createScheduledTask'||member==='updateScheduledTask')return saveError?{error:saveError}:{id:'saved'};}};
 w.eval(source);const d=w.QuixiSchedules.createEditor(ctx,task);
 const control=label=>w.document.getElementById([...d.querySelectorAll('label')].find(n=>n.textContent===label)?.htmlFor);
 const click=text=>[...d.querySelectorAll('button')].find(n=>n.textContent===text).click();
 const change=(label,value)=>{const n=control(label);n.value=value;n.dispatchEvent(new w.Event(n.tagName==='SELECT'?'change':'input',{bubbles:true}));};
 return{w,d,calls,control,click,change,close:()=>w.close()};
}
const settle=()=>new Promise(r=>setTimeout(r,250));
test('weekday schedule submits explicit context and selected model and permissions',async()=>{
 const f=fixture();try{f.change('Name','Club briefing');f.change('Instructions','Review the club plan');f.change('Frequency','weekdays');f.change('Time','08:30');f.change('Project','p');f.change('Model','a');f.change('Permissions','auto');f.click('Choose folder');await settle();f.click('Create schedule');await settle();const save=f.calls.find(c=>c.member==='createScheduledTask');assert.equal(save.args[0].cronExpression,'30 8 * * 1-5');assert.equal(save.args[0].cwd,'/chosen');assert.equal(save.args[0].spaceId,'p');assert.equal(save.args[0].model,'a');assert.equal(save.args[0].permissionMode,'auto');assert.equal(f.w.document.querySelector('dialog'),null);}finally{f.close();}
});
test('failed save preserves the draft and does not invent a workspace',async()=>{
 const f=fixture({saveError:'Disk is full'});try{f.change('Name','Daily note');f.change('Instructions','A thoughtful question');await settle();f.click('Create schedule');await settle();const save=f.calls.find(c=>c.member==='createScheduledTask');assert.equal(save.args[0].cwd,'');assert.equal(save.args[0].spaceId,null);assert.equal(f.control('Instructions').value,'A thoughtful question');assert.ok(f.d.isConnected);assert.match(f.d.querySelector('[role=alert]').textContent,/Disk is full/);}finally{f.close();}
});
test('editing custom cron preserves the original expression and paused state',async()=>{
 const f=fixture({task:{id:'s',displayName:'Quarterly',prompt:'Review',cronExpression:'0 9 1 */3 *',enabled:false,cwd:'',permissionMode:'default'}});try{await settle();assert.equal(f.control('Frequency').value,'custom');assert.equal(f.control('Cron expression').value,'0 9 1 */3 *');f.click('Save changes');await settle();const save=f.calls.find(c=>c.member==='updateScheduledTask');assert.equal(save.args[0].cronExpression,'0 9 1 */3 *');assert.equal(save.args[0].enabled,false);}finally{f.close();}
});
test('invalid cron blocks saving and reports the engine validation error',async()=>{
 const f=fixture({preview:async cron=>cron==='broken'?{error:'Invalid cron expression'}:{runs:[Date.now()+60000]}});try{f.change('Name','Test');f.change('Instructions','Prompt');f.change('Frequency','custom');f.change('Cron expression','broken');await settle();f.click('Create schedule');await settle();assert.equal(f.calls.some(c=>c.member==='createScheduledTask'),false);assert.match(f.d.querySelector('[role=alert]').textContent,/Invalid cron/);}finally{f.close();}
});
test('obsolete preview responses cannot replace the current schedule preview',async()=>{
 let release;let first=true;const f=fixture({preview:async()=>{if(first){first=false;return new Promise(resolve=>{release=resolve;});}return{runs:[Date.now()+60000],timezone:'Current'};}});try{await settle();f.change('Frequency','weekdays');await settle();assert.match(f.d.querySelector('[role=status]').textContent,/Current/);release({runs:[Date.now()+120000],timezone:'Obsolete'});await settle();assert.doesNotMatch(f.d.querySelector('[role=status]').textContent,/Obsolete/);}finally{f.close();}
});
test('optional context is collapsed with a live summary and actions outside the scrolling body',async()=>{
 const f=fixture();try{const details=f.d.querySelector('details');assert.equal(details.open,false);assert.match(details.querySelector('summary').textContent,/No project · No folder · Manual/);assert.ok(f.d.querySelector(':scope > .schedule-editor-actions'));details.open=true;f.change('Project','p');f.change('Permissions','auto');assert.match(details.querySelector('summary').textContent,/Club · club · Auto/);f.click('Clear');assert.match(details.querySelector('summary').textContent,/Club · No folder · Auto/);await settle();assert.equal(f.d.querySelectorAll('.schedule-preview li').length,2);}finally{f.close();}
});
