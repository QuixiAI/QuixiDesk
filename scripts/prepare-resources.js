#!/usr/bin/env node
/**
 * Prepare & rebrand QuixiDesk resources from Claude.app (v3.6556.2)
 * - Copies ion-dist, .vite/renderer, preload scripts, fonts, icons, i18n
 * - Surgically rebrands user-facing "Claude" -> "QuixiDesk" (including SVG wordmarks & ICU strings)
 * - Extracts Hon/Uon flags from index.chunk-jPKPzV-P.js and builds bootstrap/*.json
 */
const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const ROOT = "/Users/eric/quixi-desk";
const DMG_RES = "/Volumes/Claude/Claude.app/Contents/Resources";
const ASAR_DIR = path.join(ROOT, "asar-extracted");
const OUT_RES = path.join(ROOT, "resources");

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

console.log("[1/6] Copying ion-dist, renderer, preload, fonts, and icons...");
ensureDir(OUT_RES);
ensureDir(path.join(OUT_RES, "preload"));
ensureDir(path.join(OUT_RES, "bootstrap"));

// Copy ion-dist using rsync for speed
execFileSync("rsync", [
  "-a",
  "--delete",
  `${DMG_RES}/ion-dist/`,
  `${OUT_RES}/ion-dist/`,
]);

// Copy .vite/renderer
execFileSync("rsync", [
  "-a",
  "--delete",
  `${ASAR_DIR}/.vite/renderer/`,
  `${OUT_RES}/renderer/`,
]);

// Copy fonts and tray icons + en-US.json
if (fs.existsSync(`${DMG_RES}/fonts`)) {
  execFileSync("rsync", ["-a", `${DMG_RES}/fonts/`, `${OUT_RES}/fonts/`]);
}
if (fs.existsSync(`${DMG_RES}/en-US.json`)) {
  fs.copyFileSync(`${DMG_RES}/en-US.json`, `${OUT_RES}/en-US.json`);
}
if (fs.existsSync(`${DMG_RES}/electron.icns`)) {
  fs.copyFileSync(`${DMG_RES}/electron.icns`, `${OUT_RES}/quixi.icns`);
}
if (fs.existsSync(`${ASAR_DIR}/resources/bundled-skills`)) {
  execFileSync("rsync", ["-a", "--delete", `${ASAR_DIR}/resources/bundled-skills/`, `${OUT_RES}/bundled-skills/`]);
  ensureDir(`${OUT_RES}/bundled-skills-extracted`);
  for (const f of fs.readdirSync(`${OUT_RES}/bundled-skills`)) {
    if (f.endsWith(".skill")) {
      execFileSync("unzip", ["-q", "-o", `${OUT_RES}/bundled-skills/${f}`, "-d", `${OUT_RES}/bundled-skills-extracted`]);
    }
  }
}
if (fs.existsSync(`${ASAR_DIR}/resources/artifact-libs`)) {
  execFileSync("rsync", ["-a", "--delete", `${ASAR_DIR}/resources/artifact-libs/`, `${OUT_RES}/artifact-libs/`]);
}

// Copy preload scripts
const PRELOADS = [
  "mainView.js",
  "mainWindow.js",
  "quickWindow.js",
  "aboutWindow.js",
  "findInPage.js",
  "localExecConsent.js",
  "buddy.js",
  "artifactShell.js",
  "coworkArtifact.js",
];
for (const p of PRELOADS) {
  const src = path.join(ASAR_DIR, ".vite/build", p);
  if (fs.existsSync(src)) {
    fs.copyFileSync(src, path.join(OUT_RES, "preload", p));
  }
}

console.log("[2/6] Rebranding i18n JSON dictionaries (Claude -> QuixiDesk)...");
function rebrandTextValue(str) {
  if (typeof str !== "string") return str;
  // Replace Claude with QuixiDesk while keeping ICU placeholders {name} intact
  return str
    .replace(/Claude Desktop/g, "QuixiDesk")
    .replace(/Claude for Mac/g, "QuixiDesk for Mac")
    .replace(/Claude for macOS/g, "QuixiDesk for macOS")
    .replace(/Claude for Windows/g, "QuixiDesk for Windows")
    .replace(/Claude Code/g, "QuixiDesk Code")
    .replace(/Claude Cowork/g, "QuixiDesk Cowork")
    .replace(/\bClaude\b/g, "QuixiDesk");
}

function rebrandJsonFile(filePath) {
  if (!fs.existsSync(filePath)) return;
  const raw = JSON.parse(fs.readFileSync(filePath, "utf8"));
  let count = 0;
  for (const k of Object.keys(raw)) {
    if (typeof raw[k] === "string" && raw[k].includes("Claude")) {
      raw[k] = rebrandTextValue(raw[k]);
      count++;
    } else if (raw[k] && typeof raw[k] === "object" && typeof raw[k].message === "string") {
      if (raw[k].message.includes("Claude")) {
        raw[k].message = rebrandTextValue(raw[k].message);
        count++;
      }
    }
  }
  fs.writeFileSync(filePath, JSON.stringify(raw, null, 2));
  console.log(`  Rebranded ${count} entries in ${path.relative(ROOT, filePath)}`);
}

rebrandJsonFile(path.join(OUT_RES, "en-US.json"));
rebrandJsonFile(path.join(OUT_RES, "ion-dist/i18n/en-US.json"));

console.log("[3/6] Rebranding ion-dist/index.html & SVG wordmarks...");
const indexHtmlPath = path.join(OUT_RES, "ion-dist/index.html");
let indexHtml = fs.readFileSync(indexHtmlPath, "utf8");
indexHtml = indexHtml
  .replace(/<title data-react-fallback>Claude<\/title>/g, "<title data-react-fallback>QuixiDesk</title>")
  .replace(/content="Claude is Anthropic's AI[^"]*"/g, 'content="QuixiDesk is an AI desktop workspace built for problem solvers."')
  .replace(/content="Claude"/g, 'content="QuixiDesk"')
  .replace(/aria-label="Write your prompt to Claude"/g, 'aria-label="Write your prompt to QuixiDesk"')
  .replace(/"name": "Claude"/g, '"name": "QuixiDesk"')
  .replace(
    /<svg id="static-sidebar-wordmark"[^>]*>[\s\S]*?<\/svg>/,
    `<svg id="static-sidebar-wordmark" viewBox="30 0 110 24" xmlns="http://www.w3.org/2000/svg" fill="currentColor" shape-rendering="geometricPrecision"><text x="30" y="19" fill="currentColor" style="font-family:var(--cds-font-voice, anthropic-serif, Georgia, serif);font-size:17.5px;font-weight:600;letter-spacing:-0.02em">QuixiDesk</text></svg>`
  );
fs.writeFileSync(indexHtmlPath, indexHtml);

// Rebrand shared-1-BJhqMmkf.js (ClaudeLogo Bh and ProductLogo qh)
const shared1Path = path.join(OUT_RES, "ion-dist/assets/v1/shared-1-BJhqMmkf.js");
if (fs.existsSync(shared1Path)) {
  let s1 = fs.readFileSync(shared1Path, "utf8");
  // 1. Update viewBox dimensions in Rh and zh so "QuixiDesk" fits without clipping
  s1 = s1.replace(
    'Rh={wordmark:Ph,spark:Fh,viewBox:{lockup:"0 0 112 24",wordmark:"30 0 82 24"}}',
    'Rh={wordmark:Ph,spark:Fh,viewBox:{lockup:"0 0 138 24",wordmark:"30 0 108 24"}}'
  );
  s1 = s1.replace(
    'zh={wordmark:Ih,spark:Lh,viewBox:{lockup:"0 0 147 32",wordmark:"38 0 109 32"}}',
    'zh={wordmark:Ih,spark:Lh,viewBox:{lockup:"0 0 180 32",wordmark:"38 0 142 32"}}'
  );
  // 2. Replace the <path d={c.wordmark}> in ClaudeLogo (Bh) with a <text> element rendering "QuixiDesk" in the Anthropic serif font
  s1 = s1.replace(
    'p=J1($,10,c.wordmark)?I1($,10,f("path",{d:c.wordmark}),c.wordmark):$[11]',
    'p=J1($,10,s)?I1($,10,f("text",{x:s>=32?38:30,y:s>=32?24.5:18.8,fill:"currentColor",style:{fontFamily:"var(--cds-font-voice, anthropic-serif, Georgia, serif)",fontSize:s>=32?"22px":"17px",fontWeight:600,letterSpacing:"-0.02em"},children:"QuixiDesk"}),s):$[11]'
  );
  s1 = s1.replace('"aria-label":"Claude"', '"aria-label":"QuixiDesk"');
  // 3. Replace ProductLogo (qh) label `Claude ${r}`:"Claude" -> `QuixiDesk ${r}`:"QuixiDesk"
  s1 = s1.replace('let d=r?`Claude ${r}`:"Claude"', 'let d=r?`QuixiDesk ${r}`:"QuixiDesk"');
  fs.writeFileSync(shared1Path, s1);
  console.log("  Rebranded ClaudeLogo & ProductLogo in shared-1-BJhqMmkf.js");
}

console.log("[4/6] Rebranding renderer & ion-dist JS defaultMessage strings...");
function rebrandJsSource(code) {
  return code
    .replace(/defaultMessage:"([^"]*Claude[^"]*)"/g, (_, msg) => {
      return `defaultMessage:"${rebrandTextValue(msg)}"`;
    })
    .replace(/defaultMessage:'([^']*Claude[^']*)'/g, (_, msg) => {
      return `defaultMessage:'${rebrandTextValue(msg)}'`;
    })
    .replace(/MI="Claude"/g, 'MI="QuixiDesk"')
    .replace(/t===void 0\?"Claude":t/g, 't===void 0?"QuixiDesk":t')
    .replace(/document\.title=vm\|\|"Claude"/g, 'document.title=vm||"QuixiDesk"')
    .replace(/document\.title="Claude"/g, 'document.title="QuixiDesk"')
    .replace(/document\.title="Claude Code"/g, 'document.title="QuixiDesk Code"')
    .replace(/"Security Scan \| Claude Code"/g, '"Security Scan | QuixiDesk Code"')
    .replace(/anthropic:"Claude API"/g, 'anthropic:"QuixiDesk API"')
    .replace(/"claude-api":"Claude API"/g, '"claude-api":"QuixiDesk API"')
    .replace(/"claude-desktop":"Claude Desktop"/g, '"claude-desktop":"QuixiDesk"')
    .replace(
      'let r=document.createElementNS(e,"path");return r.setAttribute("d",Bs),r.setAttribute("fill","currentColor"),t.append(n,r),t',
      't.setAttribute("viewBox","0 0 138 24");let r=document.createElementNS(e,"text");return r.setAttribute("x","30"),r.setAttribute("y","19"),r.setAttribute("fill","currentColor"),r.setAttribute("style","font-family:var(--cds-font-voice, anthropic-serif, Georgia, serif);font-size:18px;font-weight:600;letter-spacing:-0.02em"),r.textContent="QuixiDesk",t.append(n,r),t'
    );
}

const v1Dir = path.join(OUT_RES, "ion-dist/assets/v1");
let jsModifiedCount = 0;
for (const f of fs.readdirSync(v1Dir)) {
  if (!f.endsWith(".js")) continue;
  const p = path.join(v1Dir, f);
  const orig = fs.readFileSync(p, "utf8");
  if (!orig.includes("Claude")) continue;
  const next = rebrandJsSource(orig);
  if (next !== orig) {
    fs.writeFileSync(p, next);
    jsModifiedCount++;
  }
}
console.log(`  Rebranded defaultMessage strings across ${jsModifiedCount} ion-dist JS chunks`);

// Rebrand .vite/renderer files
function walkRenderer(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walkRenderer(p);
    else if (p.endsWith(".js") || p.endsWith(".html")) {
      let orig = fs.readFileSync(p, "utf8");
      let next = rebrandJsSource(orig);
      if (e.name.startsWith("AboutWindow-")) {
        next = next
          .replace('let r=t||"Claude",i=r.startsWith("Claude")?"Claude":""', 'let r=t||"QuixiDesk",i=r.startsWith("QuixiDesk")?"QuixiDesk":""');
      }
      if (e.name.startsWith("MainWindowPage-")) {
        next = next.replace('useState)("Claude")', 'useState)("QuixiDesk")');
      }
      if (e.name === "quick-window.html") {
        next = next.replace(/Claude starburst logo/g, "QuixiDesk starburst logo");
      }
      if (next !== orig) {
        fs.writeFileSync(p, next);
        console.log(`  Rebranded renderer file: ${path.relative(OUT_RES, p)}`);
      }
    }
  }
}
walkRenderer(path.join(OUT_RES, "renderer"));

console.log("[5/6] Extracting Hon/Uon flags and building bootstrap payloads...");
const mainChunkPath = path.join(ASAR_DIR, ".vite/build/index.chunk-jPKPzV-P.js");
const mainChunk = fs.readFileSync(mainChunkPath, "utf8");

// Extract Hon object
const honStart = mainChunk.indexOf("var Hon={");
const uonStart = mainChunk.indexOf("Uon={", honStart);
const wonStart = mainChunk.indexOf(",Won=", uonStart);

let Hon = {};
let Uon = {};
if (honStart !== -1 && uonStart !== -1 && wonStart !== -1) {
  const honCode = mainChunk.slice(honStart + 8, uonStart - 1);
  const uonCode = mainChunk.slice(uonStart + 4, wonStart);
  Hon = Function(`"use strict"; return (${honCode});`)();
  Uon = Function(`"use strict"; return (${uonCode});`)();
}

// Compute growthbook.features exactly as fsn(e) in index.chunk-jPKPzV-P.js does:
const eCfg = {
  scheduledTasksEnabled: true,
  featureDiscoveryDisabled: false,
  titleNeedsCli: false,
  claudeInChromeEnabled: true,
  builtinBrowserEnabled: true,
  autoMode: { code: true, cowork: true },
  bypassPermissionsModeDisabled: false,
  selfHostedSessions: true,
  noSessionRunsOnThisDevice: false,
  alwaysStartWithDefaultModel: false,
  autoUpdatesAllowed: false,
  sessionHost: { io: { downloads: true } },
};
const lsn = {
  scheduledTasksEnabled: (e) => e.scheduledTasksEnabled,
  featureDiscoveryDisabled: (e) => e.featureDiscoveryDisabled,
  titleNeedsCli: (e) => !!e.titleNeedsCli,
  claudeInChromeEnabled: (e) => e.claudeInChromeEnabled,
  builtinBrowserEnabled: (e) => e.builtinBrowserEnabled,
  autoModeCode: (e) => e.autoMode.code,
  autoModeCowork: (e) => e.autoMode.cowork,
  autoModeCodeOrBypassDisabled: (e) => e.autoMode.code || !!e.bypassPermissionsModeDisabled,
  selfHostedSessions: (e) => !!e.selfHostedSessions,
  sessionsRunOnThisComputer: (e) => !e.noSessionRunsOnThisDevice,
  alwaysStartWithDefaultModel: (e) => !!e.alwaysStartWithDefaultModel,
};
const computedRoles = {
  defaultModel: { model: "claude-sonnet-4-6", overrideSticky: false, nuxId: null },
  coworkConfig: { enableAdditionalDirectoriesClaudeMd: true, autoDownloadInBackground: false },
  sessionHostDownloads: true,
};

const growthbookFeatures = {};
for (const [k, ruleObj] of Object.entries(Uon)) {
  let val;
  switch (ruleObj.rule) {
    case "always":
      val = ruleObj.value;
      break;
    case "when":
      val = lsn[ruleObj.condition]?.(eCfg) ? ruleObj.value : undefined;
      break;
    case "follows":
      val = Boolean(lsn[ruleObj.condition]?.(eCfg));
      break;
    case "computed":
      val = computedRoles[ruleObj.role];
      break;
  }
  if (val !== undefined) {
    growthbookFeatures[k] = { defaultValue: val };
  }
}

const ORG_UUID = "00000000-0000-4000-8000-000000000001";
const USER_UUID = "00000000-0000-4000-8000-000000000002";
const nowIso = new Date().toISOString();

function buildModelsForSurface(surface) {
  const isCode = surface === "code";
  const makeThinking = (rec) => {
    const base = {
      effort_options: [
        { id: "low", name: "Low", description: "Quick replies to simple questions" },
        {
          id: "medium",
          name: "Medium",
          description: "Balanced for everyday work",
          ...(rec === "medium" ? { recommended: true, badge: { message: "Default", variant: "neutral" } } : {}),
        },
        {
          id: "high",
          name: "High",
          description: "Complex, detailed work",
          ...(rec === "high" ? { recommended: true, badge: { message: "Default", variant: "neutral" } } : {}),
        },
        {
          id: "max",
          name: "Max",
          description: "The hardest problems. Takes longest.",
          tooltip: {
            content:
              "May use excessive tokens resulting in long response times and may hit token limits. Use sparingly for the hardest tasks.",
          },
        },
      ],
      description:
        "Higher effort means more thorough responses, but takes longer and uses your limits faster.",
    };
    if (isCode) {
      return { ...base, type: "effort" };
    }
    return {
      ...base,
      mode_options: [
        { id: "auto", name: "Thinking", description: "Can think for more complex tasks" },
        { id: "off", name: "Off" },
      ],
    };
  };

  return [
    {
      id: "claude-sonnet-4-6",
      name: "Sonnet 4.6",
      description: "Best for everyday tasks",
      thinking: makeThinking("medium"),
    },
    {
      id: "claude-opus-4-7",
      name: "Opus 4.7",
      description: "Most capable for complex work",
      thinking: makeThinking("high"),
    },
    {
      id: "claude-haiku-4-5-20251001",
      name: "Haiku 4.5",
      description: "Fastest for quick answers",
      thinking: makeThinking("medium"),
    },
  ];
}

// Load gatedMessages from index.chunk-DQTnv1Pl.js if available
let gatedMessages;
try {
  const gmMod = require(path.join(ASAR_DIR, ".vite/build/index.chunk-DQTnv1Pl.js"));
  if (typeof gmMod.publishGatedMessages === "function") {
    gatedMessages = gmMod.publishGatedMessages({ appLocale: "en-US", accountLocale: "en-US" });
  }
} catch (_) {}

const coworkSystemPrompt = `<application_details>
QuixiDesk is operating as an agent inside the QuixiDesk desktop app. QuixiDesk runs with a secure workspace sandbox on the user's computer, allowing controlled access to workspace folders and tools.
</application_details>`;

const bootstrapJson = {
  account: {
    tagged_id: `cowork_3p_${USER_UUID}`,
    uuid: USER_UUID,
    email_address: "user@quixidesk.local",
    full_name: "QuixiDesk",
    display_name: "QuixiDesk",
    created_at: nowIso,
    updated_at: nowIso,
    accepted_clickwrap_versions: {},
    is_verified: true,
    age_is_verified: true,
    memberships: [
      {
        role: "admin",
        seat_tier: "unassigned",
        created_at: nowIso,
        updated_at: nowIso,
        organization: {
          uuid: ORG_UUID,
          id: 0,
          name: "QuixiDesk",
          settings: {
            claude_ai_skill_publish_policy_effective: "review",
          },
          parent_organization_uuid: null,
          capabilities: ["chat", "claude_pro", "claude_max"],
          billing_type: "stripe_subscription",
          free_credits_status: null,
          api_disabled_reason: null,
          api_disabled_until: null,
          rate_limit_tier: null,
          data_retention: null,
          raven_type: null,
          claude_ai_bootstrap_models_config: [],
        },
      },
    ],
    workspace_memberships: [],
    invites: [],
    settings: {
      enabled_wiggle_egress: true,
      wiggle_egress_hosts_template: "full_egress",
      enabled_geolocation: false,
    },
  },
  current_user_access: {
    features: [
      { feature: "chat", status: "available" },
      { feature: "claude_code", status: "available" },
      { feature: "claude_code_desktop", status: "available" },
      { feature: "skills", status: "available" },
      { feature: "skill_creation", status: "available" },
      { feature: "tool_approval_default_always_allow", status: "available" },
      { feature: "claude_code_fast_mode", status: "blocked_by_platform" },
      { feature: "cowork", status: "available" },
      { feature: "claude_browser_extension", status: "available" },
      { feature: "miami_grand_prix", status: "available" },
      { feature: "skill_plugins_scanning", status: "available" },
    ],
  },
  featureFlagsOrgUuid: ORG_UUID,
  locale: "en-US",
  statsig: {
    user: { userID: USER_UUID },
    values: {},
    values_hash: "custom3p",
  },
  feature_flags: {
    user: { userID: USER_UUID },
    values: {},
    values_hash: "custom3p",
  },
  growthbook: {
    features: growthbookFeatures,
  },
  model_selector_config: ["cowork", "code", "chat"].map((surface) => ({
    id: surface,
    models: buildModelsForSurface(surface),
  })),
  model_selector_state: ["cowork", "code", "chat"].map((surface) => ({
    id: surface,
    model: "claude-sonnet-4-6",
    selection_source: "global_default",
  })),
  bootstrap_push_revision: 1,
  install_id: USER_UUID,
  intercom_account_hash: null,
  ...(gatedMessages ? { gated_messages: gatedMessages } : {}),
  gated_imports: {},
  gated_imports_build_id: "spa-dev",
  system_prompts: {
    cowork_system_prompt: {
      value: { prompt: coworkSystemPrompt },
      on: true,
      off: false,
      source: "defaultValue",
      ruleId: null,
    },
  },
};

fs.writeFileSync(
  path.join(OUT_RES, "bootstrap/bootstrap.json"),
  JSON.stringify(bootstrapJson, null, 2)
);

const S = { status: "supported" };
const U = { status: "unavailable" };
const desktopFeatures = {
  launch: S,
  coworkBrowser: S,
  artifactsPane: S,
  nativeQuickEntry: S,
  quickEntryDictation: U,
  quickEntryGlobalShortcut: S,
  trayIcon: S,
  customQuickEntryDictationShortcut: S,
  ultrareviewErrorCategories: S,
  spawnTaskPendingPop: S,
  spawnTaskInFlightIds: S,
  folderWorktrees: S,
  folderWorktreesRemote: S,
  folderWorktreesListedDiscard: S,
  sableOtter: U,
  telescopeBridge: U,
  mcpFinishInApp: U,
  copperLarkA: S,
  copperLarkB: S,
  copperLarkC: U,
  copperLarkD: S,
  copperLarkH: S,
  quietPenguin: U,
  rubberDuck: S,
  chillingSlothFeat: S,
  chillingSlothSshShell: S,
  terminalShellReported: S,
  sshBrowserPane: U,
  chillingSlothSshWorktreeLocation: S,
  chillingSlothSshPuttyKeyConversion: U,
  chillingSlothRemoteFilePreview: S,
  chillingSlothEnterprise: S,
  chillingSlothLocal: S,
  yukonSilver: S,
  yukonSilverGems: S,
  yukonSilverGemsCache: S,
  wakeScheduler: S,
  keepAwake: S,
  codeSessionKeepAwake: S,
  ccdPromptSuggestionsSetting: S,
  ccdLandingTools: S,
  ccdMcpVerbsMidTurn: S,
  ccdMcpTurnOnSshHost: S,
  ccdRestartRemoteClaudeCode: S,
  ccdSshStartsWithoutBlockedPlugins: S,
  openWithConsentAnyLanding: S,
  desktopTopBar: S,
  ccdPlugins: S,
  ccdPluginSourceRouting: S,
  computerUse: S,
  computerUseRollout: S,
  computerUseComplianceGate: S,
  computerUseAppScoped: S,
  computerUsePerAppAccess: S,
  coworkWatchRecord: U,
  coworkCopperHeron: U,
  coworkWatchers: U,
  coworkKappa: U,
  coworkArtifacts: S,
  trustedFolderSettingsConsent: U,
  coworkAutoModeAlwaysAllowOverride: U,
  framebufferPreview: S,
  iosSimulator: S,
  androidEmulator: U,
  androidEmulatorCreate: U,
  iosSimulatorH264: S,
  iosSimulatorAccessibilityReader: S,
  iosSimulatorDuo: U,
  simulatorScreenshotEditor: U,
  simulatorMiniView: U,
  grandPrix: U,
  chromeExtensionInstall: S,
  ccdGitEngine: U,
  sessionPrOwnership: U,
  sessionFolderFileAccess: S,
  conversationFolderGrants: S,
  localSessionsWithoutGit: S,
  tearOffHalo: S,
  coworkArtifactPopout: S,
  sidePanePopoutNativeViews: S,
  simulatorNativeViewPopout: S,
  simulatorPopoutShortcuts: S,
  simulatorNativeViewPerPane: S,
  popoutZoom: S,
  popoutTitleBarOverlay: {
    status: "unsupported",
    reason: "Pop-out title bar overlay requires Windows or Linux",
    unsupportedCode: "unsupported_platform",
  },
  filePreviewNativeViewRouting: S,
  claudeDesignWindow: S,
  coworkRemoteSessionSpaces: S,
  spaceMemoryBridge: S,
  multiAccount: U,
  grandPrixRequest: U,
  bootPlaceholder: S,
  coworkBranchSession: S,
  coworkScheduledTaskProjects: S,
  coworkLocalSessionProjects: S,
  coworkScheduledTaskRepoint: S,
  sshAttachmentTransport: S,
  coworkSeededSummon: S,
  seededKind: S,
  coworkTextNotTyped: U,
  sendNowEscalation: S,
  steerDetachField: S,
  sessionMentions: S,
  inlinePastes: S,
  turnDiscardedEvent: S,
  turnBoxAtReplies: S,
  wslForkSession: U,
  wslTerminal: U,
  wslMcp: U,
  wslPreview: U,
  wslLocalFiles: U,
  wslRoutines: U,
  humbleSky: U,
  violinBow: U,
  violinBowHomeSettings: U,
  coworkThinkingInSend: S,
  bootstrapConfig: S,
  chatIn3p: S,
  chatTab: S,
  selfHostedSessions: S,
  selfHostedCowork: S,
  desktopHome: U,
  chatCodeExecution: S,
  mcpPersistentAlwaysAllow: S,
  mcpScheduledTaskAlwaysAllow: S,
  mcpScheduledTaskApprovalLifetime: S,
  epitaxyMcpApps: S,
  sideSessionProposalPicks: S,
  sideSessionProposalWorktreePick: S,
  sideSessionOfferCloudStart: S,
  sideSessions: S,
  btwChildSessions: U,
  nestedArchiveKeepsFamily: U,
  epitaxyMcpAppPane: U,
  ccdConnectorTools: U,
  ccdTeleportSnapshot: U,
};
fs.writeFileSync(
  path.join(OUT_RES, "bootstrap/desktop-features.json"),
  JSON.stringify(desktopFeatures, null, 2)
);

const desktopManagedConfig = {
  deploymentMode: "3p",
  appVersion: "3.6556.2",
  cookielessOrigin: true,
  deploymentDisplayName: "QuixiDesk",
  disableEssentialTelemetry: true,
  disableNonessentialTelemetry: true,
  preferredSystemLanguages: ["en-US"],
};
fs.writeFileSync(
  path.join(OUT_RES, "bootstrap/desktop-managed-config.json"),
  JSON.stringify(desktopManagedConfig, null, 2)
);

const UUID_MAIN = "5de9b164-2257-4603-a1e1-e00cdb72056c";
const initialStores = {
  "Custom3pSetup_$_bootstrapState": {
    configured: true,
    origin: "local",
    health: "ok",
  },
  "Custom3pSetup_$_hybridTransition": {
    state: "none",
  },
  "SupportBundle_$_supportBundleState": {
    stage: "closed",
    canSend: false,
    showSendUi: false,
  },
  "Simulator_$_attachment": [],
  "Simulator_$_debugAttaching": [],
  "OfficeAddinFiles_$_connectedFilesState": {
    files: [],
    isFeatureEnabled: false,
  },
  "WindowState_$_hostPower": {
    recentSleeps: [],
    recentStalls: [],
  },
  "WindowState_$_deviceLink": {
    appStartedMs: Date.now(),
    connected: true,
    recentGaps: [],
  },
  "WindowState_$_compositing": {
    software: false,
  },
  "AutoUpdater_$_updaterState": {
    status: "idle",
  },
  "BrowserNavigation_$_navigationState": {
    canGoBack: false,
    canGoForward: false,
  },
  "ManagedConfig_$_managedRendererConfig": {
    forceLoginOrgUUIDs: null,
    loginSsoOrgDomain: null,
    disableEssentialTelemetry: true,
    disableNonessentialTelemetry: true,
    disableMobileSimulatorTools: false,
    banner: null,
    deploymentDisplayName: "QuixiDesk",
    deploymentDisplaySubtitle: null,
    dictationAvailable: false,
    userPluginMarketplacesEnabled: true,
    userPluginUploadsEnabled: true,
    noSessionRunsOnThisDevice: false,
    configDeprecations: [],
    configDeprecationNoticesSuppressed: true,
    configOrgDelivered: false,
    userContentFrameAllowed: true,
  },
  "CliSessionImport_$_cliImport": {
    seq: 0,
    stage: "closed",
  },
  "ComputerUseLivePreview_$_computerUsePreview": {
    active: false,
  },
  "ComputerUseLivePreview_$_browserPreview": {
    active: false,
  },
  "LocalAgentModeSessions_$_sessionsBridgeStatus": {
    conflict: false,
    dispatchAgentName: null,
  },
  "LocalAgentModeSessions_$_interactiveAuth": {
    needsAuth: false,
  },
  "GrandPrix_$_grandPrixStatus": {
    paired: {},
    installed: {},
    displayNames: {},
    iconDataUris: {},
  },
  "Launch_$_activeServers": [],
  "RemoteControlServing_$_remoteControlServeStatus": {
    available: false,
    reason: "pending",
    permitted: false,
    intent: "unprompted",
    machineName: "QuixiDesk-Mac",
    homePath: "/Users/eric",
    folders: [],
    atLimit: false,
    activeSessions: 0,
    capacity: 5,
    stayReachable: { requested: false, mode: "off", onBattery: false },
  },
  "ViolinFret_$_violinFretStatus": {
    available: false,
    choice: "unset",
    gatesHosting: true,
  },
  "ClaudeVM_$_apiReachability": {
    reachability: "reachable",
    willTryRecover: false,
  },
  "ClaudeVM_$_hostIPv6Notice": "none",
  "FindInPage_$_density": "comfortable",
};

fs.writeFileSync(
  path.join(OUT_RES, "bootstrap/initial-stores.json"),
  JSON.stringify(initialStores, null, 2)
);

console.log("[6/6] Building resources/preload/combined_preload.js & running preload self-test...");
const mainViewSrc = fs.readFileSync(path.join(OUT_RES, "preload/mainView.js"), "utf8");
const mainWindowSrc = fs.readFileSync(path.join(OUT_RES, "preload/mainWindow.js"), "utf8");
const findInPageSrc = fs.readFileSync(path.join(OUT_RES, "preload/findInPage.js"), "utf8");
const localExecConsentSrc = fs.readFileSync(path.join(OUT_RES, "preload/localExecConsent.js"), "utf8");

const combinedPreload = `// Auto-generated QuixiDesk Preload Bridge (Electron-free wry initialization script)
(function () {
  try {
    if (typeof window === "undefined") return;
    if (window.__QUIXI_PRELOAD_INSTALLED__) return;
    if (window.location && window.location.protocol === "about:") return;
    window.__QUIXI_PRELOAD_INSTALLED__ = true;

    const DESKTOP_FEATURES = ${JSON.stringify(desktopFeatures)};
    const DESKTOP_MANAGED_CONFIG = ${JSON.stringify(desktopManagedConfig)};
    const BOOT_PLACEHOLDER = { enabled: true, shellZoomFactor: 1 };
    const INITIAL_STORES = ${JSON.stringify(initialStores)};

    window.__QUIXI_STORES__ = Object.assign({}, INITIAL_STORES, window.__QUIXI_STORES__ || {});
    const listeners = (window.__QUIXI_LISTENERS__ = window.__QUIXI_LISTENERS__ || new Map());

    function extractStoreKey(channel) {
      if (typeof channel !== "string") return null;
      const m = channel.match(/_\\$_([A-Za-z0-9_]+_\\$_[A-Za-z0-9_]+)_\\$store\\$_(?:getStateSync|getState|update)$/);
      return m ? m[1] : null;
    }

    window.__quixi_eipc_emit__ = function (channel, ...args) {
      try {
        if (typeof channel === "string" && channel.endsWith("_$store$_update")) {
          const sk = extractStoreKey(channel);
          if (sk && args.length > 0) {
            window.__QUIXI_STORES__[sk] = args[0];
          }
        }
        const set = listeners.get(channel);
        if (set) {
          const ev = { sender: {}, ports: [] };
          for (const fn of Array.from(set)) {
            try {
              fn(ev, ...args);
            } catch (err) {
              console.error("[QuixiDesk IPC listener error]", channel, err);
            }
          }
        }
      } catch (err) {
        console.error("[QuixiDesk emit error]", err);
      }
    };

    const argv = [
      "quixi-app",
      "--desktop-features=" + JSON.stringify(DESKTOP_FEATURES),
      "--desktop-managed-config=" + JSON.stringify(DESKTOP_MANAGED_CONFIG),
      "--desktop-boot-placeholder=" + JSON.stringify(BOOT_PLACEHOLDER),
    ];

    const proc = {
      arch: "arm64",
      platform: "darwin",
      type: "renderer",
      version: "3.6556.2",
      versions: {
        electron: "44.9.3",
        chrome: "152.0.7977.167",
        node: "24.22.0",
      },
      env: {},
      argv: argv,
      isMainFrame: true,
      contextIsolated: true,
      cwd: function () {
        return "/";
      },
      getHeapStatistics: function () {
        return {
          totalHeapSize: 65536,
          totalHeapSizeExecutable: 4096,
          totalPhysicalSize: 65536,
          totalAvailableSize: 524288,
          usedHeapSize: 32768,
          heapSizeLimit: 2097152,
          mallocedMemory: 8192,
          peakMallocedMemory: 16384,
          doesZapGarbage: false,
          numberOfNativeContexts: 1,
          numberOfDetachedContexts: 0,
        };
      },
      getBlinkMemoryInfo: function () {
        return { allocated: 32768, marked: 16384, total: 65536 };
      },
    };
    globalThis.process = Object.assign(globalThis.process || {}, proc);

    const ipcRenderer = {
      invoke: async function (channel, ...args) {
        try {
          const res = await fetch("/__eipc__/invoke", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ channel, args }),
          });
          if (res.ok) {
            const data = await res.json();
            if (data && "error" in data && data.error) {
              throw new Error(data.error);
            }
            if (data && "result" in data) {
              return data.result;
            }
            return data;
          }
        } catch (err) {
          const sk = extractStoreKey(channel);
          if (sk && sk in window.__QUIXI_STORES__) {
            return window.__QUIXI_STORES__[sk];
          }
          console.warn("[QuixiDesk invoke fallback]", channel, err);
        }
        const sk = extractStoreKey(channel);
        if (sk && sk in window.__QUIXI_STORES__) {
          return window.__QUIXI_STORES__[sk];
        }
        return null;
      },
      sendSync: function (channel, ...args) {
        const sk = extractStoreKey(channel);
        if (sk && sk in window.__QUIXI_STORES__) {
          return { result: window.__QUIXI_STORES__[sk] };
        }
        try {
          const xhr = new XMLHttpRequest();
          xhr.open("POST", "/__eipc__/sync", false);
          xhr.setRequestHeader("Content-Type", "application/json");
          xhr.send(JSON.stringify({ channel, args }));
          if (xhr.status === 200 && xhr.responseText) {
            return JSON.parse(xhr.responseText);
          }
        } catch (_) {}
        return { result: null };
      },
      send: function (channel, ...args) {
        try {
          fetch("/__eipc__/send", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ channel, args }),
          }).catch(function () {});
        } catch (_) {}
      },
      on: function (channel, listener) {
        if (!listeners.has(channel)) {
          listeners.set(channel, new Set());
        }
        listeners.get(channel).add(listener);
        return ipcRenderer;
      },
      once: function (channel, listener) {
        const wrapper = function (ev, ...a) {
          ipcRenderer.removeListener(channel, wrapper);
          listener(ev, ...a);
        };
        return ipcRenderer.on(channel, wrapper);
      },
      removeListener: function (channel, listener) {
        const set = listeners.get(channel);
        if (set) set.delete(listener);
        return ipcRenderer;
      },
      removeAllListeners: function (channel) {
        if (channel) listeners.delete(channel);
        else listeners.clear();
        return ipcRenderer;
      },
    };

    const webFrame = {
      frameToken: "quixi-main-frame-token",
      routingId: 1,
      top: {
        frameToken: "quixi-main-frame-token",
        routingId: 1,
      },
      getZoomFactor: function () {
        return 1;
      },
      setZoomFactor: function () {},
      setVisualZoomLevelLimits: function () {},
    };

    const contextBridge = {
      exposeInMainWorld: function (key, val) {
        if (key === "process") {
          Object.assign(globalThis.process, val, {
            argv: argv,
            isMainFrame: true,
            getHeapStatistics: proc.getHeapStatistics,
            getBlinkMemoryInfo: proc.getBlinkMemoryInfo,
          });
          return;
        }
        if (
          val &&
          typeof val === "object" &&
          !Array.isArray(val) &&
          globalThis[key] &&
          typeof globalThis[key] === "object" &&
          !Array.isArray(globalThis[key])
        ) {
          Object.assign(globalThis[key], val);
        } else {
          globalThis[key] = val;
        }
      },
      executeInMainWorld: function (spec) {
        if (spec && typeof spec.func === "function") {
          return spec.func.apply(globalThis, spec.args || []);
        }
      },
    };

    const webUtils = {
      getPathForFile: function (file) {
        return (file && (file.path || file.name)) || "";
      },
    };

    const electronModule = {
      ipcRenderer: ipcRenderer,
      contextBridge: contextBridge,
      webFrame: webFrame,
      webUtils: webUtils,
    };

    const prevRequire = globalThis.require;
    globalThis.require = function (id) {
      if (id === "electron" || id === "electron/renderer" || id === "electron/common") {
        return electronModule;
      }
      if (typeof prevRequire === "function") {
        return prevRequire(id);
      }
      throw new Error("Module not found: " + id);
    };

    // Execute original preload bundles in isolated function scopes
    (function () {
      ${mainViewSrc}
    })();
    (function () {
      ${mainWindowSrc}
    })();
    (function () {
      ${findInPageSrc}
    })();
    (function () {
      ${localExecConsentSrc}
    })();
  } catch (err) {
    console.error("[QuixiDesk combined_preload fatal error]", err);
  }
})();
`;

const combinedPreloadPath = path.join(OUT_RES, "preload/combined_preload.js");
fs.writeFileSync(combinedPreloadPath, combinedPreload);

// Self-test in a mocked browser VM context
const vm = require("vm");
const sandbox = {
  console,
  URL,
  URLSearchParams,
  TextEncoder,
  TextDecoder,
  WeakSet,
  WeakMap,
  Map,
  Set,
  Promise,
  Error,
  TypeError,
  SyntaxError,
  RegExp,
  Object,
  Array,
  String,
  Number,
  Boolean,
  Date,
  Math,
  JSON,
  Symbol,
  Proxy,
  Reflect,
  Uint8Array,
  setTimeout: () => 1,
  clearTimeout: () => {},
  setInterval: () => 1,
  clearInterval: () => {},
  crypto: { randomUUID: () => "00000000-0000-4000-8000-000000000000" },
  navigator: { userAgent: "QuixiDesk/3.6556.2 Claude/3.6556.2", languages: ["en-US"] },
  location: {
    href: "app://localhost/",
    protocol: "app:",
    host: "localhost",
    hostname: "localhost",
    origin: "app://localhost",
    pathname: "/",
    search: "",
    hash: "",
  },
  opener: null,
  addEventListener: () => {},
  removeEventListener: () => {},
  postMessage: () => {},
  fetch: async () => ({ ok: true, json: async () => ({ result: null }) }),
  document: {
    addEventListener: () => {},
    removeEventListener: () => {},
    documentElement: { setAttribute: () => {}, removeAttribute: () => {}, hasAttribute: () => false },
  },
};
sandbox.window = sandbox;
sandbox.self = sandbox;
sandbox.top = sandbox;
sandbox.globalThis = sandbox;
sandbox.frames = [];

vm.runInNewContext(combinedPreload, sandbox);
console.log("  [Self-Test] claude.web services:", Object.keys(sandbox["claude.web"] || {}).length);
console.log("  [Self-Test] claude.settings services:", Object.keys(sandbox["claude.settings"] || {}).length);
console.log("  [Self-Test] claude.hybrid services:", Object.keys(sandbox["claude.hybrid"] || {}).length);
console.log("  [Self-Test] claude.internal.ui services:", Object.keys(sandbox["claude.internal.ui"] || {}).length);
console.log("  [Self-Test] desktopBootFeatures.chatIn3p:", sandbox.desktopBootFeatures?.chatIn3p);
console.log(
  "  [Self-Test] ManagedConfig.getStateSync().deploymentDisplayName:",
  sandbox["claude.web"].ManagedConfig.managedRendererConfigStore.getStateSync().deploymentDisplayName
);
console.log(
  "  [Self-Test] Custom3pSetup.bootstrapStateStore.getStateSync().health:",
  sandbox["claude.settings"].Custom3pSetup.bootstrapStateStore.getStateSync().health
);
console.log("Done! Resources prepared in /Users/eric/quixi-desk/resources");

