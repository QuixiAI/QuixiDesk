const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const vm=require('node:vm');
const {webcrypto}=require('node:crypto');
function fixture() {
  const window={};
  vm.runInNewContext(fs.readFileSync(require('node:path').join(__dirname,'../../resources/ui/chat-workflows.js'),'utf8'),{window,crypto:webcrypto,queueMicrotask,TextDecoder,TextEncoder,Uint8Array,btoa});
  return window.QuixiChat;
}
const settle=()=>new Promise(resolve=>setTimeout(resolve,0));
const file=(name,type,bytes)=>({name,type,size:bytes.length,arrayBuffer:async()=>Uint8Array.from(bytes).buffer});
test('image bytes survive the queue and serialization instead of becoming text',async()=>{
  const api=fixture(),bytes=Buffer.from('89504e470d0a1a0a010203','hex');
  const items=api.queueFiles([file('photo.png','image/png',bytes)]);
  assert.throws(()=>api.attachmentsForSend(items),/finish loading/);
  await settle();
  const result=api.attachmentsForSend(items);
  assert.equal(result[0].mimeType,'image/png');assert.equal(result[0].data,bytes.toString('base64'));assert.equal(result[0].text,undefined);
  assert.match(items[0].preview,/^data:image\/png;base64,/);
});
test('a failed file remains visible and blocks send until retried or removed',async()=>{
  const api=fixture(),items=api.queueFiles([file('report.pdf','application/pdf',Buffer.from('%PDF-1.7')),file('notes.txt','text/plain',Buffer.from('Keep me'))]);
  await settle();assert.equal(items[0].status,'error');assert.equal(items[1].status,'ready');
  assert.throws(()=>api.attachmentsForSend(items),/report.pdf/);
  assert.equal(api.attachmentsForSend(items.slice(1))[0].text,'Keep me');
  items[0].file=file('report.pdf','text/plain',Buffer.from('Replaced contents'));
  await api.retryAttachment(items[0]);assert.equal(items[0].status,'ready');
});
test('cancelled file reads cannot reappear after removal or overwrite another draft',async()=>{
  const api=fixture();let finish;const items=api.queueFiles([{name:'slow.txt',type:'text/plain',size:4,arrayBuffer:()=>new Promise(resolve=>{finish=resolve;})}]);
  await settle();api.cancelAttachment(items[0]);finish(new TextEncoder().encode('late').buffer);await settle();
  assert.equal(items[0].status,'cancelled');assert.equal(items[0].text,undefined);
});
test('invalid UTF-8 and excessive counts are rejected without losing other attachments',async()=>{
  const api=fixture(),items=api.queueFiles([file('binary.txt','text/plain',Uint8Array.from([0xff,0xfe,0xff]))]);await settle();
  assert.match(items[0].error,/UTF-8/);
  assert.throws(()=>api.attachmentsForSend(Array.from({length:9},()=>({name:'a.txt',status:'ready',text:'ok',size:2}))),/up to 8/);
});
test('editing excludes attachment payloads while transcript previews retain filenames',()=>{
  const api=fixture(),message={message:{content:[{type:'text',text:'Review this'},{type:'text',text:'Attached file: notes.txt\nsecret body',attachment:{name:'notes.txt',mimeType:'text/plain'}}]}};
  assert.equal(api.editableText(message),'Review this');assert.equal(api.transcriptAttachments(message)[0].name,'notes.txt');
});
