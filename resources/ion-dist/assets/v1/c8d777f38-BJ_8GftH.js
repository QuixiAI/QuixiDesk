import{d as e}from"./vendor-intl-DYfox5Co.js";var t=[{id:"briefing",name:e({defaultMessage:"Briefing",id:"Fa9/R7HZD+"}),description:e({defaultMessage:"Summary of your calendar, emails, and messages.",id:"L5fSntp2ig"}),icon:"Sun",category:"communication",trigger:{kind:"schedule",defaultCron:"18 12 * * 1-5"},suggestedConnectors:["Google Calendar","Gmail","Slack"],prompt:e({defaultMessage:`Generate a briefing to help me catch up. Include:

1. Schedule \u2014 List upcoming meetings and events with times, attendees, and any preparation needed
2. Important emails \u2014 Summarize unread emails that need attention, grouped by urgency
3. Messages requiring response \u2014 Flag any direct messages or mentions that need a reply
4. Action items \u2014 List any pending tasks or follow-ups from recent activity

Keep the briefing concise and scannable. If there\u2019s nothing notable in a section, skip it rather than saying \u201Cnothing to report.\u201D`,id:"hgi9CJwnsD"})},{id:"email-triage",name:e({defaultMessage:"Email triage",id:"3iyTdW9juF"}),description:e({defaultMessage:"Categorize and prioritize your inbox, with draft responses for urgent items.",id:"gtm9a/tT6v"}),icon:"Mailbox",category:"communication",trigger:{kind:"schedule",defaultCron:"52 14 * * 1-5"},suggestedConnectors:["Gmail"],prompt:e({defaultMessage:`Review my inbox for new and unread emails. For each email:

1. Categorize into: Urgent, Action Needed, FYI, or Low Priority
2. Summarize the key point in one sentence
3. Draft a response for any emails marked Urgent or Action Needed

Present results grouped by category, with Urgent items first. For drafted responses, keep the tone professional and concise. If the inbox is empty or all caught up, confirm that briefly.`,id:"tdBRyW7UHT"})},{id:"system-health-check",name:e({defaultMessage:"System health check",id:"RdPldf0Fji"}),description:e({defaultMessage:"Monitor infrastructure and services for errors, outages, and performance issues.",id:"KvFk8QuGe7"}),icon:"Activity",category:"monitoring",trigger:{kind:"schedule",defaultCron:"47 11 * * *"},suggestedConnectors:["PagerDuty","Datadog","Sentry"],prompt:e({defaultMessage:`Perform a health check across connected monitoring services. Report:

1. Active incidents \u2014 Any ongoing outages or incidents with severity and status
2. Errors and warnings \u2014 New errors or warning patterns since the last check
3. Performance anomalies \u2014 Unusual latency, error rates, or resource usage
4. Resolved items \u2014 Incidents or alerts that were resolved since the last check

Prioritize by severity. For each issue, include the service affected and a direct link if available. If all systems are healthy, confirm with a brief \u201CAll systems nominal\u201D message.`,id:"BqRfqNX+Di"})},{id:"issue-triage",name:e({defaultMessage:"Issue triage",id:"GlMj2BEvru"}),description:e({defaultMessage:"Review and categorize incoming issues, bugs, and feature requests.",id:"uj+aroeUVk"}),icon:"Tasks",category:"productivity",trigger:{kind:"schedule",defaultCron:"23 15 * * 1-5"},suggestedConnectors:["Linear"],prompt:e({defaultMessage:`Review new and unassigned issues across connected project trackers. For each issue:

1. Categorize as: Bug, Feature Request, Improvement, or Question
2. Assess priority based on content: Critical, High, Medium, or Low
3. Suggest assignment if the issue clearly relates to a specific team or area
4. Flag duplicates if similar issues already exist

Present a summary table of all triaged issues, then detailed notes for Critical and High priority items. If there are no new issues, confirm briefly.`,id:"eHq4NShwPE"})},{id:"pr-review-digest",name:e({defaultMessage:"PR review digest",id:"g3kQxNOL13"}),description:e({defaultMessage:"Overview of open PRs, review status, and what needs attention.",id:"5JbTL6LpVU"}),icon:"GitPullRequest",category:"monitoring",trigger:{kind:"schedule",defaultCron:"49 17 * * 1-5"},prompt:e({defaultMessage:`Review all open pull requests across the connected repositories. For each PR, report:

1. Title and author
2. How long it\u2019s been open
3. Review status \u2014 approved, changes requested, or awaiting review
4. CI status \u2014 passing, failing, or pending
5. Merge conflicts \u2014 whether the branch is up to date

Highlight any PRs that have been open for more than 3 days or have failing checks. Sort by oldest first. If there are no open PRs, confirm briefly.`,id:"MpTZOA62Yt"})},{id:"dependency-update-check",name:e({defaultMessage:"Dependency update check",id:"KVaaV6V6h1"}),description:e({defaultMessage:"Scan for outdated packages, security patches, and breaking changes.",id:"Lnbw3pq+rU"}),icon:"ShieldSearch",category:"monitoring",trigger:{kind:"schedule",defaultCron:"16 18 * * 1"},prompt:e({defaultMessage:`Check all dependencies in the connected repositories for available updates. For each outdated dependency:

1. Current version vs latest version
2. Update type \u2014 major, minor, or patch
3. Breaking changes noted in changelogs
4. Security advisories \u2014 any known CVEs or vulnerabilities

Prioritize security-related updates. Group results by repository and severity. If everything is up to date, confirm briefly.`,id:"xnkYlgu5Sv"})},{id:"release-notes-drafter",name:e({defaultMessage:"Release notes drafter",id:"neecnpXH1U"}),description:e({defaultMessage:"Draft user-facing release notes each time a PR merges to the main branch.",id:"gKO8vMZH0A"}),icon:"Scroll",category:"reporting",trigger:{kind:"github",events:["pull_request.closed"]},prompt:e({defaultMessage:`A pull request was just merged to the main branch. Draft a release-notes entry for it.

1. Read the PR title, description, and diff. Identify the user-visible change \u2014 a new feature, a bug fix, a breaking change, or an internal-only refactor.
2. If the change is internal-only (tests, CI, build tooling, comments, dead-code removal, pure refactors), stop and post a comment saying \u201CInternal change \u2014 no release note.\u201D Do not draft an entry for these.
3. Otherwise, draft a one-paragraph entry written for end users, not contributors. Use the imperative voice. Avoid jargon and internal codenames. Do not mention file paths, function names, or commit SHAs.
4. Classify the entry under one of: New, Improved, Fixed, or Breaking.
5. If this is a breaking change, explicitly list what users need to do to migrate.
6. Post the draft as a PR comment. Include a one-line \u201Csuggested changelog location\u201D hint if the repo has an obvious CHANGELOG.md or docs/changelog path.

Keep drafts short \u2014 2 to 4 sentences. Long release notes don\u2019t get read.`,id:"zGpLnXNFT1"})},{id:"flaky-test-tracker",name:e({defaultMessage:"Flaky test tracker",id:"kLH2DJIS8k"}),description:e({defaultMessage:"Find tests that pass and fail intermittently across recent CI runs.",id:"drxqOcB89Y"}),icon:"Beaker",category:"monitoring",trigger:{kind:"schedule",defaultCron:"54 15 * * 1"},prompt:e({defaultMessage:`Identify flaky tests \u2014 tests that both passed and failed on the same code over the past week.

1. Pull CI workflow runs from the last 7 days on the default branch. Focus on test jobs.
2. For each test failure, note the test name, file, and the commit it ran against. A test is flaky if it failed on a commit where the same test also passed (in a retry, a re-run, or a later run on the same SHA).
3. Rank flaky tests by flake rate (failures \u00F7 total runs) and total time wasted on retries.
4. For the top 5, include: test name and file path, flake rate, a link to one failing and one passing run on the same commit, and the failure message pattern if it\u2019s consistent.
5. Note any tests that are newly flaky this week vs. ones that appeared in last week\u2019s report.
6. For the #1 flakiest test, attempt a fix: read the test and the code it exercises, identify the likely cause (race condition, unawaited promise, time/order dependence, shared state, network reliance), apply a targeted fix, verify it passes locally across several runs, and open a pull request titled \u201Cfix(flaky): '<test name>'\u201D with the root-cause analysis in the description. If you can\u2019t determine a safe fix, open a draft PR that quarantines the test (skip with a comment linking this report) instead.

Present as a ranked table followed by per-test detail, then a link to the fix PR. If no flaky tests are found, confirm that briefly along with the total number of test runs scanned.`,id:"9rAo4EFKjq"})}];function n(e){return t.find(t=>t.id===e)}export{n,t};