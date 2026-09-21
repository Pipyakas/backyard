// Backyard - Local LLM Eval Lab

// ---------------------------------------------------------------------------
// Theme management
// ---------------------------------------------------------------------------
function setAccent(accent) {
    document.documentElement.setAttribute('data-accent', accent);
    localStorage.setItem('accent', accent);
}

function setTheme(theme) {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
    const select = document.getElementById('theme-select');
    if (select) select.value = theme;
}

function initTheme() {
    setTheme(localStorage.getItem('theme') || 'dark');
    setAccent(localStorage.getItem('accent') || 'gray');
}

initTheme();

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function $(id) { return document.getElementById(id); }

function esc(s) {
    return String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

function fmtNum(v, digits = 1) {
    if (v === null || v === undefined || isNaN(v)) return '-';
    return Number(v).toFixed(digits);
}

function fmtTime(iso) {
    if (!iso) return '-';
    return iso.replace('T', ' ').slice(0, 16);
}

async function api(path, opts = {}) {
    const res = await fetch(path, {
        headers: { 'Content-Type': 'application/json' },
        ...opts,
    });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) {
        const err = new Error(data.error || `HTTP ${res.status}`);
        err.status = res.status;
        throw err;
    }
    return data;
}

async function apiPost(path, body) {
    return api(path, { method: 'POST', body: JSON.stringify(body || {}) });
}

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------
function showNotification(message, type = 'info', duration = 5000) {
    const container = $('notification-container');
    if (!container) return;
    const icons = { success: '✓', error: '✕', info: 'ℹ' };
    const notification = document.createElement('div');
    notification.className = `notification ${type}`;
    notification.innerHTML = `
        <span class="icon">${icons[type] || icons.info}</span>
        <span class="message">${esc(message)}</span>
        <button class="close-btn" onclick="this.parentElement.remove()">×</button>
    `;
    container.appendChild(notification);
    if (duration > 0) {
        setTimeout(() => {
            notification.style.animation = 'slideIn 0.3s ease reverse';
            setTimeout(() => notification.remove(), 300);
        }, duration);
    }
}

// ---------------------------------------------------------------------------
// Tabs / navigation
// ---------------------------------------------------------------------------
const TAB_PATHS = {
    'leaderboard': '/',
    'benchmarks': '/benchmarks',
    'evals': '/evals',
    'endpoints': '/endpoints',
    'settings': '/settings',
};

const PATH_TABS = Object.fromEntries(Object.entries(TAB_PATHS).map(([k, v]) => [v, k]));

function loadTabContent(path) {
    const tabId = PATH_TABS[path] || 'leaderboard';
    document.querySelectorAll('.tab').forEach(t => {
        t.classList.toggle('active', TAB_PATHS[t.dataset.tab] === path);
    });
    document.querySelectorAll('.tab-content').forEach(section => {
        section.classList.toggle('active', section.id === tabId);
    });
    if (tabId === 'leaderboard') loadLeaderboard();
    if (tabId === 'benchmarks') { loadBenchHistory(); refreshSpeedChartModels(); loadEndpoints(); }
    if (tabId === 'evals') { loadEvalHistory(); renderEvalChart(); loadEvalEndpoints(); }
    if (tabId === 'endpoints') loadEndpoints();
    if (tabId === 'settings') loadSystemInfo();
}

function navigateTo(path) {
    history.pushState(null, '', path);
    loadTabContent(path);
}

function initTabs() {
    document.querySelectorAll('.tab').forEach(tab => {
        tab.addEventListener('click', (e) => {
            e.preventDefault();
            navigateTo(TAB_PATHS[tab.dataset.tab]);
        });
    });
    window.addEventListener('popstate', () => loadTabContent(window.location.pathname));
}

// ---------------------------------------------------------------------------
// Modals
// ---------------------------------------------------------------------------
function closeModal(modalId) {
    const modal = $(modalId);
    if (modal) modal.style.display = 'none';
}

function showConfirm(title, message, onConfirm) {
    $('dialog-title').textContent = title;
    $('dialog-message').textContent = message;
    $('dialog-modal').style.display = 'flex';
    const confirmBtn = $('dialog-confirm');
    const cancelBtn = $('dialog-cancel');
    const handler = () => {
        closeModal('dialog-modal');
        confirmBtn.removeEventListener('click', handler);
        cancelBtn.removeEventListener('click', cancelHandler);
        onConfirm();
    };
    const cancelHandler = () => {
        closeModal('dialog-modal');
        confirmBtn.removeEventListener('click', handler);
        cancelBtn.removeEventListener('click', cancelHandler);
    };
    confirmBtn.addEventListener('click', handler);
    cancelBtn.addEventListener('click', cancelHandler);
}

// ---------------------------------------------------------------------------
// Admin / auth state
// ---------------------------------------------------------------------------
let adminUser = null;

function refreshAdminUi() {
    const isAdmin = true; // single-user webapp: forms always visible
    $('admin-status-btn').style.display = 'inline-block';
    $('admin-status-btn').textContent = isAdmin ? `Admin: ${adminUser.username}` : 'Admin';
    ['bench-run-form-card', 'eval-run-form-card', 'eval-endpoint-card', 'machine-add-card', 'model-admin-card', 'endpoint-add-card'].forEach(id => {
        const el = $(id);
        if (el) el.style.display = isAdmin ? '' : 'none';
    });
    ['bench-login-hint', 'eval-login-hint', 'endpoint-login-hint'].forEach(id => {
        const el = $(id);
        if (el) el.style.display = isAdmin ? 'none' : '';
    });
    renderAuthSection();
}

async function refreshAdminState() {
    try {
        const me = await api('/api/auth/me');
        adminUser = me.user || null;
    } catch (_) {
        adminUser = null;
    }
    refreshAdminUi();
}

async function renderAuthSection() {
    const container = $('auth-section');
    if (!container) return;
    let status;
    try {
        status = await api('/api/auth/status');
    } catch (_) {
        container.innerHTML = '<p class="empty-state">Auth API unavailable.</p>';
        return;
    }

    if (adminUser) {
        container.innerHTML = `
            <div class="status-grid">
                <div class="status-row"><span>Logged in as</span><span>${esc(adminUser.username)}</span></div>
            </div>
            <div class="modal-actions" style="margin-top: 12px;">
                <button class="btn secondary" onclick="logoutAdmin()">Logout</button>
            </div>`;
        return;
    }

    if (!status.has_users) {
        container.innerHTML = `
            <p class="text-muted">Create the admin account to run tests and manage machines.</p>
            <form id="setup-form" style="margin-top: 12px;">
                <div class="form-row">
                    <div class="form-group">
                        <label>Username</label>
                        <input type="text" id="setup-username" required minlength="3">
                    </div>
                    <div class="form-group">
                        <label>Password</label>
                        <input type="password" id="setup-password" required minlength="8">
                    </div>
                    <div class="form-group" style="align-self: flex-end;">
                        <button class="btn primary" type="submit">Create Admin</button>
                    </div>
                </div>
            </form>`;
        $('setup-form').addEventListener('submit', async (e) => {
            e.preventDefault();
            try {
                await apiPost('/api/auth/local/setup', {
                    username: $('setup-username').value.trim(),
                    password: $('setup-password').value,
                });
                await apiPost('/api/auth/login', {
                    username: $('setup-username').value.trim(),
                    password: $('setup-password').value,
                });
                showNotification('Admin account created', 'success');
                refreshAdminState();
            } catch (err) {
                showNotification(err.message, 'error');
            }
        });
    } else {
        container.innerHTML = `
            <p class="text-muted">You are not logged in. Login to run tests and manage machines.</p>
            <button class="btn primary" style="margin-top: 12px;" onclick="window.location.href='/auth'">Login</button>`;
    }
}

async function logoutAdmin() {
    try { await apiPost('/api/auth/logout', {}); } catch (_) {}
    adminUser = null;
    refreshAdminUi();
    showNotification('Logged out', 'info');
}

// ---------------------------------------------------------------------------
// Leaderboard (local Artificial Analysis clone)
// ---------------------------------------------------------------------------
// Unified model: one row per (model_label × endpoint) built from the raw
// /api/leaderboard metric buckets. Speed rows come from tps/tg_tps joined
// with latency/ttft/success; quality rows come from every other metric.
let lbData = { speed: [], quality: [] };
let lbSort = { key: 'tps', dir: -1 };

const QUALITY_METRICS = ['mswe_pass_rate', 'tau2_avg_reward', 'lcb_pass_at_1', 'swebench_resolved', 'toolathlon_score', 'frontierswe_score', 'deepswe_reward', 'deepswe_partial', 'tbench_reward', 'perf_speedup', 'score', 'pass_at_1', 'rouge_l_f1', 'success_rate'];

const HARNESS_LABELS = {
    mswe_pass_rate: 'micro_swe', tau2_avg_reward: 'tau2', lcb_pass_at_1: 'livecodebench',
    swebench_resolved: 'swebench', toolathlon_score: 'toolathlon', frontierswe_score: 'frontier-swe',
    deepswe_reward: 'deepswe', deepswe_partial: 'deepswe', tbench_reward: 'tbench',
    perf_speedup: 'perf_takehome', score: 'score', pass_at_1: 'pass@1', rouge_l_f1: 'rouge-l',
    success_rate: 'bench',
};

function lbKey(e) { return `${e.model}|||${e.machine}`; }

async function loadLeaderboard() {
    try {
        const data = await api('/api/leaderboard');
        const m = data.metrics || {};
        buildLeaderboardModel(m);
        const upd = $('lb-updated-at');
        if (upd) upd.textContent = `· updated ${new Date().toLocaleTimeString()}`;
        populateLbFilters();
        renderLeaderboardTables();
        loadEndpointsStrip();
    } catch (err) {
        showNotification('Leaderboard: ' + err.message, 'error');
    }
}

function buildLeaderboardModel(m) {
    const speedByKey = new Map();
    for (const e of (m.tps || []).concat(m.tg_tps || [])) {
        const k = lbKey(e);
        if (!speedByKey.has(k) || (e.run_id || 0) > (speedByKey.get(k).run_id || 0)) {
            speedByKey.set(k, { model: e.model, endpoint: e.machine, tps: e.value, run_id: e.run_id, run_created_at: e.run_created_at });
        }
    }
    const joinMetric = (entries, field) => {
        const best = new Map();
        for (const e of entries) {
            const k = lbKey(e);
            if (!best.has(k) || (e.run_id || 0) > (best.get(k).run_id || 0)) best.set(k, e.value);
        }
        for (const [k, v] of best) {
            if (speedByKey.has(k)) speedByKey.get(k)[field] = v;
        }
    };
    joinMetric(m.latency_ms || [], 'latency');
    joinMetric(m.ttft_ms || [], 'ttft');
    joinMetric(m.success_rate || [], 'success');

    const quality = [];
    for (const metric of QUALITY_METRICS) {
        if (metric === 'success_rate') continue;
        for (const e of (m[metric] || [])) {
            quality.push({
                model: e.model, endpoint: e.machine, harness: HARNESS_LABELS[metric] || metric,
                metric, value: e.value, run_id: e.run_id, run_created_at: e.run_created_at,
            });
        }
    }
    // De-dupe quality to newest run per (model × endpoint × harness)
    const qBest = new Map();
    for (const q of quality) {
        const k = `${q.model}|||${q.endpoint}|||${q.harness}`;
        if (!qBest.has(k) || (q.run_id || 0) > (qBest.get(k).run_id || 0)) qBest.set(k, q);
    }
    lbData = { speed: [...speedByKey.values()], quality: [...qBest.values()] };
}

function populateLbFilters() {
    const eps = [...new Set([...lbData.speed.map(r => r.endpoint), ...lbData.quality.map(r => r.endpoint)])].sort();
    const sel = $('lb-endpoint');
    if (sel) {
        const cur = sel.value;
        sel.innerHTML = '<option value="">All endpoints</option>' +
            eps.map(e => `<option value="${esc(e)}">${esc(e)}</option>`).join('');
        if (eps.includes(cur)) sel.value = cur;
    }
    const hs = [...new Set(lbData.quality.map(r => r.harness))].sort();
    const hsel = $('lb-harness');
    if (hsel) {
        const cur = hsel.value;
        hsel.innerHTML = '<option value="">All harnesses</option>' +
            hs.map(h => `<option value="${esc(h)}">${esc(h)}</option>`).join('');
        if (hs.includes(cur)) hsel.value = cur;
    }
}

function lbFilteredSpeed() {
    const q = ($('lb-search')?.value || '').toLowerCase();
    const ep = $('lb-endpoint')?.value || '';
    let rows = lbData.speed.filter(r =>
        (!q || r.model.toLowerCase().includes(q)) && (!ep || r.endpoint === ep));
    const { key, dir } = lbSort;
    const val = (r) => key === 'model' ? r.model.toLowerCase()
        : key === 'endpoint' ? (r.endpoint || '').toLowerCase()
        : (r[key] ?? -Infinity);
    rows.sort((a, b) => {
        const va = val(a), vb = val(b);
        if (typeof va === 'string') return dir * va.localeCompare(vb);
        return dir * (va - vb);
    });
    return rows;
}

function renderLeaderboardTables() {
    renderSpeedLb();
    renderQualityLb();
    renderHighlights();
    document.querySelectorAll('#leaderboard-speed th[data-sort]').forEach(th => {
        th.classList.toggle('sorted-desc', th.dataset.sort === lbSort.key && lbSort.dir === -1);
        th.classList.toggle('sorted-asc', th.dataset.sort === lbSort.key && lbSort.dir === 1);
        th.onclick = () => {
            const k = th.dataset.sort;
            if (lbSort.key === k) lbSort.dir *= -1;
            else lbSort = { key: k, dir: k === 'model' || k === 'endpoint' ? 1 : -1 };
            renderSpeedLb();
        };
    });
}

function barHtml(value, max, fmt) {
    if (value === undefined || max <= 0) return '<span class="text-muted">-</span>';
    const pct = Math.max(2, Math.min(100, (value / max) * 100));
    return `<span class="lb-bar"><span class="lb-bar-fill" style="width:${pct}%"></span><span class="lb-bar-val">${fmt}</span></span>`;
}

function renderSpeedLb() {
    const tbody = $('leaderboard-speed')?.querySelector('tbody');
    if (!tbody) return;
    const rows = lbFilteredSpeed();
    if (!rows.length) {
        tbody.innerHTML = '<tr><td colspan="8" class="empty-state">No speed runs yet — queue one from the Speed tab.</td></tr>';
        return;
    }
    const maxTps = Math.max(...rows.map(r => r.tps ?? 0), 0);
    tbody.innerHTML = rows.map((r, i) => `<tr>
        <td class="text-muted">${i + 1}</td>
        <td><strong>${esc(r.model)}</strong></td>
        <td>${esc(r.endpoint)}</td>
        <td class="num"><strong>${r.tps !== undefined ? fmtNum(r.tps) : '-'}</strong>${r.tps !== undefined ? barHtml(r.tps, maxTps, '') : ''}</td>
        <td class="num">${r.latency !== undefined ? fmtNum(r.latency, 0) + ' ms' : '-'}</td>
        <td class="num">${r.ttft !== undefined ? fmtNum(r.ttft, 0) + ' ms' : '-'}</td>
        <td class="num">${r.success !== undefined ? fmtNum(r.success * 100, 0) + '%' : '-'}</td>
        <td><button class="btn small" onclick="showRunDetail(${r.run_id})">#${r.run_id}</button></td>
    </tr>`).join('');
}

function renderQualityLb() {
    const tbody = $('leaderboard-quality')?.querySelector('tbody');
    if (!tbody) return;
    const q = ($('lb-search')?.value || '').toLowerCase();
    const ep = $('lb-endpoint')?.value || '';
    const h = $('lb-harness')?.value || '';
    const rows = lbData.quality
        .filter(r => (!q || r.model.toLowerCase().includes(q)) && (!ep || r.endpoint === ep) && (!h || r.harness === h))
        .sort((a, b) => b.value - a.value);
    if (!rows.length) {
        tbody.innerHTML = '<tr><td colspan="6" class="empty-state">No quality evals yet — queue one from the Quality tab.</td></tr>';
        return;
    }
    tbody.innerHTML = rows.map((r, i) => `<tr>
        <td class="text-muted">${i + 1}</td>
        <td><strong>${esc(r.model)}</strong></td>
        <td>${esc(r.endpoint)}</td>
        <td><span class="badge">${esc(r.harness)}</span></td>
        <td class="num"><strong>${fmtNum(r.value, 3)}</strong></td>
        <td><button class="btn small" onclick="showRunDetail(${r.run_id})">#${r.run_id}</button></td>
    </tr>`).join('');
}

function renderHighlights() {
    const el = $('lb-highlights');
    if (!el) return;
    const cards = [];
    const byTps = [...lbData.speed].filter(r => r.tps !== undefined).sort((a, b) => b.tps - a.tps);
    if (byTps.length) {
        const b = byTps[0];
        cards.push({ label: 'Fastest', value: `${fmtNum(b.tps)} tok/s`, sub: `${b.model} · ${b.endpoint}` });
    }
    const byLat = [...lbData.speed].filter(r => r.latency !== undefined).sort((a, b) => a.latency - b.latency);
    if (byLat.length) {
        const b = byLat[0];
        cards.push({ label: 'Lowest latency', value: `${fmtNum(b.latency, 0)} ms`, sub: `${b.model} · ${b.endpoint}` });
    }
    const byTtft = [...lbData.speed].filter(r => r.ttft !== undefined).sort((a, b) => a.ttft - b.ttft);
    if (byTtft.length) {
        const b = byTtft[0];
        cards.push({ label: 'Lowest TTFT', value: `${fmtNum(b.ttft, 0)} ms`, sub: `${b.model} · ${b.endpoint}` });
    }
    if (lbData.quality.length) {
        const b = [...lbData.quality].sort((a, b) => b.value - a.value)[0];
        cards.push({ label: `Top quality · ${b.harness}`, value: fmtNum(b.value, 3), sub: `${b.model} · ${b.endpoint}` });
    }
    el.innerHTML = cards.length
        ? cards.map(c => `<div class="lb-stat"><span class="lb-stat-label">${esc(c.label)}</span><span class="lb-stat-value">${esc(c.value)}</span><span class="text-muted">${esc(c.sub)}</span></div>`).join('')
        : '<p class="text-muted">Run a speed test or quality eval to populate the leaderboard.</p>';
}

async function loadEndpointsStrip() {
    const el = $('leaderboard-endpoints');
    if (!el) return;
    try {
        const data = await api('/api/endpoints');
        const eps = data.endpoints || [];
        el.innerHTML = eps.length
            ? eps.map(e => `<span class="lb-ep"><strong>${esc(e.name)}</strong><span class="mono">${esc(e.base_url)}</span>${e.model ? `<span class="badge quant">${esc(e.model)}</span>` : ''}</span>`).join('')
            : '<p class="text-muted">No endpoints yet — add one on the Endpoints tab.</p>';
    } catch (_) {}
}

function gpuBadge(gpuType) {
    const cls = gpuType === 'nvidia' ? 'cuda' : gpuType === 'amd' ? 'vulkan' : 'quant';
    return `<span class="badge ${cls}">${esc(gpuType)}</span>`;
}

// ---------------------------------------------------------------------------
// Speed test form + history
// ---------------------------------------------------------------------------

async function submitBenchRun(e) {
    e.preventDefault();
    const form = new FormData(e.target);
    try {
        await apiPost('/api/benchmarks', {
            endpoint_id: parseInt(form.get('endpoint_id'), 10),
            prompt: form.get('prompt') || undefined,
            iterations: parseInt(form.get('iterations') || '5', 10),
            max_tokens: parseInt(form.get('max_tokens') || '128', 10),
            temperature: parseFloat(form.get('temperature') || '0.7'),
            stream: form.get('stream') === 'on',
        });
        showNotification('Speed test queued', 'success');
        setTimeout(loadBenchHistory, 1000);
    } catch (err) {
        showNotification(err.message, 'error');
    }
}

let endpointsCache = [];

async function loadEndpoints() {
    try {
        const data = await api('/api/endpoints');
        endpointsCache = data.endpoints || [];
        const html = endpointsCache.length
            ? endpointsCache.map(ep => `
                <div class="engine-item" style="display:flex;justify-content:space-between;align-items:center;gap:12px">
                    <span><strong>${esc(ep.name)}</strong> <span class="mono" style="font-size:0.85em">${esc(ep.base_url)}</span>
                    ${ep.model ? ` <span class="badge quant">${esc(ep.model)}</span>` : ''}
                    ${ep.provider ? ` <span class="badge">${esc(ep.provider)}</span>` : ''}
                    ${ep.has_api_key ? '<span class="badge pending">key</span>' : ''}</span>
                    <span style="display:flex;gap:6px">
                        <button class="btn small secondary" onclick="probeEndpoint(${ep.id})">Probe</button>
                        <button class="btn small danger" onclick="deleteEndpoint(${ep.id}, '${esc(ep.name)}')">Delete</button>
                    </span>
                </div>`).join('')
            : '<p class="text-muted">No endpoints yet. Add ccgw / l0 / l1 / d1 / r2 / r3 above.</p>';
        const el1 = $('endpoints-list');
        const el2 = $('endpoints-list-2');
        if (el1) el1.innerHTML = html;
        if (el2) el2.innerHTML = html;
        // populate bench endpoint select
        const sel = $('bench-endpoint');
        if (sel) sel.innerHTML = endpointsCache.map(ep => `<option value="${ep.id}">${esc(ep.name)} (${esc(ep.base_url)})</option>`).join('');
        const sel2 = $('eval-endpoint');
        if (sel2) sel2.innerHTML = endpointsCache.map(ep => `<option value="${ep.id}">${esc(ep.name)} (${esc(ep.base_url)})</option>`).join('');
    } catch (err) { showNotification('Endpoints: ' + err.message, 'error'); }
}

async function addEndpoint() {
    try {
        await apiPost('/api/endpoints', {
            name: ($('ep2-name')?.value || $('ep-name')?.value || '').trim(),
            base_url: ($('ep2-url')?.value || $('ep-url')?.value || '').trim(),
            model: ($('ep2-model')?.value || $('ep-model')?.value || '').trim() || null,
            api_key: ($('ep2-key')?.value || $('ep-key')?.value || '').trim() || null,
            provider: ($('ep2-provider')?.value || 'generic'),
        });
        showNotification('Endpoint added', 'success');
        ['ep2-name','ep2-url','ep2-model','ep2-key','ep-name','ep-url','ep-model','ep-key'].forEach(id => { const el=$(id); if(el) el.value=''; });
        loadEndpoints(); loadEvalEndpoints();
    } catch (err) { showNotification(err.message, 'error'); }
}

async function deleteEndpoint(id, name) {
    showConfirm('Delete Endpoint', `Delete endpoint "${name}"?`, async () => {
        try { await fetch(`/api/endpoints/${id}`, { method: 'DELETE' }); showNotification('Endpoint deleted', 'success'); loadEndpoints(); loadEvalEndpoints(); } catch (err) { showNotification(err.message, 'error'); }
    });
}

async function probeEndpoint(id) {
    try {
        const data = await apiPost(`/api/endpoints/${id}/health`, {});
        showNotification('Probe: ' + (data.output || 'ok').slice(0, 300), 'info', 8000);
    } catch (err) { showNotification('Probe failed: ' + err.message, 'error'); }
}

// ---------------------------------------------------------------------------
// Quality-eval endpoints + runs (legacy: same store, shares endpointsCache)
// ---------------------------------------------------------------------------

async function loadEvalEndpoints() {
    try {
        const data = await api('/api/evals/endpoints');
        // Only fill from legacy path if the modern one hasn't loaded yet.
        if (!endpointsCache.length) {
            endpointsCache = data.endpoints || [];
            const list = $('endpoints-list');
            if (list) {
                list.innerHTML = endpointsCache.length
                    ? endpointsCache.map(ep => `
                        <div class="engine-item" style="display: flex; justify-content: space-between; align-items: center;">
                            <span><strong>${esc(ep.name)}</strong> <span class="mono">${esc(ep.base_url)}</span>
                            ${ep.model ? ` <span class="badge quant">${esc(ep.model)}</span>` : ''}
                            ${ep.has_api_key ? '<span class="badge pending">key</span>' : ''}</span>
                            <button class="btn small danger" onclick="deleteEvalEndpoint(${ep.id}, '${esc(ep.name)}')">Delete</button>
                        </div>`).join('')
                    : '<p class="text-muted">No endpoints registered.</p>';
            }
            const sel = $('eval-endpoint');
            if (sel) {
                sel.innerHTML = endpointsCache.map(ep => `<option value="${ep.id}">${esc(ep.name)} (${esc(ep.base_url)})</option>`).join('');
            }
        } else {
            const sel = $('eval-endpoint');
            if (sel) {
                sel.innerHTML = endpointsCache.map(ep => `<option value="${ep.id}">${esc(ep.name)} (${esc(ep.base_url)})</option>`).join('');
            }
        }
    } catch (err) {
        showNotification('Endpoints: ' + err.message, 'error');
    }
}

async function addEvalEndpoint() {
    try {
        await apiPost('/api/evals/endpoints', {
            name: $('ep-name').value.trim(),
            base_url: $('ep-url').value.trim(),
            model: $('ep-model').value.trim() || null,
            api_key: $('ep-key').value.trim() || null,
        });
        showNotification('Endpoint registered', 'success');
        $('ep-name').value = ''; $('ep-url').value = ''; $('ep-model').value = ''; $('ep-key').value = '';
        loadEvalEndpoints();
    } catch (err) {
        showNotification(err.message, 'error');
    }
}

function deleteEvalEndpoint(id, name) {
    showConfirm('Delete Endpoint', `Delete endpoint "${name}"?`, async () => {
        try {
            await fetch(`/api/evals/endpoints/${id}`, { method: 'DELETE' });
            showNotification('Endpoint deleted', 'success');
            loadEvalEndpoints();
        } catch (err) {
            showNotification(err.message, 'error');
        }
    });
}

async function submitEvalRun(e) {
    e.preventDefault();
    const form = new FormData(e.target);
    try {
        await apiPost('/api/evals/runs', {
            endpoint_id: parseInt(form.get('endpoint_id'), 10),
            harness: form.get('harness'),
            config: { num_tasks: parseInt(form.get('num_tasks') || '3', 10) },
        });
        showNotification('Quality eval queued', 'success');
        setTimeout(loadEvalHistory, 1000);
    } catch (err) {
        showNotification(err.message, 'error');
    }
}

function statusBadge(status) {
    const cls = status === 'done' ? 'running' : status === 'failed' ? 'failed' : 'pending';
    return `<span class="badge ${cls}">${esc(status)}</span>`;
}

async function loadBenchHistory() {
    const tbody = $('bench-history-body');
    if (!tbody) return;
    try {
        const runs = (await api('/api/runs?limit=100')).runs.filter(r => r.kind === 'bench');
        if (!runs.length) {
            tbody.innerHTML = '<tr><td colspan="8" class="empty-state">No speed tests yet.</td></tr>';
            return;
        }
        tbody.innerHTML = runs.map(r => {
            const m = r.metrics || {};
            const speed = m.tps ?? m.tg_tps;
            const lat = m.latency_ms ?? m.ttft_ms;
            const target = endpointName(r.endpoint_id);
            return `<tr>
                <td class="mono">#${r.id}</td>
                <td><strong>${esc(r.model_label)}</strong></td>
                <td>${esc(target)}</td>
                <td>${statusBadge(r.status)}</td>
                <td><strong>${speed !== undefined ? fmtNum(speed) + ' tok/s' : '-'}</strong></td>
                <td>${lat !== undefined ? fmtNum(lat, 0) + ' ms' : '-'}</td>
                <td>${fmtTime(r.created_at)}</td>
                <td><button class="btn small" onclick="showRunDetail(${r.id})">Details</button></td>
            </tr>`;
        }).join('');
    } catch (err) {
        tbody.innerHTML = `<tr><td colspan="8" class="empty-state">${esc(err.message)}</td></tr>`;
    }
}

async function loadEvalHistory() {
    const tbody = $('eval-history-body');
    if (!tbody) return;
    try {
        const runs = (await api('/api/runs?limit=100')).runs.filter(r => r.kind === 'eval');
        if (!runs.length) {
            tbody.innerHTML = '<tr><td colspan="8" class="empty-state">No quality evals yet.</td></tr>';
            return;
        }
        tbody.innerHTML = runs.map(r => {
            const m = r.metrics || {};
            const score = m.mswe_pass_rate ?? m.tau2_avg_reward ?? m.lcb_pass_at_1 ?? m.swebench_resolved ?? m.toolathlon_score ?? m.frontierswe_score ?? m.deepswe_reward ?? m.tbench_reward ?? m.perf_speedup ?? m.score;
            const target = endpointName(r.endpoint_id);
            return `<tr>
                <td class="mono">#${r.id}</td>
                <td><strong>${esc(r.model_label)}</strong></td>
                <td>${esc(target)}</td>
                <td><span class="badge">${esc(r.task)}</span></td>
                <td>${statusBadge(r.status)}</td>
                <td><strong>${score !== undefined ? fmtNum(score, 3) : '-'}</strong></td>
                <td>${fmtTime(r.created_at)}</td>
                <td><button class="btn small" onclick="showRunDetail(${r.id})">Details</button></td>
            </tr>`;
        }).join('');
    } catch (err) {
        tbody.innerHTML = `<tr><td colspan="8" class="empty-state">${esc(err.message)}</td></tr>`;
    }
}

function endpointName(id) {
    if (!id) return '-';
    return endpointsCache.find(x => x.id === id)?.name || `endpoint-${id}`;
}

async function showRunDetail(runId) {
    try {
        const [runRes, resultsRes] = await Promise.all([
            api(`/api/runs/${runId}`),
            api(`/api/runs/${runId}/results`),
        ]);
        const run = runRes.run;
        const results = resultsRes.results;

        $('run-modal-title').textContent = `Run #${runId} — ${run.model_label}`;
        $('run-modal-metrics').innerHTML = results.length
            ? results.map(r => `
                <div class="detail-item">
                    <span class="detail-label">${esc(r.metric)}</span>
                    <span class="detail-value"><strong>${fmtNum(r.value, 3)}</strong> ${esc(r.unit || '')}</span>
                </div>`).join('')
            : '<p class="text-muted">No parsed metrics.</p>';

        const raw = run.stdout || (run.error || '');
        $('run-modal-output').textContent = run.status === 'failed'
            ? `Status: failed\n\n${run.error}\n\n--- output ---\n${raw}`
            : raw || '(no output)';
        $('run-modal').style.display = 'flex';
    } catch (err) {
        showNotification(err.message, 'error');
    }
}

// ---------------------------------------------------------------------------
// Charts
// ---------------------------------------------------------------------------
let speedChart = null;
let evalChart = null;

async function refreshSpeedChartModels() {
    const sel = $('speed-history-model');
    if (!sel) return;
    const data = await api('/api/leaderboard');
    const models = [...new Set((data.metrics.tg_tps || []).map(e => e.model))];
    const current = sel.value;
    sel.innerHTML = '<option value="">All models</option>' +
        models.map(m => `<option value="${esc(m)}">${esc(m)}</option>`).join('');
    if (models.includes(current)) sel.value = current;
    renderSpeedChart();
}

async function renderSpeedChart() {
    const canvas = $('speed-chart');
    if (!canvas) return;
    const data = await api('/api/leaderboard');
    let entries = data.metrics.tg_tps || [];
    const selected = $('speed-history-model')?.value;
    if (selected) entries = entries.filter(e => e.model === selected);
    entries.sort((a, b) => a.run_id - b.run_id);

    if (speedChart) speedChart.destroy();
    if (!entries.length) return;
    speedChart = new Chart(canvas, {
        type: 'line',
        data: {
            labels: entries.map(e => `${e.model} · ${e.machine}`),
            datasets: [{
                label: 'generation tok/s',
                data: entries.map(e => e.value),
                borderColor: 'rgb(99, 179, 237)',
                backgroundColor: 'rgba(99, 179, 237, 0.15)',
                fill: false,
                tension: 0.2,
            }],
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            plugins: { legend: { display: false } },
            scales: { y: { beginAtZero: true, title: { display: true, text: 'tok/s' } } },
        },
    });
}

async function renderEvalChart() {
    const canvas = $('eval-chart');
    if (!canvas) return;
    const task = $('eval-chart-task')?.value || 'coding';
    const data = await api('/api/leaderboard');
    let entries = [];
    if (task === 'coding') entries = data.metrics.pass_at_1 || [];
    else if (task === 'summarization') entries = data.metrics.rouge_l_f1 || [];
    else entries = (data.metrics.score || []).filter(e => {
        try { return JSON.parse(e.task || '{}').task === task; } catch (_) { return false; }
    });

    if (evalChart) evalChart.destroy();
    if (!entries.length) return;
    evalChart = new Chart(canvas, {
        type: 'bar',
        data: {
            labels: entries.map(e => `${e.model} · ${e.machine}`),
            datasets: [{
                label: task,
                data: entries.map(e => e.value),
                backgroundColor: 'rgba(99, 179, 237, 0.6)',
                borderColor: 'rgb(99, 179, 237)',
                borderWidth: 1,
            }],
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            plugins: { legend: { display: false } },
            scales: { y: { beginAtZero: true, max: 1 } },
        },
    });
}

// ---------------------------------------------------------------------------
// Settings / system info
// ---------------------------------------------------------------------------
async function loadSystemInfo() {
    try {
        const runs = (await api('/api/runs?limit=1000')).runs || [];
        const endpoints = (await api('/api/endpoints')).endpoints || [];
        $('sys-bench-count').textContent = runs.filter(r => r.kind === 'bench').length;
        $('sys-eval-count').textContent = runs.filter(r => r.kind === 'eval').length;
        $('sys-machine-count').textContent = endpoints.length;
    } catch (_) {}
    $('sys-host').textContent = window.location.host;
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
async function init() {
    initTheme();
    initTabs();
    loadTabContent(window.location.pathname);
    await refreshAdminState();
    $('benchmark-form')?.addEventListener('submit', submitBenchRun);
    $('eval-form')?.addEventListener('submit', submitEvalRun);
    setInterval(refreshAdminState, 60000);
    setInterval(() => {
        const active = document.querySelector('.tab-content.active');
        if (active && active.id === 'leaderboard') loadLeaderboard();
        if (active && active.id === 'benchmarks') loadBenchHistory();
        if (active && active.id === 'evals') loadEvalHistory();
    }, 15000);
}

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
