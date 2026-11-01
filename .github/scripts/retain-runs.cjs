'use strict';

const KEEP = 10;
const integer = (value) => Number.isSafeInteger(value) && value > 0;

function planRetention(workflows, currentRunId) {
  const candidates = [];
  const summaries = workflows.map(({ id, name, runs }) => {
    if (!integer(id)) throw new Error('Invalid workflow ID');
    const unique = new Map();
    for (const run of runs) {
      if (!integer(run.id) || run.workflow_id !== id || !Number.isFinite(Date.parse(run.created_at))) {
        throw new Error(`Invalid run in workflow ${id}`);
      }
      unique.set(run.id, run);
    }
    const sorted = [...unique.values()].sort((a, b) =>
      Date.parse(b.created_at) - Date.parse(a.created_at) || b.id - a.id);
    const old = sorted.slice(KEEP);
    const eligible = old.filter((run) => run.status === 'completed' && run.id !== currentRunId);
    candidates.push(...eligible);
    return {
      id, name, total: sorted.length,
      kept: sorted.slice(0, KEEP).map((run) => run.id),
      protected: old.filter((run) => run.status !== 'completed' || run.id === currentRunId).map((run) => run.id),
      candidates: eligible.length,
    };
  });
  candidates.sort((a, b) => Date.parse(a.created_at) - Date.parse(b.created_at) || a.id - b.id);
  return { workflows: summaries, candidates };
}

function rateDelay(error, attempt, now) {
  const headers = error.response?.headers ?? {};
  const status = error.status;
  if (status !== 429 && !(status === 403 &&
      (headers['retry-after'] !== undefined || headers['x-ratelimit-remaining'] === '0' ||
       /rate.?limit|abuse/i.test(error.message)))) return null;
  const waits = [];
  if (headers['retry-after'] !== undefined) {
    const value = headers['retry-after'];
    const delay = /^\d+(?:\.\d+)?$/.test(value) ? Number(value) * 1000 : Date.parse(value) - now;
    if (!Number.isFinite(delay)) throw new Error('Invalid GitHub Retry-After header');
    waits.push(delay);
  }
  if (headers['x-ratelimit-remaining'] === '0') {
    const reset = Number(headers['x-ratelimit-reset']);
    if (!Number.isFinite(reset) || reset <= 0) throw new Error('Missing GitHub rate-limit reset');
    waits.push(reset * 1000 - now + 1000);
  }
  return Math.max(1000, ...(waits.length ? waits : [60_000 * 2 ** attempt]));
}

async function retainRuns({ github, repo, currentRunId, dryRun = true, maxDeletions = 100,
  sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms)), now = Date.now,
  log = () => {} }) {
  if (!repo || ![repo.owner, repo.repo].every((part) =>
    typeof part === 'string' && /^[\w.-]+$/.test(part) && part !== '.' && part !== '..')) {
    throw new Error('An explicit owner/repository is required');
  }
  if (!integer(maxDeletions) || maxDeletions > 500) throw new Error('Deletion limit must be 1..500');
  if (currentRunId !== undefined && !integer(currentRunId)) throw new Error('Invalid current run ID');
  const base = `/repos/${repo.owner}/${repo.repo}/actions`;
  const deadline = now() + 15 * 60_000;
  let lastRequest = -Infinity;
  let lastMutation = -Infinity;
  let remainingRequests = Infinity;

  async function request(method, path, beforeAttempt) {
    for (let attempt = 0; attempt < 3; attempt++) {
      const delay = Math.max(0, lastRequest + 250 - now(),
        method === 'DELETE' ? lastMutation + 1000 - now() : 0);
      if (now() + delay >= deadline) throw new Error('Retention time budget exhausted');
      if (delay) await sleep(delay);
      if (beforeAttempt) await beforeAttempt();
      if (now() >= deadline) throw new Error('Retention time budget exhausted');
      lastRequest = now();
      if (method === 'DELETE') lastMutation = now();
      try {
        const response = await github.request(`${method} ${path}`);
        if (method === 'DELETE' && response.status !== 204) throw new Error('Unexpected GitHub deletion response');
        const remaining = response.headers?.['x-ratelimit-remaining'];
        if (remaining !== undefined && /^\d+$/.test(remaining)) remainingRequests = Number(remaining);
        return response;
      } catch (error) {
        const wait = rateDelay(error, attempt, now());
        if (wait === null || attempt === 2) throw error;
        if (wait > 120_000 || now() + wait >= deadline) {
          throw new Error(`GitHub cooldown exceeds this retention batch (${Math.ceil(wait / 1000)}s)`);
        }
        log(`GitHub rate limit: waiting ${Math.ceil(wait / 1000)}s`);
        await sleep(wait);
      }
    }
  }

  async function pages(path, field) {
    const values = new Map();
    let expected;
    let repositoryId;
    let next = `${path}?per_page=100`;
    const visited = new Set();
    while (next) {
      if (visited.has(next) || visited.size >= 100) throw new Error('Invalid or excessive pagination');
      visited.add(next);
      const response = await request('GET', next);
      if (!Array.isArray(response.data[field])) throw new Error(`Missing GitHub ${field} page`);
      expected ??= response.data.total_count;
      const repository = response.data[field][0]?.repository;
      if (repository) {
        if (repository.full_name?.toLowerCase() !== `${repo.owner}/${repo.repo}`.toLowerCase() ||
            !integer(repository.id) || (repositoryId !== undefined && repositoryId !== repository.id)) {
          throw new Error('Unexpected repository in GitHub snapshot');
        }
        repositoryId = repository.id;
      }
      for (const item of response.data[field]) {
        if (!integer(item.id)) throw new Error(`Invalid GitHub ${field} ID`);
        values.set(item.id, item);
      }
      const link = response.headers?.link?.match(/<([^>]+)>;\s*rel="next"/);
      next = undefined;
      if (link) {
        const url = new URL(link[1]);
        const canonical = integer(repositoryId) && url.pathname === `/repositories/${repositoryId}/actions/runs`;
        if (url.origin !== 'https://api.github.com' || (url.pathname !== `${base}/runs` && !canonical)) {
          throw new Error('Unexpected GitHub pagination destination');
        }
        next = url.pathname + url.search;
      }
    }
    if (!Number.isSafeInteger(expected) || expected < 0 || values.size < expected) {
      throw new Error(`Incomplete GitHub ${field} snapshot`);
    }
    return [...values.values()];
  }

  const runs = await pages(`${base}/runs`, 'workflow_runs');
  const groups = new Map();
  for (const run of runs) {
    if (!integer(run.workflow_id)) throw new Error('Invalid workflow ID in repository snapshot');
    if (!groups.has(run.workflow_id)) groups.set(run.workflow_id, {
      id: run.workflow_id, name: run.name, runs: [],
    });
    groups.get(run.workflow_id).runs.push(run);
  }
  // Complete every page before deleting: changing page offsets must not skip old runs.
  const plan = planRetention([...groups.values()].sort((a, b) => a.id - b.id), currentRunId);
  // A fresh status check and deletion each cost one request; leave room for other CI jobs.
  const available = Math.max(0, Math.floor((remainingRequests - 50) / 2));
  const selected = plan.candidates.slice(0, Math.min(maxDeletions, available));
  const result = {
    dryRun, keep: KEEP, workflows: plan.workflows, candidates: plan.candidates.length,
    selected: selected.map((run) => run.id), deleted: [], missing: [], deferred: [],
    remaining: plan.candidates.length,
  };
  if (dryRun) return result;
  try {
    for (const run of selected) {
      try {
        await request('DELETE', `${base}/runs/${run.id}`, async () => {
          // Recheck after pacing and every cooldown, immediately before the mutation.
          const current = (await request('GET', `${base}/runs/${run.id}`)).data;
          if (current.id !== run.id || current.workflow_id !== run.workflow_id) {
            throw new Error(`GitHub run identity changed: ${run.id}`);
          }
          if (current.status !== 'completed' || current.id === currentRunId || current.run_attempt !== run.run_attempt) {
            throw Object.assign(new Error(`Run ${run.id} became active or was rerun`), { deferred: true });
          }
        });
        result.deleted.push(run.id);
        result.remaining--;
        log(`Deleted workflow run ${run.id} (workflow ${run.workflow_id}), including logs and artifacts`);
      } catch (error) {
        if (error.deferred) {
          result.deferred.push(run.id);
          continue;
        }
        if (error.status !== 404) throw error;
        result.missing.push(run.id);
        result.remaining--;
      }
    }
  } catch (error) {
    error.retention = result;
    throw error;
  }
  return result;
}

let cliToken;
async function ghRequest(route) {
  const [method, path] = route.split(' ', 2);
  const url = new URL(path, 'https://api.github.com');
  if (!['GET', 'DELETE'].includes(method) || url.origin !== 'https://api.github.com' ||
      url.username || url.password || url.hash || !/^\/(repos|repositories)\//.test(url.pathname)) {
    throw new Error('Only official GitHub Actions API requests are allowed');
  }
  cliToken ??= (async () => {
    const http = require('node:http');
    const httpProxy = process.env.https_proxy || process.env.HTTPS_PROXY || process.env.http_proxy || process.env.HTTP_PROXY;
    if ((process.env.all_proxy || process.env.ALL_PROXY) &&
        !httpProxy) {
      throw new Error('Provide an HTTP(S) proxy; the built-in CLI transport does not support SOCKS-only configuration');
    }
    if (httpProxy) {
      if (typeof http.setGlobalProxyFromEnv !== 'function') throw new Error('CLI proxy support requires Node.js 24.14+');
      http.setGlobalProxyFromEnv();
    }
    return new Promise((resolve, reject) => {
      require('node:child_process').execFile('gh', ['auth', 'token', '--hostname', 'github.com'],
        { maxBuffer: 64 * 1024, timeout: 60_000, killSignal: 'SIGTERM' }, (error, stdout) => {
          const token = stdout?.trim();
          if (error || !token || /\s/.test(token)) reject(new Error('Unable to read GitHub CLI credentials'));
          else resolve(token);
        });
    });
  })();
  const token = await cliToken;
  let response;
  try {
    response = await fetch(url, { method, redirect: 'error', signal: AbortSignal.timeout(60_000),
      headers: { accept: 'application/vnd.github+json', authorization: `Bearer ${token}`,
        'user-agent': 'script-hub-retention', 'x-github-api-version': '2026-03-10' } });
  } catch {
    throw new Error('GitHub API connection failed or timed out');
  }
  const status = response.status;
  const headers = Object.fromEntries(response.headers);
  let data;
  try {
    data = status === 204 ? null : await response.json();
  } catch {
    throw new Error('Invalid or incomplete GitHub API response');
  }
  if (status >= 400) {
    const message = String(data?.message ?? 'request failed').split(token).join('[redacted]');
    const error = new Error(`GitHub ${status}: ${message}`);
    error.status = status;
    error.response = { headers };
    throw error;
  }
  return { status, headers, data };
}

async function cli(args) {
  let repo;
  let dryRun = true;
  let maxDeletions = 100;
  for (let index = 0; index < args.length; index++) {
    if (args[index] === '--repo') {
      const parts = (args[++index] ?? '').split('/');
      if (parts.length !== 2) throw new Error('Use --repo owner/repository');
      repo = { owner: parts[0], repo: parts[1] };
    } else if (args[index] === '--max-deletions') {
      maxDeletions = Number(args[++index]);
    } else if (args[index] === '--apply') {
      dryRun = false;
    } else {
      throw new Error(`Unknown argument: ${args[index]}`);
    }
  }
  return retainRuns({ github: { request: ghRequest }, repo, dryRun, maxDeletions, log: console.error });
}

module.exports = { planRetention, rateDelay, retainRuns, ghRequest };
if (require.main === module) {
  cli(process.argv.slice(2)).then((result) => console.log(JSON.stringify(result))).catch((error) => {
    if (error.retention) console.error(JSON.stringify(error.retention));
    console.error(error.message);
    process.exitCode = 1;
  });
}
