/* ═══════════════════════════════════════════════════════════════════════
   GravityLancacheUI — Frontend Application
   ═══════════════════════════════════════════════════════════════════════ */

// ── Utilities ────────────────────────────────────────────────────────

function formatBytes(bytes) {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  const i = Math.floor(Math.log(Math.abs(bytes)) / Math.log(k));
  return (bytes / Math.pow(k, i)).toFixed(i > 2 ? 2 : 1) + ' ' + sizes[i];
}

function formatPercent(value) {
  return value.toFixed(1) + '%';
}

function timeAgo(dateStr) {
  const now = Date.now();
  const date = new Date(dateStr).getTime();
  const diff = Math.floor((now - date) / 1000);
  if (diff < 60) return diff + 's ago';
  if (diff < 3600) return Math.floor(diff / 60) + 'm ago';
  if (diff < 86400) return Math.floor(diff / 3600) + 'h ago';
  return Math.floor(diff / 86400) + 'd ago';
}

function serviceBadge(service) {
  const cls = 'badge badge-service badge-' + (service || 'other');
  return `<span class="${cls}">${service || 'other'}</span>`;
}

function hitRateBadge(rate) {
  const cls = rate >= 90 ? 'badge-hit' : rate >= 50 ? 'badge-miss' : 'badge-miss';
  return `<span class="badge ${cls}">${formatPercent(rate)}</span>`;
}

function hitRateBar(hitBytes, missBytes) {
  const total = hitBytes + missBytes;
  if (total === 0) return '<div class="hit-rate-bar"><div class="hit-rate-hit" style="width:0"></div></div>';
  const hitPct = (hitBytes / total) * 100;
  const missPct = (missBytes / total) * 100;
  return `<div class="hit-rate-bar">
    <div class="hit-rate-hit" style="width:${hitPct}%"></div>
    <div class="hit-rate-miss" style="width:${missPct}%"></div>
  </div>`;
}

// ── API Client ───────────────────────────────────────────────────────

const API = {
  async get(endpoint) {
    const res = await fetch('/api/v1' + endpoint);
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  },

  async put(endpoint, data) {
    const res = await fetch('/api/v1' + endpoint, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  },

  async post(endpoint) {
    const res = await fetch('/api/v1' + endpoint, { method: 'POST' });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  },
};

// ── WebSocket Manager ────────────────────────────────────────────────

class WSManager {
  constructor() {
    this.ws = null;
    this.listeners = [];
    this.reconnectDelay = 1000;
  }

  connect() {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    this.ws = new WebSocket(`${protocol}//${location.host}/api/v1/ws`);

    this.ws.onopen = () => {
      document.getElementById('ws-status').innerHTML =
        '<span class="live-dot"></span> Live';
      this.reconnectDelay = 1000;
    };

    this.ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        this.listeners.forEach(fn => fn(data));
      } catch (e) { /* ignore malformed */ }
    };

    this.ws.onclose = () => {
      document.getElementById('ws-status').innerHTML =
        '<span style="color:var(--color-miss)">⚠ Reconnecting...</span>';
      setTimeout(() => this.connect(), this.reconnectDelay);
      this.reconnectDelay = Math.min(this.reconnectDelay * 2, 10000);
    };

    this.ws.onerror = () => this.ws.close();
  }

  on(fn) {
    this.listeners.push(fn);
    return () => { this.listeners = this.listeners.filter(l => l !== fn); };
  }
}

const ws = new WSManager();

// ── Router ───────────────────────────────────────────────────────────

const pages = {
  dashboard: renderDashboard,
  downloads: renderDownloads,
  cache: renderCache,
  clients: renderClients,
  prefill: renderPrefill,
  settings: renderSettings,
  logs: renderLogs,
};

let currentPage = 'dashboard';

function navigate(page) {
  currentPage = page;
  document.querySelectorAll('.nav-item').forEach(el => {
    el.classList.toggle('active', el.dataset.page === page);
  });

  const titleMap = {
    dashboard: 'Dashboard',
    downloads: 'Downloads',
    cache: 'Cache Analysis',
    clients: 'Clients',
    prefill: 'Prefill Manager',
    settings: 'Settings',
    logs: 'Backend Logs',
  };
  document.getElementById('page-title').textContent = titleMap[page] || page;

  const main = document.getElementById('main-content');
  main.innerHTML = '';
  main.className = 'main-content page-enter';

  if (pages[page]) {
    pages[page](main);
  }
}

// ── Page: Dashboard ──────────────────────────────────────────────────

async function renderDashboard(container) {
  container.innerHTML = `
    <div class="stats-grid" id="stats-grid">
      <div class="stat-card" style="--stat-accent:var(--accent-primary)">
        <div class="stat-label">Total Traffic</div>
        <div class="stat-value" id="stat-total">—</div>
        <div class="stat-sub" id="stat-total-sub">Loading...</div>
      </div>
      <div class="stat-card" style="--stat-accent:var(--color-hit)">
        <div class="stat-label">Bandwidth Saved</div>
        <div class="stat-value" id="stat-saved">—</div>
        <div class="stat-sub">From cache hits</div>
      </div>
      <div class="stat-card" style="--stat-accent:var(--color-miss)">
        <div class="stat-label">Hit Rate</div>
        <div class="stat-value" id="stat-hitrate">—</div>
        <div class="stat-sub" id="stat-hitrate-bar"></div>
      </div>
      <div class="stat-card" style="--stat-accent:var(--color-info)">
        <div class="stat-label">Total Downloads</div>
        <div class="stat-value" id="stat-downloads">—</div>
        <div class="stat-sub" id="stat-clients-sub">— clients</div>
      </div>
    </div>

    <!-- Live Network Traffic Card -->
    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header" style="justify-content:space-between">
        <div class="card-title" style="display:flex;align-items:center;gap:8px">📶 Live Network Traffic (Unraid Host)</div>
        <div id="net-interfaces-list" style="font-size:0.8rem;color:var(--text-muted)">
          Loading network interfaces...
        </div>
      </div>
      <div style="height:120px;position:relative;margin-top:12px;background:#05070c;border:1px solid var(--border-subtle);border-radius:6px;overflow:hidden">
        <canvas id="net-traffic-chart" style="width:100%;height:100%;display:block"></canvas>
      </div>
    </div>

    <div style="display:grid;grid-template-columns:2fr 1fr;gap:var(--space-md)">
      <div class="card">
        <div class="card-header">
          <div class="card-title">Recent Downloads</div>
        </div>
        <div class="table-container" id="recent-downloads-table">
          <div class="empty-state"><div class="icon">📥</div>No downloads yet</div>
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <div class="card-title">Live Activity</div>
          <span class="live-dot"></span>
        </div>
        <div id="live-feed" style="max-height:400px;overflow-y:auto">
          <div class="empty-state"><div class="icon">📡</div>Waiting for activity...</div>
        </div>
      </div>
    </div>
  `;

  loadDashboardData();
  // Draw empty chart initially
  setTimeout(drawNetTrafficChart, 50);
}

async function loadDashboardData() {
  try {
    const [dashRes, dlRes] = await Promise.all([
      API.get('/dashboard'),
      API.get('/downloads?limit=15'),
    ]);

    const s = dashRes.stats;
    document.getElementById('stat-total').textContent = formatBytes(s.total_bytes);
    document.getElementById('stat-total-sub').textContent =
      `${formatBytes(s.hit_bytes)} hit / ${formatBytes(s.miss_bytes)} miss`;
    document.getElementById('stat-saved').textContent = formatBytes(s.bandwidth_saved);
    document.getElementById('stat-hitrate').textContent = formatPercent(s.hit_rate);
    document.getElementById('stat-hitrate-bar').innerHTML = hitRateBar(s.hit_bytes, s.miss_bytes);
    document.getElementById('stat-downloads').textContent = s.total_downloads.toLocaleString();
    document.getElementById('stat-clients-sub').textContent = s.unique_clients + ' clients';

    renderDownloadsTable('recent-downloads-table', dlRes.downloads);
  } catch (e) {
    console.error('Dashboard load failed:', e);
  }
}

// ── Page: Downloads ──────────────────────────────────────────────────

async function renderDownloads(container) {
  container.innerHTML = `
    <div class="card">
      <div class="card-header">
        <div class="card-title">Download History</div>
        <div style="display:flex;gap:8px">
          <input class="input" placeholder="Filter by game or IP..." id="dl-filter" style="width:250px">
        </div>
      </div>
      <div class="table-container" id="downloads-table">
        <div class="empty-state"><div class="icon">⏳</div>Loading...</div>
      </div>
    </div>
  `;

  try {
    const res = await API.get('/downloads?limit=100');
    renderDownloadsTable('downloads-table', res.downloads);

    document.getElementById('dl-filter').addEventListener('input', (e) => {
      const filter = e.target.value.toLowerCase();
      const rows = document.querySelectorAll('#downloads-table tbody tr');
      rows.forEach(row => {
        row.style.display = row.textContent.toLowerCase().includes(filter) ? '' : 'none';
      });
    });
  } catch (e) {
    document.getElementById('downloads-table').innerHTML =
      '<div class="empty-state"><div class="icon">❌</div>Failed to load</div>';
  }
}

function renderDownloadsTable(containerId, downloads) {
  const container = document.getElementById(containerId);
  if (!downloads || downloads.length === 0) {
    container.innerHTML = '<div class="empty-state"><div class="icon">📥</div>No downloads recorded</div>';
    return;
  }

  container.innerHTML = `
    <table>
      <thead>
        <tr>
          <th>Time</th>
          <th>Service</th>
          <th>Game / ID</th>
          <th>Client</th>
          <th>Size</th>
          <th>Hit Rate</th>
        </tr>
      </thead>
      <tbody>
        ${downloads.map(d => `
          <tr>
            <td title="${d.ended_at}">${timeAgo(d.ended_at)}</td>
            <td>${serviceBadge(d.service)}</td>
            <td>${d.game_name || d.download_id || '—'}</td>
            <td style="font-family:var(--font-mono);font-size:0.8rem">${d.client_ip}</td>
            <td>${formatBytes(d.total_bytes)}</td>
            <td>${hitRateBadge(d.hit_rate)}</td>
          </tr>
        `).join('')}
      </tbody>
    </table>
  `;
}

// ── Page: Cache ──────────────────────────────────────────────────────

async function renderCache(container) {
  container.innerHTML = `
    <div class="stats-grid">
      <div class="stat-card" style="--stat-accent:var(--accent-primary)">
        <div class="stat-label">Cache Size</div>
        <div class="stat-value" id="cache-size">Scanning...</div>
        <div class="stat-sub" id="cache-files">—</div>
      </div>
      <div class="stat-card" style="--stat-accent:var(--color-hit)">
        <div class="stat-label">Last Scan</div>
        <div class="stat-value" id="cache-last-scan">—</div>
        <div class="stat-sub">Automatic scanning active</div>
      </div>
    </div>
    <div class="card">
      <div class="card-header"><div class="card-title">Cache Info</div></div>
      <p style="color:var(--text-secondary)">
        Cache directory analysis shows disk usage of your LanCache.
        Configure scan interval in Settings.
      </p>
    </div>
  `;
}

// ── Page: Clients ────────────────────────────────────────────────────

async function renderClients(container) {
  container.innerHTML = `
    <div class="card">
      <div class="card-header"><div class="card-title">Network Clients</div></div>
      <div id="clients-list">
        <div class="empty-state"><div class="icon">🖥️</div>Loading client data...</div>
      </div>
    </div>
  `;

  try {
    const res = await API.get('/downloads?limit=200');
    const clientMap = {};
    (res.downloads || []).forEach(d => {
      if (!clientMap[d.client_ip]) {
        clientMap[d.client_ip] = { ip: d.client_ip, totalBytes: 0, hitBytes: 0, count: 0, services: new Set() };
      }
      clientMap[d.client_ip].totalBytes += d.total_bytes;
      clientMap[d.client_ip].hitBytes += d.hit_bytes;
      clientMap[d.client_ip].count += 1;
      clientMap[d.client_ip].services.add(d.service);
    });

    const clients = Object.values(clientMap).sort((a, b) => b.totalBytes - a.totalBytes);

    if (clients.length === 0) {
      return;
    }

    document.getElementById('clients-list').innerHTML = `
      <table>
        <thead><tr><th>Client IP</th><th>Total Traffic</th><th>Downloads</th><th>Hit Rate</th><th>Services</th></tr></thead>
        <tbody>
          ${clients.map(c => {
            const hitRate = c.totalBytes > 0 ? (c.hitBytes / c.totalBytes) * 100 : 0;
            return `<tr>
              <td style="font-family:var(--font-mono)">${c.ip}</td>
              <td>${formatBytes(c.totalBytes)}</td>
              <td>${c.count}</td>
              <td>${hitRateBadge(hitRate)}</td>
              <td>${[...c.services].map(s => serviceBadge(s)).join(' ')}</td>
            </tr>`;
          }).join('')}
        </tbody>
      </table>
    `;
  } catch (e) {
    document.getElementById('clients-list').innerHTML =
      '<div class="empty-state"><div class="icon">❌</div>Failed to load</div>';
  }
}

// ── Page: Prefill ────────────────────────────────────────────────────

async function renderPrefill(container) {
  container.innerHTML = `
    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header">
        <div class="card-title">Cache Prefill Manager</div>
      </div>
      <p style="color:var(--text-secondary);margin-bottom:16px">
        Pre-warm your LanCache by downloading game data before your clients need it.
        Integrates with SteamPrefill, BattleNetPrefill, and EpicPrefill.
      </p>
    </div>

    <div class="stats-grid" id="prefill-platforms">
      ${['Steam', 'Battle.net', 'Epic Games'].map(name => {
        const id = name.toLowerCase().replace(/[^a-z]/g, '');
        return `
          <div class="card">
            <div class="card-header">
              <div class="card-title">${name}</div>
              <span class="badge badge-miss">Not configured</span>
            </div>
            <p style="color:var(--text-secondary);font-size:0.85rem;margin-bottom:12px">
              Configure in container shell first.
            </p>
            <button class="btn btn-ghost" onclick="runPrefill('${id}')" id="prefill-btn-${id}">
              ⚡ Run Prefill
            </button>
          </div>
        `;
      }).join('')}
    </div>
  `;
}

async function runPrefill(platform) {
  const btn = document.getElementById('prefill-btn-' + platform);
  if (btn) {
    btn.textContent = '⏳ Running...';
    btn.disabled = true;
  }

  try {
    await API.post('/prefill/run/' + platform);
    if (btn) btn.textContent = '✅ Completed';
  } catch (e) {
    if (btn) btn.textContent = '❌ Failed';
  } finally {
    setTimeout(() => {
      if (btn) { btn.textContent = '⚡ Run Prefill'; btn.disabled = false; }
    }, 3000);
  }
}

// ── Page: Settings ───────────────────────────────────────────────────

async function renderSettings(container) {
  let config = {};
  try { config = await API.get('/config'); } catch (e) { /* defaults */ }

  container.innerHTML = `
    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header"><div class="card-title">API Keys</div></div>
      <div style="margin-bottom:16px">
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">Steam Web API Key</label>
        <input class="input" id="setting-steam-key" type="password"
          placeholder="Enter your Steam Web API Key..."
          value="${config.steam_api_key_set ? '••••••••••••••••' : ''}">
        <p style="color:var(--text-muted);font-size:0.75rem;margin-top:4px">
          Get one at <a href="https://steamcommunity.com/dev/apikey" target="_blank" style="color:var(--accent-primary-light)">steamcommunity.com/dev/apikey</a>
        </p>
      </div>
    </div>

    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header"><div class="card-title">Cache Analysis</div></div>
      <div style="margin-bottom:16px">
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">Scan Interval (seconds)</label>
        <input class="input" id="setting-scan-interval" type="number" min="0" step="60"
          value="${config.cache_scan_interval_secs || 300}">
        <p style="color:var(--text-muted);font-size:0.75rem;margin-top:4px">
          Set to 0 to disable. Default: 300 (5 minutes).
        </p>
      </div>
    </div>

    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header"><div class="card-title">Database</div></div>
      <div style="margin-bottom:16px">
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">Database Path</label>
        <input class="input" id="setting-db-path" value="${config.db_path || '/data/gravitylancacheui/db.sqlite'}">
        <p style="color:var(--text-muted);font-size:0.75rem;margin-top:4px">
          SQLite (default) or PostgreSQL connection string (postgresql://user:pass@host/db)
        </p>
      </div>
      <div style="margin-bottom:16px">
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">Log Retention (days)</label>
        <input class="input" id="setting-retention" type="number" min="1"
          value="${config.log_retention_days || 90}">
      </div>
    </div>

    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header"><div class="card-title">Excluded IPs</div></div>
      <div>
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">
          Comma-separated list of IPs to exclude from tracking
        </label>
        <input class="input" id="setting-excluded-ips"
          value="${(config.excluded_ips || []).join(', ')}">
      </div>
    </div>

    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header"><div class="card-title">Paths</div></div>
      <div style="margin-bottom:16px">
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">LanCache Logs Directory</label>
        <input class="input" value="${config.lancache_logs_dir || ''}" disabled style="opacity:0.6">
        <p style="color:var(--text-muted);font-size:0.75rem;margin-top:4px">Set via LANCACHE_LOGS_DIR env var</p>
      </div>
      <div>
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">LanCache Cache Directory</label>
        <input class="input" value="${config.lancache_cache_dir || ''}" disabled style="opacity:0.6">
        <p style="color:var(--text-muted);font-size:0.75rem;margin-top:4px">Set via LANCACHE_CACHE_DIR env var</p>
      </div>
    </div>

    <div style="display:flex;gap:12px">
      <button class="btn btn-primary" id="settings-save">💾 Save Settings</button>
      <button class="btn btn-ghost" id="settings-check">🔧 Run Setup Check</button>
    </div>
  `;

  document.getElementById('settings-save').addEventListener('click', saveSettings);
  document.getElementById('settings-check').addEventListener('click', runSetupCheck);
}

async function saveSettings() {
  const btn = document.getElementById('settings-save');
  btn.textContent = '⏳ Saving...';
  try {
    await API.put('/config', {
      cache_scan_interval_secs: parseInt(document.getElementById('setting-scan-interval').value, 10),
      log_retention_days: parseInt(document.getElementById('setting-retention').value, 10),
    });
    btn.textContent = '✅ Saved!';
  } catch (e) {
    btn.textContent = '❌ Error';
  }
  setTimeout(() => { btn.textContent = '💾 Save Settings'; }, 2000);
}

async function runSetupCheck() {
  const wizard = document.getElementById('setup-wizard');
  wizard.style.display = 'flex';
  await loadSetupChecks();
}

// ── Setup Wizard ─────────────────────────────────────────────────────

async function loadSetupChecks() {
  const container = document.getElementById('wizard-checks');
  container.innerHTML = '<p style="color:var(--text-muted)">Running checks...</p>';

  try {
    const res = await API.get('/setup/check');
    container.innerHTML = (res.checks || []).map(c => {
      const iconClass = c.status === 'ok' ? 'check-ok' : c.status === 'warning' ? 'check-warn' : 'check-error';
      const icon = c.status === 'ok' ? '✅' : c.status === 'warning' ? '⚠️' : '❌';
      return `
        <div class="check-item">
          <div class="check-icon ${iconClass}">${icon}</div>
          <div class="check-info">
            <div class="check-name">${c.name}</div>
            <div class="check-message">${c.message}</div>
          </div>
        </div>
      `;
    }).join('');
  } catch (e) {
    container.innerHTML = '<p style="color:var(--color-error)">Failed to run checks</p>';
  }
}

// ── Live Feed ────────────────────────────────────────────────────────

function setupLiveFeed() {
  ws.on((data) => {
    // Update dashboard stats on any event
    if (currentPage === 'dashboard') {
      loadDashboardData();
    }

    // Add to live feed if on dashboard
    if (data.type === 'new_download' && currentPage === 'dashboard') {
      const feed = document.getElementById('live-feed');
      if (!feed) return;

      // Clear empty state
      if (feed.querySelector('.empty-state')) feed.innerHTML = '';

      const item = document.createElement('div');
      item.className = 'fade-in';
      item.style.cssText = 'padding:8px 0;border-bottom:1px solid var(--border-subtle);font-size:0.85rem';
      item.innerHTML = `
        ${serviceBadge(data.service)}
        <span style="color:var(--text-secondary);margin-left:8px">${data.client_ip}</span>
        <span style="float:right;color:var(--text-muted)">${formatBytes(data.bytes)}</span>
      `;

      feed.insertBefore(item, feed.firstChild);

      // Keep max 50 items
      while (feed.children.length > 50) feed.removeChild(feed.lastChild);
    }

    if (data.type === 'network_traffic') {
      updateNetTrafficChart(data.interfaces);
    }
  });
}

// ── Network Traffic Chart ──────────────────────────────────────────

let netTrafficHistory = [];

function updateNetTrafficChart(interfaces) {
  let totalRx = 0;
  let totalTx = 0;
  let activeInts = [];

  for (const [name, data] of Object.entries(interfaces)) {
    totalRx += data.rx_bytes_sec;
    totalTx += data.tx_bytes_sec;
    activeInts.push(`${name}: rx ${formatBytes(data.rx_bytes_sec)}/s, tx ${formatBytes(data.tx_bytes_sec)}/s`);
  }

  const detailsEl = document.getElementById('net-interfaces-list');
  if (detailsEl) {
    detailsEl.innerHTML = activeInts.length > 0
      ? activeInts.join(' | ')
      : 'No active network interfaces detected';
  }

  netTrafficHistory.push({ rx: totalRx, tx: totalTx });
  if (netTrafficHistory.length > 60) {
    netTrafficHistory.shift();
  }

  drawNetTrafficChart();
}

function drawNetTrafficChart() {
  const canvas = document.getElementById('net-traffic-chart');
  if (!canvas) return;

  const ctx = canvas.getContext('2d');
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width;
  canvas.height = rect.height;

  const w = canvas.width;
  const h = canvas.height;

  ctx.clearRect(0, 0, w, h);

  if (netTrafficHistory.length < 2) {
    ctx.fillStyle = 'rgba(255, 255, 255, 0.4)';
    ctx.font = '11px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('Waiting for network traffic data...', w / 2, h / 2);
    return;
  }

  let maxVal = 1024 * 1024; // Min 1MB/s scale
  for (const point of netTrafficHistory) {
    if (point.rx > maxVal) maxVal = point.rx;
    if (point.tx > maxVal) maxVal = point.tx;
  }
  maxVal *= 1.1;

  const getX = (index) => (index / 59) * w;
  const getY = (value) => h - (value / maxVal) * (h - 24) - 12;

  ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)';
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let i = 1; i < 4; i++) {
    const y = (i / 4) * h;
    ctx.moveTo(0, y);
    ctx.lineTo(w, y);
  }
  ctx.stroke();

  const drawLine = (dataKey, strokeColor, fillColor) => {
    ctx.beginPath();
    ctx.moveTo(getX(0), getY(netTrafficHistory[0][dataKey]));
    for (let i = 1; i < netTrafficHistory.length; i++) {
      ctx.lineTo(getX(i), getY(netTrafficHistory[i][dataKey]));
    }
    ctx.strokeStyle = strokeColor;
    ctx.lineWidth = 2;
    ctx.stroke();

    ctx.lineTo(getX(netTrafficHistory.length - 1), h);
    ctx.lineTo(getX(0), h);
    ctx.closePath();
    ctx.fillStyle = fillColor;
    ctx.fill();
  };

  drawLine('rx', 'rgba(99, 102, 241, 0.85)', 'rgba(99, 102, 241, 0.08)');
  drawLine('tx', 'rgba(168, 85, 247, 0.85)', 'rgba(168, 85, 247, 0.04)');

  ctx.font = '10px monospace';
  ctx.textAlign = 'right';

  const currentRx = netTrafficHistory[netTrafficHistory.length - 1].rx;
  const currentTx = netTrafficHistory[netTrafficHistory.length - 1].tx;

  ctx.fillStyle = '#818cf8';
  ctx.fillText(`📥 Down: ${formatBytes(currentRx)}/s`, w - 10, 18);
  ctx.fillStyle = '#c084fc';
  ctx.fillText(`📤 Up: ${formatBytes(currentTx)}/s`, w - 10, 32);
  ctx.fillStyle = 'rgba(255, 255, 255, 0.35)';
  ctx.fillText(`Max: ${formatBytes(maxVal)}/s`, w - 10, 46);
}

// ── Page: Logs ───────────────────────────────────────────────────────

let logsInterval = null;

async function renderLogs(container) {
  if (logsInterval) clearInterval(logsInterval);

  container.innerHTML = `
    <div class="card" style="display:flex;flex-direction:column;height:calc(100vh - 160px)">
      <div class="card-header" style="justify-content:space-between">
        <div class="card-title">📜 Backend System Logs</div>
        <div style="display:flex;gap:8px">
          <button class="btn btn-ghost" id="btn-refresh-logs">🔄 Refresh</button>
          <button class="btn btn-ghost" id="btn-clear-logs" style="color:var(--color-miss)">🗑️ Clear UI</button>
        </div>
      </div>
      <div id="logs-console" style="flex:1;background:#05070c;border:1px solid var(--border-subtle);border-radius:6px;padding:var(--space-md);font-family:var(--font-mono);font-size:0.82rem;overflow-y:auto;white-space:pre-wrap;color:#a9b2c3;line-height:1.45;margin-top:12px">
        <div style="color:var(--text-muted)">Loading system logs...</div>
      </div>
    </div>
  `;

  async function fetchLogs() {
    try {
      const logs = await API.get('/logs');
      const consoleBox = document.getElementById('logs-console');
      if (!consoleBox) return;

      if (!logs || logs.length === 0) {
        consoleBox.innerHTML = '<div style="color:var(--text-muted)">No logs recorded yet.</div>';
        return;
      }

      consoleBox.innerHTML = logs.map(line => {
        let color = '#e2e8f0';
        if (line.includes('ERROR') || line.includes('error')) color = '#f87171';
        else if (line.includes('WARN') || line.includes('warn')) color = '#fbbf24';
        else if (line.includes('INFO') || line.includes('info')) color = '#60a5fa';
        else if (line.includes('DEBUG') || line.includes('debug')) color = '#c084fc';

        return `<div style="color:${color};margin-bottom:2px">${escapeHtml(line)}</div>`;
      }).join('');

      consoleBox.scrollTop = consoleBox.scrollHeight;
    } catch (e) {
      const consoleBox = document.getElementById('logs-console');
      if (consoleBox) consoleBox.innerHTML = `<div style="color:var(--color-miss)">Failed to load logs: ${e.message}</div>`;
    }
  }

  function escapeHtml(text) {
    return text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#039;");
  }

  fetchLogs();
  logsInterval = setInterval(fetchLogs, 3000);

  document.getElementById('btn-refresh-logs').addEventListener('click', fetchLogs);
  document.getElementById('btn-clear-logs').addEventListener('click', () => {
    const consoleBox = document.getElementById('logs-console');
    if (consoleBox) consoleBox.innerHTML = '<div style="color:var(--text-muted)">UI Cleared. Waiting for new logs...</div>';
  });

  const observer = new MutationObserver((mutations, obs) => {
    if (!document.getElementById('logs-console')) {
      clearInterval(logsInterval);
      logsInterval = null;
      obs.disconnect();
    }
  });
  observer.observe(container, { childList: true });
}

// ── Init ─────────────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', () => {
  // Navigation
  document.querySelectorAll('.nav-item').forEach(el => {
    el.addEventListener('click', () => navigate(el.dataset.page));
  });

  // Setup wizard buttons
  document.getElementById('wizard-skip').addEventListener('click', () => {
    document.getElementById('setup-wizard').style.display = 'none';
  });
  document.getElementById('wizard-recheck').addEventListener('click', loadSetupChecks);

  // Connect WebSocket
  ws.connect();
  setupLiveFeed();

  // Load initial page
  navigate('dashboard');

  // Check if first run (setup wizard)
  API.get('/setup/check').then(res => {
    const hasError = (res.checks || []).some(c => c.status === 'error');
    if (hasError) {
      document.getElementById('setup-wizard').style.display = 'flex';
      loadSetupChecks();
    }
  }).catch(() => {});
});
