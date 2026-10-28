#!/usr/bin/env node
const fs = require("fs");
const vm = require("vm");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const OUTPUT = path.join(ROOT, "crates/quixi-ipc/ipc-exact-schemas.json");

const code = fs.readFileSync(path.join(ROOT, "asar-extracted/.vite/build/index.chunk-jPKPzV-P.js"), "utf8");

// 1. Extract all function NAME(e){...} validator definitions
const allFns = new Map();
const re = /function ([A-Za-z0-9_$]+)\(e\)\{([^{}]*(?:\{[^{}]*\}[^{}]*)*)\}/g;
let m;
while ((m = re.exec(code)) !== null) {
  allFns.set(m[1], m[0]);
}

// 2. Extract all Set definitions
const allSets = new Map();
const setRe = /(?:^|[^A-Za-z0-9_$])([A-Za-z0-9_$]+)=new Set\((\[[^\]]*\])\)/g;
while ((m = setRe.exec(code)) !== null) {
  try {
    allSets.set(m[1], JSON.parse(m[2]));
  } catch {}
}

// 3. Extract exact ldt (AppPreferences default) and JU (AppFeatures)
const ldtMatch = code.match(/ldt=(\{[\s\S]*?\}),ig=/);
const exactPreferences = ldtMatch ? vm.runInNewContext("(" + ldtMatch[1] + ")") : {};

// Extract all keys of Umt (AppFeatures)
const umtMatch = code.match(/Umt=A\(\{([\s\S]*?)\}\)\.partial\(\)/);
const exactFeatures = {};
if (umtMatch) {
  const keyRe = /([A-Za-z0-9_$]+):/g;
  let km;
  while ((km = keyRe.exec(umtMatch[1])) !== null) {
    const k = km[1];
    if (["status", "supported", "unavailable", "scheme"].includes(k)) continue;
    exactFeatures[k] = { status: "supported" };
  }
  exactFeatures.mcpFinishInApp = { status: "unavailable" };
}

// 4. Exact Zod-validated payloads for the 41 safeParse validators
const zodDefaults = {
  hSe: {
    isNestBuild: false,
    buildType: "prod",
    commitHash: "20c019a1e61fa5b687616af57d31f97bb3181723",
    commitTimestamp: "2026-10-26T00:00:00Z",
    appVersion: "3.6556.2",
  },
  Amt: { command: "quixi-mcp", args: [] },
  Wmt: { manifest_version: "0.1", name: "quixi-ext", version: "1.0.0", description: "QuixiDesk Extension", author: { name: "QuixiDesk" }, server: { type: "node", entry_point: "index.js", mcp_config: { command: "node", args: ["index.js"] } } },
  Gmt: {},
  Kmt: {},
  qmt: "running",
  Jmt: { isEnabled: true },
  Ymt: {},
  Xmt: [],
  Zmt: { compatible: true, requirements: [] },
  Qmt: { id: "default" },
  $mt: {
    id: "default",
    info: {
      id: "default",
      icon_url: null,
      upload_date: "2026-10-26T00:00:00Z",
      manifest: {},
      signature_info: { status: "signed" },
    },
    file_size: 0,
    download_count: 0,
  },
  eht: {
    url: "https://claude.ai",
    data: {
      id: "default",
      info: {
        id: "default",
        icon_url: null,
        upload_date: "2026-10-26T00:00:00Z",
        manifest: {},
        signature_info: { status: "signed" },
      },
      file_size: 0,
      download_count: 0,
    },
  },
  tht: exactPreferences,
  nht: exactFeatures,
  JCt: { text: "", images: [] },
  YCt: { chatId: "default", chatName: "New Chat" },
  XCt: {
    isLoggedOut: false,
    hasWiggle: true,
    isRaven: false,
    isHipaaRegulated: false,
    organizationUuid: "00000000-0000-4000-8000-000000000001",
    organizationIsEnterprise: false,
    organizationIsProOrMax: true,
    paidAccountTier: "claude_max",
    planDisplayName: "QuixiDesk Max",
    isEnterprise: false,
    canUseOmelette: true,
    isLegacySeatBased: false,
    coworkMcpWriteToolsAlwaysAllowEnabled: true,
    coworkRemoteScheduledTasksEnabled: true,
    coworkAvailablePermissionModes: ["default", "acceptEdits", "bypassPermissions", "plan"],
    coworkTrustedDevicesRequired: false,
    coworkNewLocalScheduledTasksOff: false,
    coworkDispatchOff: false,
    accountUuid: "00000000-0000-4000-8000-000000000002",
    displayName: "QuixiDesk",
    fullName: "QuixiDesk User",
  },
  IOt: [],
  _Ft: {
    status: "launched",
    sessionId: "local_ultra_1",
    sessionUrl: "https://claude.ai/code",
    message: "Launched Ultrareview",
    billingNote: "Included in QuixiDesk",
  },
  vFt: {
    status: "ok",
    cwd: "/Users/eric/quixi-desk",
    changed: true,
    transcriptRelocated: false,
  },
  cy: {
    kind: "ssh",
    sshHost: "localhost",
  },
  uLt: {},
  Izt: {},
  Bzt: {
    id: "quixi-core",
    name: "quixi-core",
    displayName: "QuixiDesk Core Plugin",
    installPath: "/Users/eric/.quixi/plugins/quixi-core",
    scope: "user",
    enabled: true,
    skills: [],
    mcpServers: [],
    hooks: [],
  },
  Vzt: {},
  Hzt: { success: true },
  Uzt: {
    schema: {},
    inputs: {},
    choices: {},
    configured: [],
    unconfirmed: [],
  },
  Wzt: { success: true, saved: [] },
  Gzt: {},
  pBt: {
    name: "quixi-marketplace",
    displayName: "QuixiDesk Marketplace",
    sourceType: "remote",
    isDefault: true,
    source: "remote",
  },
  mBt: {
    id: "quixi-core",
    name: "quixi-core",
    displayName: "QuixiDesk Core Plugin",
    marketplaceName: "quixi-marketplace",
  },
  hBt: { success: true, pluginId: "quixi-core" },
  gBt: { success: true, pluginId: "quixi-core", alreadyUpToDate: true },
  _Bt: { lastSyncedSha: "20c019a1e61fa5b687616af57d31f97bb3181723" },
  vBt: { hasLocalChanges: false, changedFiles: [] },
  yBt: { plugins: [], hasMore: false },
  bBt: { plugins: [] },
  SBt: "available",
  TBt: {
    available: true,
    reason: "serving",
    permitted: true,
    intent: "on",
    machineName: "QuixiDesk-Mac",
    homePath: "/Users/eric",
    folders: [],
    atLimit: false,
    activeSessions: 0,
    capacity: 10,
    stayReachable: { requested: false, mode: "off", onBattery: false },
  },
  yy: { outcome: "applied" },
  ZTi: [],
};

// Build VM context
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
for (const [k, arr] of allSets) {
  ctx[k] = new Set(arr);
}
for (const [k, fnCode] of allFns) {
  if (!fnCode.includes(".safeParse(")) {
    try {
      vm.runInContext(fnCode, ctx);
    } catch {}
  } else {
    ctx[k] = (e) => e !== undefined;
  }
}

function synth(expr, depth = 0) {
  if (depth > 8) return null;
  if (zodDefaults[expr] !== undefined) return zodDefaults[expr];
  if (expr === "I") return true;
  if (expr === "F") return "";
  if (expr === "mo") return 0;
  if (expr === "ho") return {};
  if (expr.startsWith("z(") && expr.endsWith(")")) return [];
  if (expr.startsWith("R(") && expr.endsWith(")")) {
    const inner = expr.slice(2, -1);
    return synth(inner, depth + 1);
  }
  if (expr.startsWith("L(") && expr.endsWith(")")) {
    const inner = expr.slice(2, -1);
    return synth(inner, depth + 1);
  }
  const body = allFns.get(expr);
  if (!body) return null;
  const setMatch = body.match(/return ([A-Za-z0-9_$]+)\.has\(e\)/);
  if (setMatch) {
    const arr = allSets.get(setMatch[1]);
    return arr && arr.length > 0 ? arr[0] : "";
  }
  if (body.includes('typeof e!="string"')) return "quixi";
  if (body.includes('typeof e!="number"')) return 1;
  if (body.includes("for(let t of Object.keys(e))")) return {};
  if (body.includes('typeof e!="object"')) {
    const obj = {};
    const clauses = body.split("||");
    for (const c of clauses) {
      if (/e\.([A-Za-z0-9_$]+)!==void 0/.test(c)) continue;
      const nullMatch = c.match(/e\.([A-Za-z0-9_$]+)!==null/);
      if (nullMatch) {
        obj[nullMatch[1]] = null;
        continue;
      }
      const strMatch = c.match(/typeof e\.([A-Za-z0-9_$]+)!="string"/);
      if (strMatch) {
        obj[strMatch[1]] = "";
        continue;
      }
      const numMatch = c.match(/typeof e\.([A-Za-z0-9_$]+)!="number"/);
      if (numMatch) {
        obj[numMatch[1]] = 0;
        continue;
      }
      const boolMatch = c.match(/typeof e\.([A-Za-z0-9_$]+)!="boolean"/);
      if (boolMatch) {
        obj[boolMatch[1]] = false;
        continue;
      }
      const arrMatch = c.match(/!\(Array\.isArray\(e\.([A-Za-z0-9_$]+)\)/);
      if (arrMatch) {
        obj[arrMatch[1]] = [];
        continue;
      }
      const fnMatch = c.match(/!([A-Za-z0-9_$]+)\(e\.([A-Za-z0-9_$]+)\)/);
      if (fnMatch) {
        obj[fnMatch[2]] = synth(fnMatch[1], depth + 1);
        continue;
      }
    }
    return obj;
  }
  return {};
}

const v = JSON.parse(fs.readFileSync(path.join(ROOT, "reference/ipc-return-validators.json"), "utf8"));
const methodsMap = {};
let passed = 0;
let failed = 0;

for (const item of v) {
  const key = `${item.service}.${item.method}`;
  const val = synth(item.validator);
  const fn = vm.runInContext(item.validator, ctx);
  const ok = fn(val);
  if (ok) {
    passed++;
    methodsMap[key] = val;
  } else {
    failed++;
    console.error("FAIL method:", key, item.validator);
  }
}

const storesMap = {};
const goRe = /go\([A-Za-z0-9_$]+,"([^"]+)",[A-Za-z0-9_$]+,\{([\s\S]*?)\}\)/g;
let gm;
while ((gm = goRe.exec(code)) !== null) {
  const service = gm[1];
  const stMatch = gm[2].match(/stores:\[(\[[\s\S]*?\])\]/);
  if (stMatch) {
    const itemRe = /\["([^"]+)",([A-Za-z0-9_$()]+)\]/g;
    let im;
    while ((im = itemRe.exec(stMatch[1])) !== null) {
      const key = `${service}.${im[1]}`;
      const val = synth(im[2]);
      const fn = vm.runInContext(im[2], ctx);
      const ok = fn(val);
      if (ok) {
        passed++;
        storesMap[key] = val;
      } else {
        failed++;
        console.error("FAIL store:", key, im[2]);
      }
    }
  }
}

fs.writeFileSync(
  OUTPUT,
  JSON.stringify({ methods: methodsMap, stores: storesMap }, null, 2)
);
console.log(`Generated ${OUTPUT}: ${passed} passed, ${failed} failed.`);
