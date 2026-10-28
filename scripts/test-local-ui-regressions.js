#!/usr/bin/env node
// Exercise the shipped renderer functions, including patches applied during preparation.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const assets = path.join(__dirname, '../resources/ion-dist/assets/v1');
function load(file, name, next, globals = {}) {
  const text = fs.readFileSync(path.join(assets, file), 'utf8');
  const start = text.indexOf(`function ${name}(`);
  const end = text.indexOf(next, start);
  assert.ok(start >= 0 && end > start, `${name} extraction anchor`);
  return vm.runInNewContext(`(${text.slice(start, end)})`, globals);
}
const infer = load('shared-1-BJhqMmkf.js', 'up', 'function dp(', {of: () => false, lp: () => null});
const projects = [{id:'qa', folders:[{path:'/tmp/café'}]}, {id:'main', folders:[{path:'/work/main'}]}];
assert.equal(infer(projects, ['/work/main','/tmp/café']).id, 'main');
assert.equal(infer(projects, ['/tmp/café']).id, 'qa');
assert.equal(infer(projects, ['/unknown']), undefined);
const filter = load('ce8be540f-CkdtaDOw.js', 'Je', 'var Ye=', {Ne: () => false});
const query = {owner:null, active:null, type:'chat', repo:null, project:null, archived:null};
assert.equal(filter({kind:'task',shared:false}, 0, query, 0), true);
assert.equal(filter({kind:'chat',shared:false}, 0, query, 0), true);
assert.equal(filter({kind:'project',shared:false}, 0, query, 0), false);
const summary = load('c05b96c7f-MPVkefMW.js', 'B_', 'function V_(');
const tools = [{type:'tool_use',name:'Write',id:'write'}, {type:'tool_use',name:'Edit',id:'edit'}];
const intl = {formatMessage: (_message, args) => `${args.count} tools failed`};
assert.equal(summary(intl, tools, {getToolResult: () => ({is_error:true})}), '2 tools failed');
console.log('Renderer regressions passed: selected root order, Sessions type filtering, truthful failed-tool summaries.');
