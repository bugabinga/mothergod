// Telegram webhook -> mechanical command or BDFL wake (issue #5). The
// bot's webhook points here; deploy-telegram-worker.yml sets it. Slash
// commands read or mutate existing GitHub state and answer immediately.
// Everything else follows the original path: authenticate Telegram,
// store the operator's update in KV (twice: the inbox to work through,
// the chat log to remember), show the "typing..." indicator, and under
// it a draft of what the run is doing, until an answer goes out, then
// dispatch the BDFL. No model runs in this worker.

// The chat log: one KV key holding the last KEEP turns of operator
// conversation, the only memory that outlives a run. Both sides are
// written by whoever knows the message happened, the operator's here and
// the bot's in .github/scripts/tg-send, and neither by an agent by hand,
// because bookkeeping an agent has to remember is bookkeeping it will
// eventually forget (#183).
const CHATLOG = "chatlog";
const KEEP = 40;
// Chars of an entry's text: 40 entries have to stay readable in one
// screenful, and the full update is in the inbox until the run drains it.
const SUMMARY = 200;
const CLOCKLOG = "clocklog";
// Two days of ticks: enough history to judge "a day of clean ticks"
// with margin, small enough to read in one KV get.
const TICKS = 48;

// sendChatAction's status expires after 5s, so the refresh sits just
// inside that.
const TYPING_TICK_MS = 4000;
// A run that dies without answering must not leave the operator watching
// a bot type forever. Past this, the indicator gives up; the reply, when
// it comes, arrives on a quiet screen instead of a lying one.
const TYPING_CAP_MS = 20 * 60 * 1000;
// sendMessageDraft's preview lives 30s; re-sending the same draft_id
// inside that window keeps it on screen. 20s cut it too close (operator
// report, 2026-09-24: the preview blanked between refreshes, then
// reappeared on the next changed draft): a cold alarm tick or Telegram's
// own delivery lag eats into a 10s margin. 10s margin is three ticks of
// slack instead of one.
const DRAFT_REFRESH_MS = 10 * 1000;
// One operator chat, so one draft: Telegram animates edits to the same
// id and the real reply removes it. Several runs may be live at once
// (#742), so the draft is every live run's text, oldest run first.
const DRAFT_ID = 1;
const DRAFT_LIMIT = 4096; // Telegram's cap on draft text
// A run id as GitHub mints it (GITHUB_RUN_ID) is digits; tg-draft falls
// back to the session id where no run number exists, so letters and
// dashes pass too. Anything longer or stranger is not ours.
const RUN_ID = /^[A-Za-z0-9_-]{1,64}$/;

/**
 * The "typing..." indicator, as a self-refreshing alarm loop.
 *
 * It lives here rather than in the answering GitHub Actions run because
 * the run is structurally unable to get it right (operator report,
 * 2026-08-23, third attempt): the run starts seconds to minutes after
 * the message it answers, so it cannot type during the wait that matters
 * most, and when the lane is busy the run that types is not the run that
 * answers. The worker is awake at the exact moment the operator's
 * message arrives, which is the moment the indicator has to start.
 *
 * One instance for one operator chat, tracking two lists, either of
 * which keeps the loop alive:
 * - `arrivals`: the wall-clock time of every message that landed and has
 *   not been answered, oldest first. A drain loop answering three queued
 *   messages must keep typing between replies one and three, which a
 *   boolean cannot express (PR #155 review). One deadline covers them
 *   all, `TYPING_CAP_MS` after the last one landed; past it they are
 *   given up on together.
 * - `runs`: every run that has said what it is doing within the cap,
 *   `{run, text, at}`, oldest first. Any seat's run, not only the one
 *   answering a message: the operator asked to watch them all (#742),
 *   and a draft costs nothing while the chat is closed. A run is given
 *   up on `TYPING_CAP_MS` after its last word, which is the only exit a
 *   cancelled run has (agent-review.yml cancels a superseded review
 *   mid-flight, before its reset step).
 *
 * Four verbs:
 * - /start, a message landed: append its arrival time.
 * - /stop, an answer went out (`.github/scripts/tg-send`): drop the
 *   oldest arrival, because a drain answers in arrival order.
 * - /reset?since=<ms>&run=<id>, a run ended (a seat workflow's last
 *   step): drop the run's own text, and with `since`, every arrival
 *   older than that run's start. `since` is the BDFL's, the one seat
 *   that answers: those were its messages, and whether it answered
 *   them, died, or was skipped by the pause guard, nobody is working
 *   on them now. The other seats reset with `run` alone, because a
 *   heartbeat ending must not silence a message the BDFL still owes
 *   (#742). A dead run's last words are not shown for it either way.
 * - /draft, body `{text, run}`, the run says what it is doing
 *   (`.github/scripts/tg-draft`, a hook on every tool call): typing
 *   with content (#733). The loop shows the live runs' texts as one
 *   Telegram message draft, an ephemeral preview the real reply
 *   replaces, so a burst of parallel hooks collapses to the last text
 *   per run. A run's first word starts the loop when nothing else has,
 *   because a scheduled wake has no /start before it.
 *
 * The `since` cutoff is the whole reason /reset is not a blunt zero. A
 * message that lands mid-run belongs to the NEXT run, which the
 * concurrency lane has already queued (agent-bdfl.yml), and zeroing it
 * would leave the operator staring at a silent screen for exactly the
 * wait this indicator exists to fill (PR #155 review, third round).
 *
 * Nothing here is trusted for long: every run's /reset clears what was
 * its own, and the cap clears the lot if a run dies without resetting.
 * Drift lives one cap at most.
 */
export class Typing {
  constructor(state, env) {
    this.state = state;
    this.env = env;
    this.commandTail = Promise.resolve();
  }

  async fetch(request) {
    const url = new URL(request.url);
    if (url.pathname === "/command") {
      let payload;
      try {
        payload = await request.json();
      } catch {
        return new Response(null, { status: 400 });
      }
      if (
        !Number.isSafeInteger(payload?.updateId)
        || typeof payload?.parsed?.name !== "string"
        || typeof payload.parsed.args !== "string"
      ) {
        return new Response(null, { status: 400 });
      }
      const pending = this.commandTail.then(async () => {
        const key = `command:${payload.updateId}`;
        const stored = await this.state.storage.get(key);
        if (typeof stored === "string") return new Response(stored);
        const body = await command(this.env, payload.parsed);
        await this.state.storage.put(key, body);
        return new Response(body);
      });
      this.commandTail = pending.then(
        () => undefined,
        () => undefined,
      );
      return pending;
    }

    const now = Date.now();
    if (url.pathname === "/draft") {
      let text;
      let run;
      try {
        ({ text, run } = await request.json());
      } catch {
        return new Response(null, { status: 400 });
      }
      if (
        typeof text !== "string" || !text.trim() || text.length > DRAFT_LIMIT
        || typeof run !== "string" || !RUN_ID.test(run)
      ) {
        return new Response(null, { status: 400 });
      }
      // Body first, storage second: from here every await is a storage
      // call, which the object's input gate serializes, so two seats'
      // hooks landing together cannot each write a list missing the
      // other's slot. A run keeps its slot, so the draft does not reorder
      // under the operator's eyes each time one run speaks.
      const runs = (await this.state.storage.get("runs")) ?? [];
      const slot = runs.findIndex((entry) => entry.run === run);
      const entry = { run, text, at: now };
      if (slot === -1) runs.push(entry);
      else runs[slot] = entry;
      await this.state.storage.put("runs", runs);
      // A loop already running keeps its own cadence; only a cold object
      // is woken, else every hook of a chatty run would tick it at once.
      if ((await this.state.storage.getAlarm()) == null) {
        await this.state.storage.setAlarm(now);
      }
      return new Response(null, { status: 204 });
    }
    const arrivals = (await this.state.storage.get("arrivals")) ?? [];
    const runs = (await this.state.storage.get("runs")) ?? [];
    if (url.pathname === "/start") {
      arrivals.push(now);
      await this.state.storage.put({
        arrivals,
        deadline: now + TYPING_CAP_MS,
      });
      await this.state.storage.setAlarm(now);
      return new Response(null, { status: 204 });
    }
    // `since` is the answering seat's: arrivals older than it are
    // settled, and a value that is not a time falls back to "now", which
    // settles every one. A reset naming a run and no `since` is a seat
    // that answers nobody (#742): it drops that run's text and leaves
    // every arrival to the run that owes them. A reset naming no run
    // drops every run's text. The safe direction is silence, never a
    // bot that types at nobody.
    const sinceRaw = url.searchParams.get("since");
    const run = url.searchParams.get("run");
    const stop = url.pathname === "/stop";
    const answers = Boolean(sinceRaw) || !run;
    const since = Number(sinceRaw);
    const cutoff = since > 0 ? since : now;
    const owed = stop ? arrivals.slice(1) : answers ? arrivals.filter((at) => at >= cutoff) : arrivals;
    const working = stop ? runs : (run ? runs.filter((entry) => entry.run !== run) : []);
    await this.settle(owed, working);
    return new Response(null, { status: 204 });
  }

  // Both lists written back, or the loop ended when both are empty:
  // nothing is owed and nobody is working.
  async settle(arrivals, runs) {
    if (arrivals.length || runs.length) {
      await this.state.storage.put({ arrivals, runs });
    } else {
      await this.state.storage.deleteAlarm();
      await this.state.storage.deleteAll();
    }
  }

  async alarm() {
    const now = Date.now();
    // Past the deadline an owed answer is given up on, so the reply, when
    // it comes, arrives on a quiet screen instead of a lying one. No
    // deadline means a stop won the race with a scheduled tick.
    const deadline = await this.state.storage.get("deadline");
    const arrivals = deadline && now < deadline
      ? ((await this.state.storage.get("arrivals")) ?? [])
      : [];
    const runs = ((await this.state.storage.get("runs")) ?? [])
      .filter((entry) => now - entry.at < TYPING_CAP_MS);
    if (!arrivals.length && !runs.length) {
      await this.state.storage.deleteAll();
      return;
    }
    await this.state.storage.put({ arrivals, runs });
    await this.telegram("sendChatAction", { action: "typing" });
    // The draft goes out when it changed or when its preview is about to
    // expire. This loop is its only sender, and it composes the text
    // from the list on every tick, so a draft or a reset that lands
    // while a send is in flight shows on the next tick.
    // Cut on a code point boundary: a slice through a surrogate pair is
    // a text Telegram may reject, and the hook's own lines never reach
    // the cap, only a caller holding the secret could.
    const joined = runs.map((entry) => entry.text).join("\n\n");
    const cut = /[\uD800-\uDBFF]$/.test(joined.slice(0, DRAFT_LIMIT)) ? DRAFT_LIMIT - 1 : DRAFT_LIMIT;
    const text = joined.slice(0, cut);
    const shown = await this.state.storage.get("shown");
    const shownAt = await this.state.storage.get("shownAt");
    if (text && (text !== shown || shownAt == null || now - shownAt >= DRAFT_REFRESH_MS)) {
      await this.telegram("sendMessageDraft", { draft_id: DRAFT_ID, text });
      await this.state.storage.put({ shown: text, shownAt: now });
    }
    await this.state.storage.setAlarm(now + TYPING_TICK_MS);
  }

  // Never throws: decoration must not kill the loop that carries it.
  async telegram(method, body) {
    await fetch(`https://api.telegram.org/bot${this.env.BOT_TOKEN}/${method}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ chat_id: this.env.OPERATOR_CHAT_ID, ...body }),
    }).catch(() => {});
  }
}

// Never throws: a failed indicator loses decoration, never data, so it
// must not abort the webhook that carries the message. It answers with
// the object's status, 502 when the object is unreachable, because the
// caller that ends the indicator is also our only probe that the object
// answers at all.
async function typing(env, verb, body) {
  try {
    const response = await env.TYPING.get(env.TYPING.idFromName("operator")).fetch(
      `https://typing.invalid${verb}`,
      body === undefined ? undefined : { method: "POST", body },
    );
    return response.status;
  } catch (error) {
    console.error("typing", verb, error);
    return 502;
  }
}

/**
 * Append the operator's side of the conversation to the chat log.
 *
 * Read-modify-write on one key with exactly two writers, this and
 * tg-send. They collide only if a message lands in the same moment a
 * reply goes out, and the loss is one line of memory, never an update:
 * the update is already under its own `u:` key. A third writer would
 * need real serialization, so do not add one.
 *
 * Never throws, for the same reason the reaction does not: this is
 * memory, and losing memory must not cost the message it is about.
 */
export async function remember(env, message) {
  try {
    const entries = parse(await env.INBOX.get(CHATLOG));
    entries.push({
      from: "operator",
      message_id: message.message_id,
      date: message.date,
      text: (message.text ?? "").slice(0, SUMMARY),
    });
    await env.INBOX.put(CHATLOG, JSON.stringify(entries.slice(-KEEP)));
  } catch (error) {
    console.error("chatlog", error);
  }
}

// A missing key and a corrupt one mean the same thing here: start a log.
// Parsing sits outside `remember`'s catch on purpose, so that a value
// somebody once hand-wrote to this key cannot be mistaken for KV being
// down and stop the log forever.
function parse(stored) {
  try {
    const log = JSON.parse(stored ?? "[]");
    return Array.isArray(log) ? log : [];
  } catch {
    return [];
  }
}

const GITHUB_API = "https://api.github.com";
const TELEGRAM_API = "https://api.telegram.org";
const TELEGRAM_LIMIT = 4096;

// Roles are the keys in agents/models.json. Workflow names are their
// committed `name:` values; dispatchability comes from each workflow's
// trigger, not from wishful routing here. The reviewer needs a PR event and
// therefore cannot be manually dispatched.
const AGENTS = {
  bdfl: { workflow: "agent-bdfl.yml", name: "agent-bdfl", dispatch: true },
  maintainer: {
    workflow: "agent-heartbeat.yml",
    name: "agent-heartbeat",
    dispatch: true,
  },
  reviewer: {
    workflow: "agent-review.yml",
    name: "agent-review",
    dispatch: false,
  },
  deslopper: {
    workflow: "agent-deslop.yml",
    name: "agent-deslop",
    dispatch: true,
  },
};

const HELP = [
  "mothergod commands",
  "/help: show this list",
  "/status: repository and fleet status",
  "/pause <hours>: pause all agents for 1–168 hours",
  "/resume: close the global pause",
  "/run <agent>: run bdfl, maintainer, or deslopper",
  "/budget: allowance use, burn rate, and projection",
  "/runs [agent]: recent agent runs",
  "/blocked: blocked-on-human items",
  "/diff <pr>: pull request diff summary",
  "/agents: each agent's latest run",
  "/models: each role's current model and effort",
  "/digest: latest operations digest",
].join("\n");

class UpstreamError extends Error {
  constructor(status = null) {
    super(status === null ? "GitHub response was malformed" : `GitHub HTTP ${status}`);
  }
}

function parseCommand(text) {
  if (typeof text !== "string" || !text.startsWith("/")) return null;
  const match = text.match(/^\/([a-z][a-z0-9_]*)(?:@[a-z0-9_]+)?(?:\s+([\s\S]*))?\s*$/i);
  if (!match) return { name: "unknown", args: "" };
  return { name: match[1].toLowerCase(), args: (match[2] ?? "").trim() };
}

function githubHeaders(env, json) {
  return {
    authorization: `Bearer ${env.GITHUB_PAT}`,
    accept: "application/vnd.github+json",
    "user-agent": "mothergod-telegram-worker",
    ...(json ? { "content-type": "application/json" } : {}),
  };
}

async function github(env, path, options = {}) {
  let response;
  try {
    response = await fetch(`${GITHUB_API}/repos/${env.GITHUB_REPO}${path}`, {
      method: options.method ?? "GET",
      headers: githubHeaders(env, options.body !== undefined),
      ...(options.body !== undefined
        ? { body: JSON.stringify(options.body) }
        : {}),
    });
  } catch {
    throw new UpstreamError();
  }
  if (!response.ok) throw new UpstreamError(response.status);
  if (options.json === false) return { response, data: null };
  try {
    return { response, data: await response.json() };
  } catch {
    throw new UpstreamError();
  }
}

function escapeHtml(text) {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

// Same visible contract as .github/scripts/tg-send: escape arbitrary API
// text, link #refs through the repository, disable previews, and reply to the
// message that asked. This worker cannot call a repository-side executable,
// so the boundary is repeated here rather than approximated with form data.
function telegramText(body, repo) {
  const render = (source) =>
    escapeHtml(source).replace(
      /(^|[^\w/#])#(\d{1,6})\b/g,
      (_, prefix, number) => `${prefix}<a href="https://github.com/${repo}/issues/${number}">#${number}</a>`,
    );
  let source = body.trim();
  let rendered = render(source);
  if (rendered.length <= TELEGRAM_LIMIT) return rendered;

  // API-authored titles, filenames, or a digest can be unexpectedly large.
  // Find the longest prefix that fits after escaping and link expansion.
  let low = 0;
  let high = source.length;
  const suffix = "\n…";
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (render(source.slice(0, middle) + suffix).length <= TELEGRAM_LIMIT) low = middle;
    else high = middle - 1;
  }
  return render(source.slice(0, low) + suffix);
}

async function reply(env, message, body) {
  let response;
  try {
    response = await fetch(`${TELEGRAM_API}/bot${env.BOT_TOKEN}/sendMessage`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        chat_id: message.chat.id,
        text: telegramText(body, env.GITHUB_REPO),
        parse_mode: "HTML",
        link_preview_options: { is_disabled: true },
        ...(message.message_id
          ? { reply_parameters: { message_id: message.message_id } }
          : {}),
      }),
    });
  } catch {
    console.error("telegram command reply failed");
    return;
  }
  if (!response.ok) console.error("telegram command reply failed", response.status);
}

function noArgs(command, args) {
  return args ? `Usage: /${command}` : null;
}

function array(value) {
  if (!Array.isArray(value)) throw new UpstreamError();
  return value;
}

function workflowRuns(value) {
  if (!value || !Array.isArray(value.workflow_runs)) throw new UpstreamError();
  return value.workflow_runs;
}

function formatTime(value) {
  const date = typeof value === "number" ? new Date(value * 1000) : new Date(value);
  if (!Number.isFinite(date.getTime())) return "unknown time";
  return date.toISOString().slice(0, 16).replace("T", " ") + " UTC";
}

function runState(run) {
  if (!run || typeof run !== "object") throw new UpstreamError();
  if (run.status !== "completed") return `⏳ ${run.status || "unknown"}`;
  if (run.conclusion === "success") return "✅ success";
  if (run.conclusion === "cancelled" || run.conclusion === "skipped") {
    return `○ ${run.conclusion}`;
  }
  return `❌ ${run.conclusion || "unknown"}`;
}

function runLine(run, role = null) {
  const who = role ?? Object.entries(AGENTS).find(([, agent]) => agent.name === run.name)?.[0] ?? run.name;
  const number = Number.isInteger(run.run_number) ? ` run ${run.run_number}` : "";
  return `${who}${number}: ${runState(run)}, ${formatTime(run.created_at)}`;
}

async function fleetRuns(env, perPage) {
  const batches = await Promise.all(
    Object.entries(AGENTS).map(async ([role, agent]) => {
      const { data } = await github(
        env,
        `/actions/workflows/${agent.workflow}/runs?per_page=${perPage}`,
      );
      return workflowRuns(data).map((run) => ({ role, run }));
    }),
  );
  return batches
    .flat()
    .sort((left, right) => Date.parse(right.run.created_at) - Date.parse(left.run.created_at));
}

function ciState(value) {
  const latest = workflowRuns(value)[0];
  if (!latest) return "CI: ○ absent";
  if (typeof latest.status !== "string") throw new UpstreamError();
  if (latest.status !== "completed") return `CI: ⏳ pending (${latest.status})`;
  if (typeof latest.conclusion !== "string") throw new UpstreamError();
  return latest.conclusion === "success"
    ? "CI: ✅ green"
    : `CI: ❌ failing (${latest.conclusion})`;
}

async function status(env, args) {
  const usage = noArgs("status", args);
  if (usage) return usage;
  const [
    { data: pauses },
    { data: pulls },
    { data: issues },
    { data: ci },
    agentRuns,
  ] = await Promise.all([
    github(env, "/issues?state=open&labels=agents-paused&per_page=1"),
    github(env, "/pulls?state=open&per_page=100"),
    github(env, "/issues?state=open&per_page=100"),
    github(env, "/actions/workflows/ci.yml/runs?per_page=1"),
    fleetRuns(env, 1),
  ]);
  const paused = array(pauses)[0];
  const openPulls = array(pulls);
  const openIssues = array(issues).filter((issue) => !issue?.pull_request);
  const active = agentRuns.filter(({ run }) => run.status !== "completed");
  if (paused && !Number.isInteger(paused.number)) throw new UpstreamError();
  const headline = paused
    ? `⏸️ Agents paused by #${paused.number}`
    : "✅ Agents active";
  const activity = active.length
    ? `Running: ${active.map(({ run }) => run.name).join(", ")}`
    : agentRuns[0]
    ? `Latest: ${runLine(agentRuns[0].run, agentRuns[0].role)}`
    : "Latest: no agent runs found";
  return `${headline}\n${ciState(ci)}\nOpen: ${openIssues.length} issues, ${openPulls.length} PRs\n${activity}`;
}

function parseHours(args) {
  if (!/^\d+$/.test(args)) return null;
  const hours = Number(args);
  return Number.isSafeInteger(hours) && hours >= 1 && hours <= 168 ? hours : null;
}

async function pause(env, args) {
  const hours = parseHours(args);
  if (hours === null) return "Usage: /pause <hours>, where hours is 1–168";
  const { data: pauses } = await github(
    env,
    "/issues?state=open&labels=agents-paused&per_page=1",
  );
  const existing = array(pauses)[0];
  if (existing && !Number.isInteger(existing.number)) throw new UpstreamError();
  if (existing) return `Already paused by #${existing.number}.`;

  const resumeAt = new Date(Date.now() + hours * 60 * 60 * 1000)
    .toISOString()
    .replace(".000", "");
  const { data: issue } = await github(env, "/issues", {
    method: "POST",
    body: {
      title: "⏸️ Agents paused by operator",
      labels: ["agents-paused"],
      body: [
        "Paused by the operator through Telegram.",
        "",
        `RESUME-AT: ${resumeAt}`,
        "",
        "Close this issue to resume earlier.",
      ].join("\n"),
    },
  });
  if (!Number.isInteger(issue?.number)) throw new UpstreamError();
  return `⏸️ Agents paused for ${hours}h by #${issue.number}, until ${formatTime(resumeAt)}.`;
}

async function resume(env, args) {
  const usage = noArgs("resume", args);
  if (usage) return usage;
  const { data: pauses } = await github(
    env,
    "/issues?state=open&labels=agents-paused&per_page=1",
  );
  const issue = array(pauses)[0];
  if (!issue) return "✅ Agents are already active.";
  if (!Number.isInteger(issue.number)) throw new UpstreamError();
  await github(env, `/issues/${issue.number}`, {
    method: "PATCH",
    body: { state: "closed", state_reason: "completed" },
  });
  return `▶️ Resumed agents by closing #${issue.number}.`;
}

async function runAgent(env, args) {
  const parts = args.split(/\s+/).filter(Boolean);
  if (parts.length !== 1 || !AGENTS[parts[0]]) {
    return "Usage: /run <agent>\nAgents: bdfl, maintainer, deslopper";
  }
  const role = parts[0];
  const agent = AGENTS[role];
  if (!agent.dispatch) return "Reviewer is event-driven and runs only for a pull request.";
  await github(env, `/actions/workflows/${agent.workflow}/dispatches`, {
    method: "POST",
    body: { ref: "main" },
    json: false,
  });
  return `▶️ Dispatched ${role}.`;
}

// The seven-day allowance, one reading per run, from the sessions database
// (ADR-0060, #891 slice 2b): session-row.py writes the reading beside the
// session row, the join gives it its instant, and the SESSIONS binding in
// wrangler.toml reads it with no token. Newest first; burn and runway read
// the newest pair inside one reset window, so a page is plenty.
const BUDGET_SQL = `SELECT s.at, a.utilization, a.resets_at
  FROM allowance a JOIN sessions s ON s.run_id = a.run_id AND s.attempt = a.attempt
  WHERE a.window = 'seven_day' AND a.resets_at IS NOT NULL
  ORDER BY s.at DESC LIMIT 100`;

function allowanceSamples(rows) {
  const samples = [];
  for (const row of array(rows)) {
    const used = Number(row?.utilization);
    const reset = Number(row?.resets_at);
    const at = Date.parse(row?.at);
    if (!(used >= 0 && used <= 1) || !(reset > 0) || !Number.isFinite(at)) continue;
    samples.push({ used, reset, at });
  }
  return samples.sort((a, b) => a.at - b.at);
}

function percent(fraction) {
  return `${(fraction * 100).toFixed(2)}%`;
}

async function budget(env, args) {
  const usage = noArgs("budget", args);
  if (usage) return usage;
  if (!env.SESSIONS) return "Budget unavailable: no sessions database bound (wrangler.toml).";
  let rows;
  try {
    ({ results: rows } = await env.SESSIONS.prepare(BUDGET_SQL).all());
  } catch (error) {
    return `Budget unavailable: sessions database unreadable: ${error?.message ?? error}`;
  }
  const samples = allowanceSamples(rows);
  if (!samples.length) return "Budget unavailable: no allowance readings in the sessions table.";

  const latest = samples.at(-1);
  const previous = samples.findLast(
    (sample) => sample.reset === latest.reset && sample.at < latest.at && sample.used <= latest.used,
  );
  const lines = [
    `Allowance: ${percent(latest.used)} used, ${percent(1 - latest.used)} remaining`,
    `Observed: ${formatTime(new Date(latest.at).toISOString())}`,
    `Reset: ${formatTime(latest.reset)}`,
  ];
  if (!previous || latest.used === previous.used) {
    lines.push("Recent burn: no measurable movement");
    lines.push("Projected exhaustion: unavailable");
    return lines.join("\n");
  }

  const elapsedHours = (latest.at - previous.at) / 3_600_000;
  const burn = (latest.used - previous.used) / elapsedHours;
  if (!(burn > 0) || !Number.isFinite(burn)) {
    lines.push("Recent burn: unavailable");
    lines.push("Projected exhaustion: unavailable");
    return lines.join("\n");
  }
  const exhaustion = latest.at + ((1 - latest.used) / burn) * 3_600_000;
  lines.push(`Recent burn: ${percent(burn)}/h over ${elapsedHours.toFixed(1)}h`);
  lines.push(
    `Projected exhaustion: ${formatTime(new Date(exhaustion).toISOString())} (${
      exhaustion < latest.reset * 1000 ? "before" : "after"
    } reset)`,
  );
  return lines.join("\n");
}

function validateAgentArg(command, args, optional) {
  const parts = args.split(/\s+/).filter(Boolean);
  if ((optional && parts.length === 0) || (parts.length === 1 && AGENTS[parts[0]])) {
    return parts[0] ?? null;
  }
  return `Usage: /${command}${optional ? " [agent]" : " <agent>"}\nAgents: ${Object.keys(AGENTS).join(", ")}`;
}

async function runs(env, args) {
  const role = validateAgentArg("runs", args, true);
  if (typeof role === "string" && role.startsWith("Usage:")) return role;
  let found;
  if (role) {
    const { data } = await github(
      env,
      `/actions/workflows/${AGENTS[role].workflow}/runs?per_page=5`,
    );
    found = workflowRuns(data).map((run) => ({ role, run }));
  } else {
    found = (await fleetRuns(env, 5)).slice(0, 5);
  }
  if (!found.length) return role ? `No runs found for ${role}.` : "No agent runs found.";
  return [
    role ? `Recent ${role} runs:` : "Recent agent runs:",
    ...found.map((entry) => runLine(entry.run, entry.role)),
  ].join("\n");
}

async function blocked(env, args) {
  const usage = noArgs("blocked", args);
  if (usage) return usage;
  const { data } = await github(
    env,
    "/issues?state=open&labels=blocked-on-human&per_page=11",
  );
  const issues = array(data);
  if (!issues.length) return "✅ No blocked-on-human items.";
  const shown = issues.slice(0, 10);
  return [
    `Blocked on human (${shown.length}${issues.length > shown.length ? "+" : ""}):`,
    ...shown.map((issue) => {
      if (!Number.isInteger(issue?.number) || typeof issue.title !== "string") throw new UpstreamError();
      return `#${issue.number} ${issue.title}`;
    }),
  ].join("\n");
}

function parsePr(args) {
  if (!/^\d+$/.test(args)) return null;
  const number = Number(args);
  return Number.isSafeInteger(number) && number > 0 ? number : null;
}

async function diff(env, args) {
  const number = parsePr(args);
  if (number === null) return "Usage: /diff <pr>";
  const [{ data: pull }, { data: changed }] = await Promise.all([
    github(env, `/pulls/${number}`),
    github(env, `/pulls/${number}/files?per_page=100`),
  ]);
  if (
    !pull
    || typeof pull.title !== "string"
    || !Number.isInteger(pull.changed_files)
    || !Number.isInteger(pull.additions)
    || !Number.isInteger(pull.deletions)
  ) {
    throw new UpstreamError();
  }
  const files = array(changed);
  const shown = files.slice(0, 12).map((file) => {
    if (
      typeof file?.filename !== "string"
      || !Number.isInteger(file.additions)
      || !Number.isInteger(file.deletions)
    ) {
      throw new UpstreamError();
    }
    return `${file.status?.slice(0, 1)?.toUpperCase() || "M"} ${file.filename} +${file.additions} −${file.deletions}`;
  });
  const more = pull.changed_files > shown.length ? [`… ${pull.changed_files - shown.length} more files`] : [];
  return [
    `PR #${number}: ${pull.title}`,
    `+${pull.additions} −${pull.deletions} across ${pull.changed_files} files`,
    ...shown,
    ...more,
  ].join("\n");
}

async function agents(env, args) {
  const usage = noArgs("agents", args);
  if (usage) return usage;
  const found = await fleetRuns(env, 1);
  return [
    "Agents:",
    ...Object.entries(AGENTS).map(([role, agent]) => {
      const latest = found.find((entry) => entry.role === role)?.run;
      return latest
        ? runLine(latest, role)
        : `${role}: no run found${agent.dispatch ? "" : " (event-driven)"}`;
    }),
  ].join("\n");
}

function decodeBase64(content) {
  const binary = atob(content.replace(/\n/g, ""));
  return new TextDecoder().decode(Uint8Array.from(binary, (char) => char.charCodeAt(0)));
}

// The JSON object inside a ledger issue body's first ```json fence, same
// contract as guard-decide.py's `_fenced` (model-limits and allowance-state
// are both written in that shape). This worker cannot call that Python file,
// so /models repeats the ladder/thrift half of its `decide()` in JS, the same
// boundary telegramText already repeats tg-send's escaping contract across.
// guard-decide.test.mjs and this file's /models tests both pin the ladder
// and thrift cases so the two cannot drift silently.
function fenced(body) {
  const match = /```json\s*([\s\S]*?)\s*```/.exec(body ?? "");
  if (!match) return null;
  try {
    return JSON.parse(match[1]);
  } catch {
    return null;
  }
}

function plainObject(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : null;
}

function blockedModels(limits, now) {
  const entries = plainObject(limits);
  const blocked = new Set();
  if (!entries) return blocked;
  for (const [model, resetAt] of Object.entries(entries)) {
    const epoch = Number(resetAt);
    if (Number.isFinite(epoch) && epoch > now) blocked.add(model);
  }
  return blocked;
}

const ALLOWANCE_WEEK = 604800;

// Mirrors allowance.py's `project`: a week-average burn projected against
// the next reset, or null for "no usable reading", which leaves the role in
// its normal tier the same way a missing ledger does.
function project(reading) {
  const observed = Number(reading?.observedAt);
  const resets = Number(reading?.resetsAt);
  const used = Number(reading?.utilization);
  if (![observed, resets, used].every(Number.isFinite)) return null;
  const elapsed = observed - (resets - ALLOWANCE_WEEK);
  const remaining = resets - observed;
  if (elapsed <= 0 || remaining <= 0) return null;
  const rate = used / elapsed;
  if (!(rate > 0)) return null;
  const exhaustsAt = observed + (1 - used) / rate;
  return exhaustsAt < resets;
}

const EFFORTS = new Set(["low", "medium", "high", "xhigh", "max"]);

// Mirrors guard-decide.py's decide(), the ladder/thrift resolution half only
// (not the pause, stale-wake, or second-gear reasons: those gate whether a
// run happens at all, not which model it would use).
function resolveModel(entry, blocked, thriftActive) {
  let chosen = plainObject(entry) ?? {};
  let thrift = false;
  if (thriftActive && plainObject(chosen.thrift)) {
    chosen = chosen.thrift;
    thrift = true;
  }
  const effort = EFFORTS.has(chosen.effort) ? chosen.effort : "";
  const ladder = Array.isArray(chosen.ladder) ? chosen.ladder : [];
  if (!ladder.length) return { model: null, effort, thrift, exhausted: false };
  const model = ladder.find((candidate) => !blocked.has(candidate));
  return model
    ? { model, effort, thrift, exhausted: false }
    : { model: null, effort, thrift, exhausted: true };
}

async function models(env, args) {
  const usage = noArgs("models", args);
  if (usage) return usage;
  const [{ data: file }, { data: limitIssues }, { data: allowanceIssues }] = await Promise.all([
    github(env, "/contents/agents/models.json"),
    github(env, "/issues?state=open&labels=model-limits&per_page=1"),
    github(env, "/issues?state=open&labels=allowance-state&per_page=1"),
  ]);
  if (typeof file?.content !== "string") throw new UpstreamError();
  let roles;
  try {
    roles = plainObject(JSON.parse(decodeBase64(file.content))?.roles);
  } catch {
    throw new UpstreamError();
  }
  if (!roles) throw new UpstreamError();

  const now = Date.now() / 1000;
  const blocked = blockedModels(fenced(array(limitIssues)[0]?.body), now);
  // Pre-#369 ledgers nest the reading under "current"; a flat object IS the
  // reading now. One ?? carries the transition, same as guard-decide.py.
  const ledger = plainObject(fenced(array(allowanceIssues)[0]?.body));
  const reading = plainObject(ledger?.current) ?? ledger;
  const thriftActive = Boolean(reading && project(reading));

  const lines = Object.entries(roles).map(([role, entry]) => {
    const { model, effort, thrift, exhausted } = resolveModel(entry, blocked, thriftActive);
    if (exhausted) return `${role}: exhausted, every rung rate-limited`;
    const label = model ?? "(action default)";
    return `${role}: ${label}${effort ? `, ${effort}` : ""}${thrift ? " [THRIFT]" : ""}`;
  });
  return [
    "Current models:",
    ...lines,
    ...(thriftActive ? ["Thrift active: allowance projected to miss the weekly reset."] : []),
  ].join("\n");
}

function lastPage(link) {
  if (!link) return null;
  const match = link.match(/[?&]page=(\d+)[^>]*>;\s*rel="last"/);
  const page = match ? Number(match[1]) : null;
  return Number.isSafeInteger(page) && page > 1 ? page : null;
}

function digestBody(body) {
  if (typeof body !== "string") throw new UpstreamError();
  return body.split(/\n---\n/)[0].trim();
}

async function digest(env, args) {
  const usage = noArgs("digest", args);
  if (usage) return usage;
  const { data: logs } = await github(env, "/issues?state=open&labels=ops-log&per_page=1");
  const issue = array(logs)[0];
  if (!issue) return "No open operations log.";
  if (!Number.isInteger(issue.number)) throw new UpstreamError();
  const path = `/issues/${issue.number}/comments?per_page=100`;
  let { response, data } = await github(env, path);
  let comments = array(data);
  const page = lastPage(response.headers.get("link"));
  if (page) {
    ({ data } = await github(env, `${path}&page=${page}`));
    comments = array(data);
  }
  const latest = comments.at(-1);
  if (!latest) return `No digest has been posted to #${issue.number}.`;
  const body = digestBody(latest.body);
  return `Latest digest (#${issue.number}, ${formatTime(latest.created_at)}):\n${body || "(empty)"}`;
}

const COMMANDS = {
  help: async (_env, args) => noArgs("help", args) ?? HELP,
  status,
  pause,
  resume,
  run: runAgent,
  budget,
  runs,
  blocked,
  diff,
  agents,
  models,
  digest,
};

async function command(env, parsed) {
  const handler = COMMANDS[parsed.name];
  try {
    return handler ? await handler(env, parsed.args) : HELP;
  } catch (error) {
    console.error(
      "telegram command failed",
      parsed.name,
      error instanceof UpstreamError ? error.message : "unexpected failure",
    );
    return `⚠️ /${handler ? parsed.name : "help"} unavailable: GitHub did not return usable data.`;
  }
}

async function commandResult(env, updateId, parsed) {
  const response = await env.TYPING.get(env.TYPING.idFromName("commands")).fetch(
    "https://typing.invalid/command",
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ updateId, parsed }),
    },
  );
  if (!response.ok) throw new Error(`command object failed: ${response.status}`);
  return response.text();
}

// The agent clock (ADR-0035). Each expression in wrangler.toml's
// [triggers] crons fires `scheduled` with the matching key here; the
// value names the seats that tick wakes. Cadence values move with
// ADR-0027's allowance lever; a new seat needs both files in one PR,
// and a changed cadence needs both keys in one PR, because a wrangler
// cron with no CLOCK key wakes nobody, silently. That pairing is a
// test, not a promise: worker.test.mjs reads wrangler.toml and asserts
// its crons are exactly these keys. Exported for that test, which
// derives every cadence fixture from here so moving the lever never
// edits a third copy of the value. GitHub's own
// `schedule:` trigger is not an alternative: its runs are attributed
// to whoever last committed the cron line, a bot actor kills them
// silently, and most clock edits here are bot-authored by design
// (incidents 2026-08-23 and 2026-08-27). A dispatch below is
// attributed to the PAT's owner by API semantics, immune to git blame.
export const CLOCK = {
  "11 */8 * * *": [
    // agent-bdfl, three times a day (ADR-0077): the tick runs the delta
    // core, and at six a day the director took two thirds of the fleet's
    // fourteen days; alarms, operator events and Telegram dispatches
    // wake it off the clock. `source: cron` lets the seat report
    // TRIGGER_EVENT=schedule downstream, telling a tick from an
    // operator dispatch.
    { workflow: "agent-bdfl.yml", inputs: { source: "cron" } },
  ],
  "22 */3 * * *": [
    // agent-heartbeat. `source: cron` marks the wake discretionary, so the
    // allowance governor may skip it (ADR-0039); a human dispatch has no
    // input and is never skipped.
    { workflow: "agent-heartbeat.yml", inputs: { source: "cron" } },
  ],
  "37 */4 * * *": [
    // agent-deslop, six times a day, off the other seats' minutes. Raised
    // from twice daily on 2026-09-24 (operator, Telegram: "deslopper does
    // not run often enough"): every one of its 14 runs that week shipped a
    // deslop PR, so slop was arriving faster than two wakes cleared it. At
    // $3 a run this is no longer the rounding error ADR-0039 left
    // ungoverned, so `source: cron` marks the wake discretionary and the
    // governor may shed it in a tight week. The fifth and last cron
    // expression the Workers Free plan allows.
    { workflow: "agent-deslop.yml", inputs: { source: "cron" } },
  ],
  "37 */12 * * *": [
    // agent-curator, twice daily (ADR-0044). `source: cron` marks the
    // wake discretionary (ADR-0039).
    { workflow: "agent-curator.yml", inputs: { source: "cron" } },
  ],
  "49 6,18 * * *": [
    // agent-herald, twice daily (ADR-0040), off the other seats'
    // minutes. `source: cron` marks the wake discretionary, so the
    // allowance governor may skip it (ADR-0039).
    // The researcher shared this tick until ADR-0062 retired the seat.
    { workflow: "agent-herald.yml", inputs: { source: "cron" } },
  ],
};

/**
 * One clock tick: wake every seat the firing cron names, then record
 * the attempt under `clocklog` so any repo-side run can verify
 * liveness from KV without Cloudflare console access. A seat that
 * fails to dispatch is recorded, never retried: the next tick is the
 * retry. The log write shares `remember`'s contract — losing memory
 * must not cost the tick.
 */
export async function tick(env, cron, at) {
  const entry = { cron, at, woke: [], failed: [] };
  for (const seat of CLOCK[cron] ?? []) {
    try {
      await github(env, `/actions/workflows/${seat.workflow}/dispatches`, {
        method: "POST",
        body: { ref: "main", ...(seat.inputs ? { inputs: seat.inputs } : {}) },
        json: false,
      });
      entry.woke.push(seat.workflow);
    } catch (error) {
      entry.failed.push(seat.workflow);
      console.error("tick", seat.workflow, error);
    }
  }
  try {
    const log = parse(await env.INBOX.get(CLOCKLOG));
    log.push(entry);
    await env.INBOX.put(CLOCKLOG, JSON.stringify(log.slice(-TICKS)));
  } catch (error) {
    console.error("clocklog", error);
  }
}

// The typing object's verbs a run may call from outside; `/start` is
// the webhook's alone, because only a message that landed may start
// the indicator.
const TYPING_VERBS = new Set(["/typing/stop", "/typing/reset", "/typing/draft"]);

export default {
  async scheduled(event, env) {
    await tick(env, event.cron, new Date(event.scheduledTime).toISOString());
  },

  async fetch(request, env) {
    if (request.method !== "POST") {
      return new Response("mothergod telegram webhook", { status: 200 });
    }
    // `/typing/stop`: one answer went out (tg-send). `/typing/reset`: a
    // run ended and owes nothing (a seat workflow's last step), which is
    // also the health probe that the object is reachable at all.
    // `/typing/draft`: the run says what it is doing (tg-draft), body
    // `{"text": ..., "run": ...}`. Authenticated with WEBHOOK_SECRET
    // under a header of its own, because the secret's role is "may talk
    // to this worker" and every caller is ours. Telegram's header name
    // is Telegram's; ours says who we are.
    const { pathname, search } = new URL(request.url);
    if (TYPING_VERBS.has(pathname)) {
      if (request.headers.get("x-mothergod-secret") !== env.WEBHOOK_SECRET) {
        return new Response(null, { status: 401 });
      }
      const status = await typing(
        env,
        pathname.replace("/typing", "") + search,
        pathname === "/typing/draft" ? await request.text() : undefined,
      );
      return new Response(null, { status });
    }
    // Telegram echoes the secret_token from setWebhook in this header;
    // a request without it is not Telegram.
    const secret = request.headers.get("x-telegram-bot-api-secret-token");
    if (secret !== env.WEBHOOK_SECRET) {
      return new Response(null, { status: 401 });
    }
    let update;
    try {
      update = await request.json();
    } catch {
      return new Response(null, { status: 200 });
    }
    const chat = update?.message?.chat?.id;
    // Non-operator chats: no action, no reply (issue #5 acceptance).
    if (
      String(chat) !== env.OPERATOR_CHAT_ID
      || !Number.isSafeInteger(update.update_id)
    ) {
      return new Response(null, { status: 200 });
    }
    // Commands are deliberately outside KV and the BDFL lane. Unknown slash
    // commands are help, not prose: a typo must not spend an agent run. The
    // command object serializes side effects and replays persisted results when
    // Telegram redelivers an update.
    const parsed = parseCommand(update.message?.text);
    if (parsed) {
      const body = await commandResult(env, update.update_id, parsed);
      await reply(env, update.message, body);
      return new Response(null, { status: 200 });
    }
    // Zero-padded key so KV's lexicographic list order is arrival order.
    const key = "u:" + String(update.update_id).padStart(12, "0");
    await env.INBOX.put(key, JSON.stringify(update));
    // Instant receipt (operator request, 2026-08-23): the 👀 reaction
    // says "stored in KV, cannot be lost", so it fires right after the
    // KV write, before the GitHub dispatch, whose API latency must not
    // delay it (PR #99 review). The BDFL switches it to ✍ on pickup;
    // its reply is the completion. Non-fatal: a failed reaction loses
    // decoration, never data.
    if (update.message.message_id) {
      await fetch(
        `https://api.telegram.org/bot${env.BOT_TOKEN}/setMessageReaction`,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            chat_id: update.message.chat.id,
            message_id: update.message.message_id,
            reaction: [{ type: "emoji", emoji: "👀" }],
          }),
        },
      ).catch(() => {});
    }
    // Eyes, then typing: the operator asked for the indicator to start
    // the moment the message is acknowledged.
    await typing(env, "/start");
    // Remembered before the wake, not after: the run that is about to
    // read this log should find it complete. Placed after the reaction
    // and the indicator, which the operator is watching for, and before
    // the dispatch, which nobody is.
    await remember(env, update.message);
    // Wake the BDFL. The dispatch carries no message text (issue #36
    // principle): it says "wake up", the run reads KV for the prose.
    // Dispatch failure is tolerable: the update is already in KV and
    // the next scheduled tick (ADR-0035) is the backstop.
    const res = await fetch(
      `https://api.github.com/repos/${env.GITHUB_REPO}/actions/workflows/agent-bdfl.yml/dispatches`,
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${env.GITHUB_PAT}`,
          accept: "application/vnd.github+json",
          "user-agent": "mothergod-telegram-worker",
          "content-type": "application/json",
        },
        body: JSON.stringify({ ref: "main" }),
      },
    );
    if (!res.ok) {
      console.error("dispatch failed", res.status, await res.text());
    }
    return new Response(null, { status: 200 });
  },
};
