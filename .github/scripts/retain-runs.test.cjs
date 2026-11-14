'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { planRetention, rateDelay, retainRuns } = require('./retain-runs.cjs');

const repo = { owner: 'owner', repo: 'repo' };
const run = (id, workflow = 1, status = 'completed') => ({
  id, workflow_id: workflow, status, run_attempt: 1,
  created_at: new Date(1_700_000_000_000 + id * 1000).toISOString(),
});
const error = (status, headers = {}, message = 'failure') =>
  Object.assign(new Error(message), { status, response: { headers } });

function fixture(groups, intercept = () => undefined) {
  let time = 1_800_000_000_000;
  const calls = [];
  const waits = [];
  const live = new Map(groups.flatMap((group) => group.runs).map((item) => [item.id, { ...item }]));
  const respond = (values, field, url) => {
    const page = Number(url.searchParams.get('page') || 1);
    return { data: { total_count: values.length, [field]: values.slice((page - 1) * 100, page * 100) },
      headers: values.length > page * 100 ? {
        link: `<https://api.github.com${url.pathname}?per_page=100&page=${page + 1}>; rel="next"`,
      } : {} };
  };
  const github = { request: async (route) => {
    const [method, path] = route.split(' ', 2);
    calls.push({ method, path, time });
    const override = await intercept({ method, path, live, calls, time });
    if (override !== undefined) return override;
    const url = new URL(path, 'https://api.github.com');
    if (url.pathname === '/repos/owner/repo/actions/runs') {
      return respond([...live.values()].sort((a, b) => b.id - a.id), 'workflow_runs', url);
    }
    const id = Number(url.pathname.match(/\/runs\/(\d+)$/)?.[1]);
    if (!live.has(id)) throw error(404);
    if (method === 'GET') return { data: { ...live.get(id) }, headers: {} };
    assert.equal(method, 'DELETE');
    live.delete(id);
    return { status: 204, data: null, headers: {} };
  } };
  return { github, calls, live, waits, options: {
    github, repo, now: () => time, sleep: async (ms) => { waits.push(ms); time += ms; },
  } };
}

test('newest ten use creation time and ID, protect active and current older runs', () => {
  const runs = Array.from({ length: 25 }, (_, i) => run(i + 1));
  runs[0].status = 'queued';
  runs[2].status = 'in_progress';
  runs[3].status = 'waiting';
  const plan = planRetention([{ id: 1, name: 'test', runs: runs.reverse() }], 2);
  assert.deepEqual(plan.workflows[0].kept, [25, 24, 23, 22, 21, 20, 19, 18, 17, 16]);
  assert.deepEqual(plan.candidates.map((item) => item.id), [5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
  assert.deepEqual(plan.workflows[0].protected, [4, 3, 2, 1]);
});

test('creation-time ties and duplicate pages do not change the retention boundary', () => {
  const runs = Array.from({ length: 12 }, (_, i) => ({ ...run(i + 1), created_at: run(1).created_at }));
  const plan = planRetention([{ id: 1, runs: [...runs, runs[11]] }]);
  assert.deepEqual(plan.candidates.map((item) => item.id), [1, 2]);
  assert.equal(plan.workflows[0].total, 12);
});

test('complete inventories include orphan workflows and refresh after deletion', async () => {
  const f = fixture([{ id: 1, name: 'first', runs: Array.from({ length: 120 }, (_, i) => run(i + 1)) },
    { id: 2, name: 'second', runs: Array.from({ length: 12 }, (_, i) => run(1000 + i, 2)) }]);
  const result = await retainRuns({ ...f.options, dryRun: false, maxDeletions: 500 });
  assert.equal(result.deleted.length, 112);
  assert.equal(result.remaining, 0);
  assert.equal(f.live.size, 20);
  const firstDelete = f.calls.findIndex((call) => call.method === 'DELETE');
  assert.equal(f.calls.slice(0, firstDelete).filter((call) => /\/runs\?/.test(call.path)).length, 2);
  const lastDelete = f.calls.findLastIndex((call) => call.method === 'DELETE');
  assert.ok(f.calls.slice(lastDelete + 1).some((call) => /\/runs\?/.test(call.path)));
  assert.ok(!f.calls.some((call) => call.path.includes('/workflows')));
  assert.equal(result.workflows.length, 2);
  const writes = f.calls.filter((call) => call.method === 'DELETE');
  assert.ok(writes.slice(1).every((call, i) => call.time - writes[i].time >= 1000));
});

test('dry run and batch limits never silently exceed the selected scope', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 15 }, (_, i) => run(i + 1)) }]);
  const preview = await retainRuns({ ...f.options, maxDeletions: 2 });
  assert.deepEqual(preview.selected, [1, 2]);
  assert.equal(preview.candidates, 5);
  assert.ok(!f.calls.some((call) => call.method === 'DELETE'));
  const applied = await retainRuns({ ...f.options, dryRun: false, maxDeletions: 2 });
  assert.deepEqual(applied.deleted, [1, 2]);
  assert.equal(applied.remaining, 3);
});

test('fresh reconciliation retains only the latest ten when runs arrive during deletion', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 12 }, (_, i) => run(i + 1)) }], ({ method, path, live }) => {
    if (method === 'DELETE' && path.endsWith('/runs/1')) live.set(13, run(13));
  });
  const result = await retainRuns({ ...f.options, dryRun: false });
  assert.deepEqual([...f.live.keys()].sort((a, b) => a - b), [4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
  assert.deepEqual(result.deleted, [1, 2, 3]);
  assert.equal(result.remaining, 0);
  assert.deepEqual(result.workflows[0].kept, [13, 12, 11, 10, 9, 8, 7, 6, 5, 4]);
});

test('a stale list cannot resurrect a run confirmed absent by its direct API', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 11 }, (_, i) => run(i + 1)) }], ({ path, live }) => {
    if (path.includes('/runs?') && !live.has(1)) {
      return { data: { total_count: 11, workflow_runs: [run(1), ...live.values()] }, headers: {} };
    }
  });
  const result = await retainRuns({ ...f.options, dryRun: false });
  assert.equal(result.refreshed, true);
  assert.deepEqual(result.deleted, [1]);
  assert.equal(result.remaining, 0);
  assert.equal(result.workflows[0].total, 10);
  assert.equal(f.calls.filter((call) => call.method === 'DELETE').length, 1);
  assert.equal(f.calls.filter((call) => call.method === 'GET' && call.path.endsWith('/runs/1')).length, 2);
});

test('an acknowledged deletion is still counted if the direct API finds the run', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 11 }, (_, i) => run(i + 1)) }], ({ method, path }) => {
    if (method === 'DELETE' && path.endsWith('/runs/1')) return { status: 204 };
  });
  const result = await retainRuns({ ...f.options, dryRun: false });
  assert.deepEqual(result.deleted, [1]);
  assert.equal(result.remaining, 1);
  assert.equal(f.calls.filter((call) => call.method === 'DELETE').length, 1);
  assert.equal(f.calls.filter((call) => call.method === 'GET' && call.path.endsWith('/runs/1')).length, 2);
});

test('a failed final inventory exposes unverified partial cleanup instead of snapshot success', async () => {
  let inventories = 0;
  const f = fixture([{ id: 1, runs: Array.from({ length: 11 }, (_, i) => run(i + 1)) }], ({ path }) => {
    if (path.includes('/runs?') && ++inventories > 1) throw error(502);
  });
  await assert.rejects(retainRuns({ ...f.options, dryRun: false }), (failure) => {
    assert.deepEqual(failure.retention.deleted, [1]);
    assert.equal(failure.retention.refreshed, false);
    return true;
  });
});

test('fresh status and attempt checks preserve a queued or rerun candidate', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 12 }, (_, i) => run(i + 1)) }], ({ method, path }) => {
    if (method === 'GET' && path.endsWith('/runs/1')) return { data: run(1, 1, 'queued') };
    if (method === 'GET' && path.endsWith('/runs/2')) return { data: { ...run(2), run_attempt: 2 } };
  });
  const result = await retainRuns({ ...f.options, dryRun: false });
  assert.deepEqual(result.deferred, [1, 2]);
  assert.equal(result.remaining, 2);
  assert.ok(!f.calls.some((call) => call.method === 'DELETE'));
});

test('batch selection reserves API quota for other CI jobs', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 15 }, (_, i) => run(i + 1)) }], ({ path, live }) => {
    if (path.includes('?')) return { data: { total_count: live.size, workflow_runs: [...live.values()] },
      headers: { 'x-ratelimit-remaining': '54' } };
  });
  const result = await retainRuns({ ...f.options, maxDeletions: 500 });
  assert.deepEqual(result.selected, [1, 2]);
});

test('already deleted runs are idempotent, identity changes fail closed', async () => {
  const groups = [{ id: 1, runs: Array.from({ length: 11 }, (_, i) => run(i + 1)) }];
  const missing = fixture(groups, ({ method, path }) => {
    if (method === 'GET' && path.endsWith('/runs/1')) throw error(404);
  });
  const result = await retainRuns({ ...missing.options, dryRun: false });
  assert.deepEqual(result.missing, [1]);
  const changed = fixture(groups, ({ method, path }) => {
    if (method === 'GET' && path.endsWith('/runs/1')) return { data: run(1, 2) };
  });
  await assert.rejects(retainRuns({ ...changed.options, dryRun: false }), /identity changed/);
  assert.ok(!changed.calls.some((call) => call.method === 'DELETE'));
});

test('rate limits respect the longer Retry-After or primary reset deadline', async () => {
  const f = fixture([{ id: 1, runs: [] }], ({ calls, time }) => {
    if (calls.length === 1) throw error(403, {
      'retry-after': '2', 'x-ratelimit-remaining': '0', 'x-ratelimit-reset': String(time / 1000 + 10),
    });
  });
  await retainRuns(f.options);
  assert.equal(f.waits[0], 11_000);
  assert.equal(rateDelay(error(429, { 'retry-after': '2' }), 0, 0), 2000);
  assert.equal(rateDelay(error(429), 1, 0), 120_000);
});

test('forbidden permissions and excessive cooldowns do not trigger repeated requests', async () => {
  for (const failure of [error(403), error(429, { 'retry-after': '600' })]) {
    const f = fixture([], () => { throw failure; });
    await assert.rejects(retainRuns(f.options));
    assert.equal(f.calls.length, 1);
  }
});

test('rate-limit retries are bounded and partial deletion is exposed on failure', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 12 }, (_, i) => run(i + 1)) }], ({ method, path }) => {
    if (method === 'DELETE' && path.endsWith('/runs/2')) throw error(429, { 'retry-after': '1' });
  });
  await assert.rejects(retainRuns({ ...f.options, dryRun: false }), (failure) => {
    assert.deepEqual(failure.retention.deleted, [1]);
    assert.equal(failure.retention.remaining, 1);
    return true;
  });
  assert.equal(f.calls.filter((call) => call.method === 'DELETE' && call.path.endsWith('/runs/2')).length, 3);
});

test('a run requeued during a deletion cooldown is not deleted by the retry', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 11 }, (_, i) => run(i + 1)) }], ({ method, live }) => {
    if (method === 'DELETE') {
      live.get(1).status = 'queued';
      throw error(429, { 'retry-after': '1' });
    }
  });
  const result = await retainRuns({ ...f.options, dryRun: false });
  assert.deepEqual(result.deleted, []);
  assert.deepEqual(result.deferred, [1]);
  assert.equal(f.calls.filter((call) => call.method === 'DELETE').length, 1);
});

test('incomplete or off-origin pagination prevents every deletion', async () => {
  for (const response of [
    { data: { total_count: 2, workflow_runs: [run(1)] }, headers: {} },
    { data: { total_count: 0, workflow_runs: [] }, headers: { link: '<https://attacker.test/x>; rel="next"' } },
  ]) {
    const f = fixture([], () => response);
    await assert.rejects(retainRuns({ ...f.options, dryRun: false }));
    assert.ok(!f.calls.some((call) => call.method === 'DELETE'));
  }
});

test('GitHub canonical repository-ID pagination is accepted only for the verified repository', async () => {
  for (const nextId of [42, 99]) {
    const f = fixture([{ id: 1, runs: [] }], ({ path }) => {
      const second = path.startsWith('/repositories/');
      return { data: { total_count: 2, workflow_runs: [{ ...run(second ? 2 : 1),
        repository: { id: 42, full_name: 'owner/repo' } }] },
      headers: second ? {} : {
        link: `<https://api.github.com/repositories/${nextId}/actions/runs?page=2>; rel="next"`,
      } };
    });
    if (nextId === 42) assert.equal((await retainRuns(f.options)).workflows[0].total, 2);
    else await assert.rejects(retainRuns(f.options), /pagination destination/);
  }
});

test('malformed identities and limits are rejected before live requests', async () => {
  assert.throws(() => planRetention([{ id: 1, runs: [run(1, 2)] }]), /Invalid run/);
  for (const options of [{ repo: { owner: '..', repo: 'repo' } }, { maxDeletions: 501 }, { currentRunId: 0 }]) {
    const f = fixture([]);
    await assert.rejects(retainRuns({ ...f.options, ...options }));
    assert.equal(f.calls.length, 0);
  }
});

async function workflowRetention(f, maxDeletions = 100, dryRun = false) {
  const workflow = readFileSync(require('node:path').join(__dirname, '../workflows/actions_retention.yml'), 'utf8');
  const script = workflow.match(/script: \|\n([\s\S]*)/)[1].replace(/^ {12}/gm, '');
  const failures = [];
  const summary = { addHeading() { return this; }, addRaw() { return this; }, async write() {} };
  const core = { info() {}, setFailed: (message) => failures.push(message), summary };
  const load = (path) => {
    assert.equal(path, './.github/scripts/retain-runs.cjs');
    return { retainRuns: (options) => retainRuns({ ...options, ...f.options }) };
  };
  const AsyncFunction = (async () => {}).constructor;
  await new AsyncFunction('require', 'core', 'context', 'process', 'github', script)(load, core,
    { repo, runId: 9999 }, { env: { RETENTION_DRY_RUN: String(dryRun), RETENTION_LIMIT: String(maxDeletions) } }, f.github);
  return failures;
}

test('automatic workflow accepts verified latest-ten inventories for each workflow', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 12 }, (_, i) => run(i + 1)) },
    { id: 2, runs: Array.from({ length: 11 }, (_, i) => run(100 + i, 2)) }]);
  assert.deepEqual(await workflowRetention(f), []);
  assert.equal(f.live.size, 20);
});

test('workflow previews never delete runs or fail for the displayed backlog', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 12 }, (_, i) => run(i + 1)) }]);
  assert.deepEqual(await workflowRetention(f, 100, true), []);
  assert.equal(f.live.size, 12);
});

test('automatic workflow fails visibly when its fresh inventory still has completed excess runs', async () => {
  const f = fixture([{ id: 1, runs: Array.from({ length: 12 }, (_, i) => run(i + 1)) }]);
  const failures = await workflowRetention(f, 1);
  assert.equal(failures.length, 1);
  assert.equal(f.live.size, 11);
});

test('automatic workflow explains protected active excess without cancelling or deleting it', async () => {
  const f = fixture([{ id: 1, runs: [run(1, 1, 'queued'), ...Array.from({ length: 10 }, (_, i) => run(i + 2))] }]);
  const failures = await workflowRetention(f);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /active|current/i);
  assert.equal(f.live.get(1).status, 'queued');
  assert.equal(f.live.size, 11);
});

test('privileged workflow uses only default-branch code and cannot recursively trigger itself', () => {
  const workflow = readFileSync(require('node:path').join(__dirname, '../workflows/actions_retention.yml'), 'utf8');
  const listeners = workflow.match(/workflows: \[([^\]]+)\]/)[1].split(',').map((value) => value.trim());
  assert.deepEqual(listeners, ['Guard Generated Tree', 'Rust GUI', 'Dependency Audit', 'Update Rulesets', 'Dependency Graph']);
  assert.ok(!listeners.includes('Actions Retention'));
  assert.ok(workflow.includes('ref: ${{ github.event.repository.default_branch }}'));
  assert.ok(workflow.includes('persist-credentials: false'));
  assert.ok(workflow.includes('actions: write'));
  assert.ok(!workflow.includes('head_sha') && !workflow.includes('write-all'));
});

function mockCliTransport(t) {
  const token = 'not-a-real-secret-token-for-test-only';
  const http = require('node:http');
  const originalProxy = http.setGlobalProxyFromEnv;
  http.setGlobalProxyFromEnv = () => {};
  t.after(() => {
    if (originalProxy) http.setGlobalProxyFromEnv = originalProxy;
    else delete http.setGlobalProxyFromEnv;
  });
  let authentications = 0;
  t.mock.method(require('node:child_process'), 'execFile', (file, args, options, done) => {
    authentications++;
    assert.equal(file, 'gh');
    assert.deepEqual(args, ['auth', 'token', '--hostname', 'github.com']);
    assert.equal(options.timeout, 60_000);
    queueMicrotask(() => done(null, `${token}\n`));
  });
  delete require.cache[require.resolve('./retain-runs.cjs')];
  return { request: require('./retain-runs.cjs').ghRequest, token, authentications: () => authentications };
}

test('CLI fetch transport reuses in-memory gh credentials and preserves safe request options', async (t) => {
  const f = mockCliTransport(t);
  const requests = [];
  t.mock.method(globalThis, 'fetch', async (url, options) => {
    requests.push([url, options]);
    assert.equal(url.origin, 'https://api.github.com');
    assert.equal(options.headers.authorization, `Bearer ${f.token}`);
    assert.equal(options.redirect, 'error');
    assert.ok(options.signal instanceof AbortSignal);
    return options.method === 'DELETE' ? new Response(null, { status: 204 }) :
      Response.json({ id: 1 }, { headers: { 'x-ratelimit-remaining': '42' } });
  });
  const result = await f.request('GET /repos/owner/repo/actions/runs/1');
  assert.equal(result.headers['x-ratelimit-remaining'], '42');
  assert.equal((await f.request('DELETE /repos/owner/repo/actions/runs/1')).status, 204);
  assert.equal(requests.length, 2);
  assert.equal(f.authentications(), 1);
});

test('CLI transport rejects other origins and redacts HTTP, timeout and malformed-body failures', async (t) => {
  const f = mockCliTransport(t);
  let calls = 0;
  t.mock.method(globalThis, 'fetch', async () => {
    calls++;
    if (calls === 1) return Response.json({ message: `rate limit ${f.token}` },
      { status: 429, headers: { 'retry-after': '2' } });
    if (calls === 2) throw new Error(`timeout ${f.token}`);
    return new Response(f.token);
  });
  await assert.rejects(f.request('GET https://attacker.test/actions/runs'), /official GitHub/);
  assert.equal(f.authentications(), 0);
  assert.equal(calls, 0);
  for (let attempt = 0; attempt < 3; attempt++) {
    await assert.rejects(f.request('GET /repos/owner/repo/actions/runs/1'), (failure) => {
      assert.ok(!failure.message.includes(f.token));
      if (attempt === 0) {
        assert.equal(failure.status, 429);
        assert.equal(failure.response.headers['retry-after'], '2');
      }
      return true;
    });
  }
  assert.equal(f.authentications(), 1);
  assert.equal(calls, 3);
});
