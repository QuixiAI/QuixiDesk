/* Copyright (c) 2026 QuixiAI. MIT license.
 * `ui-superagent` — 6-Tab Proactive Repository Orchestrator Panel (`dsh-superagent`).
 * Principle: "Take as little human attention as possible, and give as much value as possible."
 */
(() => {
  "use strict";

  const cordis = window.QuixiCordis;
  if (!cordis) return;

  const GLOSSARY_TERMS = [
    ["Source (源)", "An external place Superagent watches and speaks to (e.g. GitHub issues, pull requests, comments, or simulated webhooks)."],
    ["Trigger (触发)", "One external event with an actor, scope, thread key, and message body."],
    ["Thread (线程)", "A grouped external conversation (`github:<number>`). Holds subscribed Tickets and serializes Eval per thread."],
    ["Eval (评估)", "Two-layer decision flow: deterministic Gate (allowed senders & admission table) followed by model Assessment under AcceptMode."],
    ["AcceptMode", "`off` (only explicit requests from allowed senders), `observe` (default — evaluates proactive candidates and records `wouldAccept` without opening Tickets), `auto` (opens proactive Tickets automatically)."],
    ["Mandate (授权)", "Scoped authority attached to a Ticket (`explicit` or `inferred`), tied to a SHA-256 `policyVersion` and revalidated before resuming."],
    ["Ticket (工单)", "One unit of authorized repository work with ordered stage (`person/*`, `working/queued`, `idle`, `working`, or `ended`)."],
    ["Assignment (指派)", "One execution run on isolated worker session `worker-<ticket_id>` inside `<stateDir>/work/<short_name>`."],
    ["Outbox (发件箱)", "Durable outgoing replies (`acknowledgement`, `answer`, `result`, `attention`) bounded to <=100 words for IM readability."],
  ];

  const STAGE_LABELS = {
    "person/delivery": { label: "Delivery error", tone: "err" },
    "person/answer": { label: "Needs answer", tone: "warn" },
    "person/review": { label: "Ready for review", tone: "ok" },
    "person/blocked": { label: "Blocked on person", tone: "warn" },
    "person/failed": { label: "Worker failed", tone: "err" },
    "working/queued": { label: "Queued", tone: "info" },
    idle: { label: "Idle / Watching", tone: "muted" },
    working: { label: "Working", tone: "info" },
    ended: { label: "Ended", tone: "muted" },
  };

  cordis.definePlugin({
    id: "ui-superagent",
    title: "Superagent 6-Tab Supervision & Policy Panel",
    hostRowId: "dsh-superagent",
    setup(ctx) {
      let activeTab = "overview"; // overview | source | trigger | eval | execution | settings
      let settingsSubtab = "policy"; // sources | policy | execution | prompts
      let selectedTicketId = null;
      let latestSnap = null;
      let liveMountRefresh = null;
      let bootFetched = false;

      ctx.slot("sidebar.nav", {
        order: 55,
        label: "Superagent",
        render: ({ navigationButton, state, navigate }) => {
          const rpcFn = window.quixi?.rpc;
          if (!bootFetched && typeof rpcFn === "function") {
            bootFetched = true;
            Promise.resolve(rpcFn("Superagent", "snapshot", []))
              .then((snap) => {
                if (snap && !snap.error) {
                  const prev = latestSnap?.attentionCount || 0;
                  latestSnap = snap;
                  if ((snap.attentionCount || 0) !== prev) {
                    cordis.requestRerender();
                  }
                }
              })
              .catch(() => {});
          }
          const badgeCount = latestSnap?.attentionCount || 0;
          const label = badgeCount > 0 ? `Superagent (${badgeCount})` : "Superagent";
          return navigationButton(
            label,
            "spark",
            () => navigate("superagent"),
            state.page === "superagent" ? "active" : "",
          );
        },
      });

      const onChanged = (ev) => {
        if (ev?.detail && !ev.detail.error) {
          latestSnap = ev.detail;
          if (typeof liveMountRefresh === "function") liveMountRefresh(ev.detail);
        }
      };
      window.addEventListener("quixi:superagent-changed", onChanged);
      ctx.effect("superagent-event-listener", () => () => {
        window.removeEventListener("quixi:superagent-changed", onChanged);
        liveMountRefresh = null;
      });

      function renderStageBadge(el, stageCode) {
        const meta = STAGE_LABELS[stageCode] || { label: stageCode || "unknown", tone: "muted" };
        return el("span", { class: `sa-badge tone-${meta.tone}` }, meta.label);
      }

      function openGlossaryModal(app) {
        const { el, button, dialog } = app;
        const modal = dialog("Superagent Domain Glossary", [
          el("p", { class: "hint" }, "Core principle: Take as little human attention as possible, and give as much value as possible."),
          el("div", { class: "sa-glossary-list" },
            ...GLOSSARY_TERMS.map(([term, def]) =>
              el("div", { class: "sa-glossary-item" },
                el("strong", {}, term),
                el("p", {}, def),
              ),
            ),
          ),
          el("div", { class: "dialog-actions" }, button("Close", () => modal.close(), "primary")),
        ], { wide: true });
      }

      function openSimulateTriggerModal(app, refreshFn) {
        const { el, button, field, select, dialog, toast, rpc } = app;
        const actorInp = el("input", { value: "maintainer", placeholder: "GitHub username (e.g. alice)" });
        const scopeSel = select(
          [
            ["issues", "GitHub Issue"],
            ["pulls", "GitHub Pull Request"],
            ["reviews", "PR Review"],
          ],
          "issues",
          () => {},
        );
        const addrSel = select(
          [
            ["mention", "Explicit @mention"],
            ["assign", "Assigned to agent"],
            ["none", "Unaddressed (Proactive candidate)"],
          ],
          "mention",
          () => {},
        );
        const threadInp = el("input", { value: `github:${Math.floor(100 + Math.random() * 900)}` });
        const titleInp = el("input", { placeholder: "#142 Fix race condition in retry queue" });
        const textInp = el("textarea", {
          rows: 4,
          placeholder: "@agent Please investigate the flaky retry timeout and open a fix PR.",
        });
        const autoRouteCb = el("input", { type: "checkbox", checked: true });

        const modal = dialog("Submit / Simulate Trigger", [
          el("p", { class: "hint" }, "Inject an external event into Superagent's intake pipeline (`Trigger -> Gate -> Eval -> Ticket`). Tip: ensure the sender is in Settings -> Allowed Senders for explicit acceptance."),
          field("Sender / Actor", actorInp),
          field("Scope", scopeSel),
          field("Addressing", addrSel),
          field("Thread Key", threadInp),
          field("Title", titleInp),
          field("Message Body", textInp),
          el("label", { class: "mcp-checkbox" }, autoRouteCb, " Immediately accept & create Ticket if explicit"),
          el("div", { class: "dialog-actions" },
            button("Cancel", () => modal.close(), "quiet"),
            button("Submit Trigger", async () => {
              const title = titleInp.value.trim() || "Investigate repository issue";
              const res = await rpc("Superagent", "submitTrigger", [{
                source: "github",
                scope: scopeSel.value,
                actor: actorInp.value.trim() || "maintainer",
                isBot: false,
                addressed: addrSel.value,
                thread: threadInp.value.trim() || "github:1",
                title,
                text: textInp.value.trim() || title,
              }]);
              if (res?.error) {
                toast(res.error, true);
                return;
              }
              if (res.disposition === "stored" && autoRouteCb.checked && addrSel.value !== "none") {
                const slug = title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 28) || "task";
                const routed = await rpc("Superagent", "routeDecisions", [[{
                  kind: "accept",
                  trigger_id: res.triggerId,
                  title,
                  worktree_name: slug,
                  acknowledgement: `Accepted "${title}" — spinning up isolated worker workspace.`,
                }]]);
                const first = routed?.results?.[0];
                if (first?.refused) {
                  toast(`Trigger stored (${res.triggerId}); direct accept refused: ${first.refused}`, true);
                } else {
                  toast(`Trigger accepted -> Ticket ${first?.ticket?.id || ""} created.`);
                }
              } else {
                toast(`Trigger ${res.triggerId}: ${res.disposition}`);
              }
              modal.close();
              await refreshFn();
            }, "primary"),
          ),
        ]);
      }

      ctx.slot("main.page", {
        key: "superagent",
        label: "Superagent",
        render: (app) => {
          const { el, button, field, select, empty, toast, rpc, navigate, plainBody } = app;
          const root = el("div", { class: "sa-page" });
          const headerHost = el("div", { class: "sa-topbar" });
          const searchResultsHost = el("div", { class: "sa-search-results", hidden: true });
          const tabBar = el("div", { class: "cordis-segmented sa-tabs", role: "tablist" });
          const bodyHost = el("div", { class: "sa-body" });

          const refresh = async () => {
            const snap = await rpc("Superagent", "snapshot", []);
            if (snap?.error) {
              bodyHost.replaceChildren(empty("Superagent Unavailable", snap.error));
              return;
            }
            latestSnap = snap;
            draw(snap);
          };
          liveMountRefresh = (snap) => draw(snap);

          const openSessionById = async (sid) => {
            if (!sid) return;
            await app.loadSessions?.();
            navigate("chat", sid);
          };

          const draw = (snap) => {
            // 1. Header bar with Pause/Resume, Search, Glossary (`?`), Main Session, Submit Trigger
            const searchInp = el("input", {
              class: "sa-search-input",
              placeholder: "Search tickets, threads, triggers…",
              "aria-label": "Search Superagent",
            });
            searchInp.addEventListener("input", async () => {
              const q = searchInp.value.trim();
              if (!q) {
                searchResultsHost.hidden = true;
                searchResultsHost.replaceChildren();
                return;
              }
              const hits = await rpc("Superagent", "search", [q]);
              searchResultsHost.hidden = false;
              const rows = [];
              for (const t of hits.tickets || []) {
                rows.push(el("button", {
                  type: "button",
                  class: "sa-search-hit",
                  onclick: () => {
                    selectedTicketId = t.id;
                    activeTab = "execution";
                    searchResultsHost.hidden = true;
                    draw(snap);
                  },
                }, renderStageBadge(el, t.stage), el("strong", {}, t.title), el("small", {}, t.id)));
              }
              for (const tr of hits.items || []) {
                rows.push(el("button", {
                  type: "button",
                  class: "sa-search-hit",
                  onclick: () => {
                    activeTab = "trigger";
                    searchResultsHost.hidden = true;
                    draw(snap);
                  },
                }, el("span", { class: "sa-badge tone-muted" }, tr.source), el("span", {}, `${tr.actor}: ${tr.preview}`)));
              }
              searchResultsHost.replaceChildren(
                ...(rows.length ? rows : [el("p", { class: "hint" }, `No matches for "${q}".`)]),
              );
            });

            headerHost.replaceChildren(
              el("div", { class: "sa-title-block" },
                el("h1", {}, "Superagent"),
                el("span", { class: `sa-badge ${snap.paused ? "tone-warn" : "tone-ok"}` }, snap.paused ? "Paused" : "Active"),
                el("span", { class: "sa-badge tone-info" }, `AcceptMode: ${snap.settings?.policy?.acceptMode || "observe"}`),
                el("small", { class: "hint" }, `Policy ${snap.policyVersion || ""}`),
              ),
              el("div", { class: "sa-top-actions" },
                searchInp,
                button("+ Trigger", () => openSimulateTriggerModal(app, refresh), "quiet"),
                button("Main Session", async () => {
                  const res = await rpc("Superagent", "openMainSession", []);
                  if (res?.error) return toast(res.error, true);
                  await openSessionById(res.sessionId);
                }, "quiet"),
                button(snap.paused ? "Resume" : "Pause", async () => {
                  await rpc("Superagent", "saveSettingsBlock", ["pause", { paused: !snap.paused }]);
                  await refresh();
                }, snap.paused ? "primary" : "quiet"),
                button("?", () => openGlossaryModal(app), "quiet"),
              ),
            );

            // 2. 6-Tab Navigation (`Overview`, `Source`, `Trigger`, `Eval`, `Execution`, `Settings`)
            const tabs = [
              ["overview", `Overview${snap.attentionCount ? ` (${snap.attentionCount})` : ""}`],
              ["source", "Source"],
              ["trigger", `Trigger (${snap.overview?.flow?.received || 0})`],
              ["eval", `Eval (${snap.overview?.flow?.judged || 0})`],
              ["execution", `Execution (${(snap.tickets || []).length})`],
              ["settings", "Settings"],
            ];
            tabBar.replaceChildren(
              ...tabs.map(([id, label]) =>
                button(
                  label,
                  () => {
                    activeTab = id;
                    draw(snap);
                  },
                  activeTab === id ? "primary" : "quiet",
                ),
              ),
            );

            // 3. Active Tab Body
            if (activeTab === "overview") {
              const ov = snap.overview || {};
              const personList = ov.person || [];
              const problems = ov.problems || [];
              bodyHost.replaceChildren(
                el("div", { class: "sa-grid-cards" },
                  el("div", { class: "sa-stat-card" },
                    el("small", {}, "Needs Your Attention"),
                    el("strong", { class: "sa-stat-num" }, String(personList.length)),
                    el("span", { class: "hint" }, "Answers, reviews, or blocked tickets"),
                  ),
                  el("div", { class: "sa-stat-card" },
                    el("small", {}, "Intake -> Eval -> Tickets"),
                    el("strong", { class: "sa-stat-num" }, `${ov.flow?.received || 0} → ${ov.flow?.judged || 0} → ${ov.flow?.accepted || 0}`),
                    el("span", { class: "hint" }, `${ov.flow?.filtered || 0} filtered by Gate`),
                  ),
                  el("div", { class: "sa-stat-card" },
                    el("small", {}, "Worker Pool"),
                    el("strong", { class: "sa-stat-num" }, `${ov.executor?.running || 0} / ${ov.executor?.maxWorkers || 4}`),
                    el("span", { class: "hint" }, `${ov.executor?.queued || 0} queued · ${ov.executor?.idle || 0} watching`),
                  ),
                ),
                problems.length
                  ? el("section", { class: "sa-section" },
                      el("h3", {}, "Setup & Operational Notices"),
                      ...problems.map((p) =>
                        el("div", { class: "sa-problem-row" },
                          el("div", { class: "grow" },
                            el("strong", {}, p.title),
                            el("p", { class: "hint" }, p.detail),
                          ),
                          p.kind === "no-source" || p.kind === "no-allowed"
                            ? button("Configure", () => {
                                activeTab = "settings";
                                settingsSubtab = p.kind === "no-source" ? "sources" : "policy";
                                draw(snap);
                              }, "quiet")
                            : null,
                        ),
                      ),
                    )
                  : null,
                el("section", { class: "sa-section" },
                  el("h3", {}, "Needs Human Attention"),
                  personList.length
                    ? el("div", { class: "sa-list" },
                        ...personList.map((t) =>
                          el("div", { class: "sa-row-card" },
                            renderStageBadge(el, t.stage),
                            el("div", { class: "grow" },
                              el("strong", {}, t.title),
                              el("p", { class: "hint" }, t.question || t.latest_summary || t.origin_thread),
                            ),
                            t.canRetry
                              ? button("Retry", async () => {
                                  await rpc("Superagent", "retryTicket", [t.id]);
                                  await refresh();
                                }, "quiet")
                              : null,
                            button("Inspect", () => {
                              selectedTicketId = t.id;
                              activeTab = "execution";
                              draw(snap);
                            }, "primary"),
                          ),
                        ),
                      )
                    : el("p", { class: "hint" }, "Zero items waiting on you — Superagent is quiet when nothing needs human intervention."),
                ),
              );
            } else if (activeTab === "source") {
              const gh = snap.sources?.github || {};
              bodyHost.replaceChildren(
                el("section", { class: "sa-section" },
                  el("div", { class: "sa-row-between" },
                    el("div", {},
                      el("h3", {}, "GitHub Repository Source"),
                      el("p", { class: "hint" }, gh.repository ? `Watching ${gh.repository} every ${gh.pollIntervalSeconds}s` : "No repository configured yet."),
                    ),
                    el("div", { class: "row" },
                      button("Poll GitHub Now", async () => {
                        const r = await rpc("Superagent", "pollGithubNow", []);
                        if (r?.error) toast(r.error, true);
                        else if (r?.polled) toast(`Polled GitHub: ${r.ingested || 0} new trigger(s).`);
                        else toast(r?.reason || "Skipped poll.");
                        await refresh();
                      }, "primary"),
                      button("Edit Source Settings", () => {
                        activeTab = "settings";
                        settingsSubtab = "sources";
                        draw(snap);
                      }, "quiet"),
                    ),
                  ),
                  snap.sources?.lastGithubError
                    ? el("p", { class: "error-text" }, snap.sources.lastGithubError)
                    : null,
                ),
                el("section", { class: "sa-section" },
                  el("h3", {}, `Active External Threads (${(snap.threads || []).length})`),
                  (snap.threads || []).length
                    ? el("div", { class: "sa-list" },
                        ...(snap.threads || []).map((th) =>
                          el("div", { class: "sa-row-card" },
                            el("span", { class: "sa-badge tone-info" }, th.key),
                            el("div", { class: "grow" },
                              el("strong", {}, th.title || th.key),
                              el("p", { class: "hint" }, `Subscribed tickets: ${(th.subscriptions || []).join(", ") || "none"} · Decisions: ${(th.decisionIds || []).length}`),
                            ),
                          ),
                        ),
                      )
                    : el("p", { class: "hint" }, "No external threads recorded yet. Use '+ Trigger' above or enable GitHub polling."),
                ),
              );
            } else if (activeTab === "trigger") {
              const items = snap.triggers || [];
              bodyHost.replaceChildren(
                el("section", { class: "sa-section" },
                  el("h3", {}, "Trigger Intake Stream"),
                  items.length
                    ? el("div", { class: "sa-list" },
                        ...items.map(({ trigger: tr, disposition: disp }) =>
                          el("div", { class: "sa-row-card" },
                            el("span", { class: `sa-badge ${disp?.filtered ? "tone-muted" : "tone-info"}` },
                              disp?.filtered ? `Filtered (${disp.filtered})` : `${tr.source}/${tr.scope}`),
                            el("div", { class: "grow" },
                              el("strong", {}, `${tr.title || tr.id} · @${tr.actor} (${tr.addressed})`),
                              el("p", { class: "hint" }, tr.text || "(empty)"),
                            ),
                            !disp?.decisionId && !disp?.filtered
                              ? button("Run AI Eval", async () => {
                                  const r = await rpc("Superagent", "dispatchEval", [tr.id]);
                                  if (r?.error) return toast(r.error, true);
                                  toast("Sent candidate to Superagent Main Session.");
                                  await openSessionById(r.sessionId);
                                }, "quiet")
                              : null,
                          ),
                        ),
                      )
                    : el("p", { class: "hint" }, "No triggers received yet."),
                ),
              );
            } else if (activeTab === "eval") {
              const decs = snap.decisions || [];
              bodyHost.replaceChildren(
                el("section", { class: "sa-section" },
                  el("h3", {}, "Gate & Model Assessment Decisions"),
                  el("p", { class: "hint" }, "In `observe` mode, positive proactive assessments record `wouldAccept` so you can audit accuracy before switching to `auto`."),
                  decs.length
                    ? el("div", { class: "sa-list" },
                        ...decs.map((d) =>
                          el("div", { class: "sa-row-card" },
                            el("span", {
                              class: `sa-badge ${d.kind === "accept" ? "tone-ok" : d.wouldAccept ? "tone-warn" : "tone-muted"}`,
                            }, d.wouldAccept ? "observe: wouldAccept" : d.kind),
                            el("div", { class: "grow" },
                              el("strong", {}, `${d.thread} · Policy ${d.policyVersion}`),
                              el("p", { class: "hint" }, d.reason),
                            ),
                            d.ticketId
                              ? button(d.ticketId, () => {
                                  selectedTicketId = d.ticketId;
                                  activeTab = "execution";
                                  draw(snap);
                                }, "quiet")
                              : null,
                          ),
                        ),
                      )
                    : el("p", { class: "hint" }, "No Eval decisions recorded yet."),
                ),
              );
            } else if (activeTab === "execution") {
              const tickets = snap.tickets || [];
              const activeTicket = tickets.find((t) => t.id === selectedTicketId) || tickets[0] || null;
              if (activeTicket) selectedTicketId = activeTicket.id;

              bodyHost.replaceChildren(
                tickets.length
                  ? el("div", { class: "sa-split-execution" },
                      el("div", { class: "sa-ticket-list" },
                        ...tickets.map((t) =>
                          el("button", {
                            type: "button",
                            class: "sa-ticket-item" + (activeTicket?.id === t.id ? " active" : ""),
                            onclick: () => {
                              selectedTicketId = t.id;
                              draw(snap);
                            },
                          },
                            el("div", { class: "sa-row-between" },
                              renderStageBadge(el, t.stage),
                              el("small", {}, t.short_name),
                            ),
                            el("strong", {}, t.title),
                            el("small", { class: "hint" }, `${t.origin_thread} · ${t.mandate?.strength || "explicit"}`),
                          ),
                        ),
                      ),
                      activeTicket
                        ? el("div", { class: "sa-ticket-detail" },
                            el("div", { class: "sa-row-between" },
                              el("div", {},
                                el("h2", {}, activeTicket.title),
                                el("p", { class: "hint" }, `${activeTicket.id} · Thread ${activeTicket.origin_thread} · Mandate: ${activeTicket.mandate?.strength} (Policy ${activeTicket.mandate?.policyVersion})`),
                              ),
                              renderStageBadge(el, activeTicket.stage),
                            ),
                            activeTicket.question
                              ? el("div", { class: "sa-attention-callout" },
                                  el("strong", {}, "Question for Human Operator:"),
                                  el("p", {}, activeTicket.question),
                                )
                              : null,
                            el("div", { class: "row sa-ticket-actions" },
                              button("Dispatch Worker Turn", async () => {
                                const r = await rpc("Superagent", "openWorkerSession", [activeTicket.id, true]);
                                if (r?.error) return toast(r.error, true);
                                toast(`Dispatched worker session for ${activeTicket.short_name}.`);
                                await openSessionById(r.sessionId);
                              }, "primary"),
                              button("Open Worker Session", async () => {
                                const r = await rpc("Superagent", "openWorkerSession", [activeTicket.id, false]);
                                if (r?.error) return toast(r.error, true);
                                await openSessionById(r.sessionId);
                              }, "quiet"),
                              activeTicket.canRetry
                                ? button("Retry Assignment", async () => {
                                    const r = await rpc("Superagent", "retryTicket", [activeTicket.id]);
                                    if (r?.error) return toast(r.error, true);
                                    await refresh();
                                  }, "quiet")
                                : null,
                              !activeTicket.ended
                                ? button("Stop & Withdraw Mandate", async () => {
                                    const r = await rpc("Superagent", "stopTicket", [activeTicket.id, "Stopped by operator"]);
                                    if (r?.error) return toast(r.error, true);
                                    toast("Ticket stopped and Mandate withdrawn.");
                                    await refresh();
                                  }, "quiet")
                                : null,
                            ),
                            el("h4", {}, `Assignments (${(activeTicket.assignments || []).length})`),
                            ...(activeTicket.assignments || []).map((asg) =>
                              el("div", { class: "sa-assignment-card" },
                                el("strong", {}, `${asg.id} · ${asg.status}`),
                                el("p", { class: "hint" }, `Work dir: ${asg.directory}`),
                                ...(asg.reports || []).map((rep) =>
                                  el("div", { class: "sa-report-item" },
                                    el("span", { class: "sa-badge tone-info" }, rep.state),
                                    el("span", {}, rep.summary),
                                  ),
                                ),
                              ),
                            ),
                          )
                        : null,
                    )
                  : empty("No Tickets Yet", "Submit or poll a Trigger and accept it to spin up an isolated Worker Ticket."),
              );
            } else if (activeTab === "settings") {
              const subtabs = [
                ["policy", "Eval & Policy"],
                ["sources", "Sources (GitHub)"],
                ["execution", "Execution Pool"],
                ["prompts", "Role Prompts"],
              ];
              const subBar = el("div", { class: "cordis-segmented", style: "margin-bottom:14px" },
                ...subtabs.map(([id, label]) =>
                  button(label, () => {
                    settingsSubtab = id;
                    draw(snap);
                  }, settingsSubtab === id ? "primary" : "quiet"),
                ),
              );

              if (settingsSubtab === "policy") {
                const pol = JSON.parse(JSON.stringify(snap.settings?.policy || {}));
                const ghPol = pol.sources?.github || { allowed: [], bots: [], admission: {} };
                const modeSel = select(
                  [
                    ["off", "off — Only explicit requests from allowed senders"],
                    ["observe", "observe — Evaluate proactive candidates & log wouldAccept (Default)"],
                    ["auto", "auto — Automatically open Tickets for matched proactive rules"],
                  ],
                  pol.acceptMode || "observe",
                  () => {},
                );
                const allowedInp = el("input", {
                  value: (ghPol.allowed || []).map((p) => p.id).join(", "),
                  placeholder: "maintainer, alice, bob",
                });
                const botsInp = el("input", {
                  value: (ghPol.bots || []).join(", "),
                  placeholder: "dependabot[bot], github-actions[bot]",
                });
                const rulesJson = el("textarea", {
                  rows: 6,
                  value: JSON.stringify(pol.rules || [], null, 2),
                });
                bodyHost.replaceChildren(
                  subBar,
                  el("section", { class: "sa-section" },
                    el("h3", {}, "Authorization Policy & Proactive Rules"),
                    field("AcceptMode", modeSel),
                    field("Allowed GitHub Senders (comma-separated usernames)", allowedInp),
                    field("Evaluated Bot Accounts (comma-separated)", botsInp),
                    field("Proactive Rules (JSON array of {id, name, instruction, enabled})", rulesJson),
                    button("Save Policy (Bumps SHA-256 policyVersion)", async () => {
                      let parsedRules;
                      try {
                        parsedRules = JSON.parse(rulesJson.value);
                      } catch (e) {
                        return toast("Invalid Rules JSON: " + e.message, true);
                      }
                      pol.acceptMode = modeSel.value;
                      pol.rules = parsedRules;
                      pol.sources = pol.sources || {};
                      pol.sources.github = {
                        ...ghPol,
                        allowed: allowedInp.value.split(",").map((s) => s.trim()).filter(Boolean).map((id) => ({ id, name: id })),
                        bots: botsInp.value.split(",").map((s) => s.trim()).filter(Boolean),
                      };
                      const r = await rpc("Superagent", "saveSettingsBlock", ["policy", pol]);
                      if (r?.error) return toast(r.error, true);
                      toast(`Saved Policy (${r.policyVersion}).`);
                      await refresh();
                    }, "primary"),
                  ),
                );
              } else if (settingsSubtab === "sources") {
                const gh = { ...(snap.settings?.github || {}) };
                const enCb = el("input", { type: "checkbox", checked: Boolean(gh.enabled) });
                const repoInp = el("input", { value: gh.repository || "", placeholder: "owner/repo" });
                const acctInp = el("input", { value: gh.agentAccount || "", placeholder: "bot-username (dropped as self-messages)" });
                const pollInp = el("input", { type: "number", min: "15", max: "3600", value: String(gh.pollIntervalSeconds || 60) });
                bodyHost.replaceChildren(
                  subBar,
                  el("section", { class: "sa-section" },
                    el("h3", {}, "GitHub Watcher Configuration"),
                    el("label", { class: "mcp-checkbox" }, enCb, " Enable GitHub repository polling (`gh api`)"),
                    field("Repository (`owner/repo`)", repoInp),
                    field("Agent GitHub Account", acctInp),
                    field("Poll Interval (seconds)", pollInp),
                    button("Save GitHub Source", async () => {
                      const next = {
                        ...gh,
                        enabled: enCb.checked,
                        repository: repoInp.value.trim(),
                        agentAccount: acctInp.value.trim(),
                        pollIntervalSeconds: Math.max(15, Number(pollInp.value) || 60),
                      };
                      const r = await rpc("Superagent", "saveSettingsBlock", ["github", next]);
                      if (r?.error) return toast(r.error, true);
                      toast("Saved GitHub Source configuration.");
                      await refresh();
                    }, "primary"),
                  ),
                );
              } else if (settingsSubtab === "execution") {
                const ex = { ...(snap.settings?.executor || {}) };
                const maxInp = el("input", { type: "number", min: "1", max: "16", value: String(ex.maxWorkers || 4) });
                const rootInp = el("input", { value: ex.workRoot || "" });
                bodyHost.replaceChildren(
                  subBar,
                  el("section", { class: "sa-section" },
                    el("h3", {}, "Executor Worker Pool"),
                    field("Max Concurrent Workers (1–16)", maxInp),
                    field("Isolated Worker Root Directory", rootInp),
                    button("Save Executor Pool", async () => {
                      const r = await rpc("Superagent", "saveSettingsBlock", ["executor", {
                        ...ex,
                        maxWorkers: Math.max(1, Math.min(16, Number(maxInp.value) || 4)),
                        workRoot: rootInp.value.trim(),
                      }]);
                      if (r?.error) return toast(r.error, true);
                      toast("Saved Executor settings.");
                      await refresh();
                    }, "primary"),
                  ),
                );
              } else {
                const pr = JSON.parse(JSON.stringify(snap.prompts?.config || {}));
                const sharedTa = el("textarea", { rows: 3, value: pr.sharedNotes || "", placeholder: "Repository conventions injected into both Main and Worker prompts…" });
                const orchTa = el("textarea", { rows: 6, value: pr.orchestrator?.instructions || "" });
                const workTa = el("textarea", { rows: 6, value: pr.worker?.instructions || "" });
                bodyHost.replaceChildren(
                  subBar,
                  el("section", { class: "sa-section" },
                    el("h3", {}, `Role Prompts (Revision ${snap.prompts?.revision || 1})`),
                    field("Shared Repository Notes", sharedTa),
                    field("Main Orchestrator Instructions (`dsh-superagent-router`)", orchTa),
                    field("Executor Worker Instructions (`dsh-superagent-worker`)", workTa),
                    button("Save Role Prompts", async () => {
                      pr.sharedNotes = sharedTa.value;
                      pr.orchestrator.instructions = orchTa.value;
                      pr.worker.instructions = workTa.value;
                      const r = await rpc("Superagent", "savePrompts", [pr]);
                      if (r?.error) return toast(r.error, true);
                      toast(`Saved Role Prompts (rev ${r.revision}).`);
                      await refresh();
                    }, "primary"),
                    el("details", { style: "margin-top:12px" },
                      el("summary", {}, "Preview Composed Main Session System Prompt"),
                      plainBody("```markdown\n" + (snap.prompts?.orchestratorFull || "") + "\n```"),
                    ),
                  ),
                );
              }
            }
          };

          root.append(headerHost, searchResultsHost, tabBar, bodyHost);
          refresh();
          return root;
        },
      });
    },
  });
})();
