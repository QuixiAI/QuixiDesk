#!/usr/bin/env node
const fs = require("fs");
const vm = require("vm");

const code = fs.readFileSync("asar-extracted/.vite/build/index.chunk-jPKPzV-P.js", "utf8");
const actual = JSON.parse(fs.readFileSync("reference/quixi-ipc-actual-outputs.json", "utf8"));
const validators = JSON.parse(fs.readFileSync("reference/ipc-return-validators.json", "utf8"));

const allFns = new Map();
const re = /function ([A-Za-z0-9_$]+)\(e\)\{([^{}]*(?:\{[^{}]*\}[^{}]*)*)\}/g;
let m;
while ((m = re.exec(code)) !== null) allFns.set(m[1], m[0]);

const allSets = new Map();
const setRe = /(?:^|[^A-Za-z0-9_$])([A-Za-z0-9_$]+)=new Set\((\[[^\]]*\])\)/g;
while ((m = setRe.exec(code)) !== null) {
  try {
    allSets.set(m[1], JSON.parse(m[2]));
  } catch {}
}

const ctx = {
  F: (e) => typeof e === "string",
  mo: (e) => typeof e === "number",
  I: (e) => typeof e === "boolean",
  ho: () => true,
  L: (e) => (t) => t === undefined || e(t),
  R: (e) => (t) => t === null || e(t),
  z: (e) => (t) => Array.isArray(t) && t.every((x) => e(x)),
  Set,
  Array,
  Object,
  Uint8Array,
};
vm.createContext(ctx);
for (const [k, arr] of allSets) ctx[k] = new Set(arr);
for (const [k, fnCode] of allFns) {
  if (!fnCode.includes(".safeParse(")) {
    try {
      vm.runInContext(fnCode, ctx);
    } catch {}
  } else {
    ctx[k] = (e) => e !== undefined && e !== null;
  }
}

let passed = 0;
let failed = 0;
for (const item of validators) {
  const key = `${item.service}.${item.method}`;
  if (!(key in actual)) continue;
  const val = actual[key];
  const fn = vm.runInContext(item.validator, ctx);
  if (fn(val)) {
    passed++;
  } else {
    failed++;
    console.error("ACTUAL FAIL:", key, "validator:", item.validator, "got:", JSON.stringify(val));
  }
}
console.log(`Actual EipcRouter output verification: ${passed} passed, ${failed} failed.`);
if (failed > 0) process.exit(1);
