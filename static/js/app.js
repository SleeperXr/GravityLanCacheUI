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
  const cls = 'badge badge-service badge-' + (escapeHtml(service) || 'other');
  return `<span class="${cls}">${escapeHtml(service) || 'other'}</span>`;
}

function escapeHtml(text) {
  if (text === null || text === undefined) return '';
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function hitRateBadge(rate) {
  const cls = rate >= 90 ? 'badge-hit' : rate >= 10 ? 'badge-warning' : 'badge-error';
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

function getGameArtHtml(service, gameName, appId, downloadId) {
  let innerHtml = '';
  let borderAccent = 'rgba(255,255,255,0.05)';
  const displayLabel = service ? escapeHtml(service.toUpperCase()) : 'GAME';
  
  if (service === 'steam' && appId) {
    innerHtml = `<img src="https://cdn.akamai.steamstatic.com/steam/apps/${appId}/capsule_184x69.jpg" 
                      alt="${escapeHtml(gameName) || 'Steam'}" 
                      style="width:100%;height:100%;object-fit:cover;border-radius:4px"
                      onerror="this.style.display='none';this.nextElementSibling.style.display='flex'">
                 <div style="display:none;width:100%;height:100%;align-items:center;justify-content:center;font-size:0.65rem;font-weight:700;color:#818cf8;background:rgba(99,102,241,0.1)">STEAM</div>`;
    borderAccent = 'rgba(99, 102, 241, 0.3)';
  } else {
    // Sleek placeholders for other services
    let bg = 'rgba(255,255,255,0.03)';
    let color = 'var(--text-muted)';
    
    if (service === 'steam') { bg = 'rgba(27, 40, 56, 0.8)'; color = '#66c0f4'; borderAccent = 'rgba(102, 192, 244, 0.3)'; }
    else if (service === 'epic' || service === 'epicgames') { bg = 'rgba(32, 32, 32, 0.8)'; color = '#f5f5f5'; borderAccent = 'rgba(255,255,255,0.2)'; }
    else if (service === 'battlenet') { bg = 'rgba(0, 174, 240, 0.1)'; color = '#00aef0'; borderAccent = 'rgba(0, 174, 240, 0.3)'; }
    else if (service === 'xbox' || service === 'windowsupdate') { bg = 'rgba(16, 124, 16, 0.1)'; color = '#107c10'; borderAccent = 'rgba(16, 124, 16, 0.3)'; }
    else if (service === 'nintendo') { bg = 'rgba(224, 0, 0, 0.1)'; color = '#e00000'; borderAccent = 'rgba(224, 0, 0, 0.3)'; }
    else if (service === 'playstation') { bg = 'rgba(0, 48, 143, 0.1)'; color = '#00308f'; borderAccent = 'rgba(0, 48, 143, 0.3)'; }
    
    innerHtml = `<div style="width:100%;height:100%;display:flex;align-items:center;justify-content:center;font-size:0.65rem;font-weight:800;color:${color};background:${bg}">${displayLabel}</div>`;
  }
  
  return `<div class="game-art-container" style="width:80px;height:30px;background:#05070c;border:1px solid ${borderAccent};border-radius:4px;overflow:hidden;position:relative;flex-shrink:0;box-shadow:0 2px 4px rgba(0,0,0,0.3);display:flex;align-items:center;justify-content:center">
    ${innerHtml}
  </div>`;
}

// ── API Client ───────────────────────────────────────────────────────

const API = {
  async get(endpoint) {
    const headers = {};
    const apiKey = localStorage.getItem('gravity_api_key');
    if (apiKey) headers['X-API-Key'] = apiKey;

    const res = await fetch('/api/v1' + endpoint, { headers });
    if (res.status === 401) { handleUnauthorized(); throw new Error('Unauthorized'); }
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  },

  async put(endpoint, data) {
    const headers = { 'Content-Type': 'application/json' };
    const apiKey = localStorage.getItem('gravity_api_key');
    if (apiKey) headers['X-API-Key'] = apiKey;

    const res = await fetch('/api/v1' + endpoint, {
      method: 'PUT',
      headers,
      body: JSON.stringify(data),
    });
    if (res.status === 401) { handleUnauthorized(); throw new Error('Unauthorized'); }
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  },

  async post(endpoint) {
    const headers = {};
    const apiKey = localStorage.getItem('gravity_api_key');
    if (apiKey) headers['X-API-Key'] = apiKey;

    const res = await fetch('/api/v1' + endpoint, { method: 'POST', headers });
    if (res.status === 401) { handleUnauthorized(); throw new Error('Unauthorized'); }
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
  },
};

function handleUnauthorized() {
  const key = prompt('GravityLancacheUI is protected by an API Key. Please enter the API Key:');
  if (key) {
    localStorage.setItem('gravity_api_key', key.trim());
    location.reload();
  }
}

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
      <div class="card-header" style="justify-content:space-between;align-items:center;flex-wrap:wrap;gap:12px">
        <div class="card-title" style="display:flex;align-items:center;gap:12px">
          <span>📶 Live Network Traffic (Unraid Host)</span>
          <select id="net-interface-select" style="background:var(--bg-input);color:var(--text-primary);border:1px solid var(--border-subtle);border-radius:6px;padding:4px 10px;font-size:0.75rem;cursor:pointer;outline:none;font-weight:600">
            <option value="all">All Interfaces</option>
          </select>
        </div>
        <div id="net-interfaces-list" style="font-size:0.8rem;color:var(--text-muted);font-family:var(--font-mono)">
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
    populateLiveFeed(dlRes.downloads.slice(0, 10));

    if (dashRes.parser_status) {
      updateParserProgress(dashRes.parser_status);
    }
  } catch (e) {
    console.error('Dashboard load failed:', e);
  }
}

function updateParserProgress(status) {
  const container = document.getElementById('parser-progress-container');
  const label = document.getElementById('parser-progress-label');
  const wrapper = document.getElementById('parser-progress-bar-wrapper');
  const bar = document.getElementById('parser-progress-bar');
  const text = document.getElementById('parser-progress-text');

  if (!container || !bar || !text) return;

  container.style.display = 'flex';

  if (status.is_catching_up) {
    container.classList.remove('live');
    if (label) {
      label.classList.remove('live');
      label.textContent = '⚡ SCANNING...';
    }
    if (wrapper) wrapper.style.display = 'block';
    
    // Ensure at least 1.5% is visible if progress is > 0 but tiny
    const displayPercent = status.percentage > 0 ? Math.max(1.5, status.percentage) : 0;
    bar.style.width = displayPercent.toFixed(1) + '%';
    
    const scannedStr = formatBytes(status.current_offset);
    const totalStr = formatBytes(status.total_size);
    text.textContent = `${status.percentage.toFixed(1)}% (${scannedStr} / ${totalStr})`;
  } else {
    container.classList.add('live');
    if (label) {
      label.classList.add('live');
      label.textContent = '🟢 SCANNING LIVE';
    }
    if (wrapper) wrapper.style.display = 'none';
    text.textContent = `Offset: ${formatBytes(status.current_offset)}`;
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
            <td title="${escapeHtml(d.ended_at)}">${timeAgo(d.ended_at)}</td>
            <td>${serviceBadge(d.service)}</td>
            <td>
              <div style="display:flex;align-items:center;gap:12px">
                ${getGameArtHtml(d.service, d.game_name, d.app_id, d.download_id)}
                <div style="display:flex;flex-direction:column;gap:1px;align-items:flex-start">
                  <span style="font-weight:700;color:var(--text-primary);font-size:0.95rem;text-align:left">${escapeHtml(d.game_name || d.download_id || '—')}</span>
                  ${d.game_name ? `<span style="font-size:0.75rem;color:var(--text-muted);font-family:var(--font-mono)">ID: ${escapeHtml(d.download_id || '—')}</span>` : ''}
                </div>
              </div>
            </td>
            <td style="font-family:var(--font-mono);font-size:0.8rem">${escapeHtml(d.client_ip)}</td>
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

    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header"><div class="card-title">💾 Cached Platforms & Games</div></div>
      <div class="table-container" id="cache-games-table">
        <div class="empty-state"><div class="icon">⏳</div>Loading cached games...</div>
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

  loadCacheData();
}

async function loadCacheData() {
  try {
    const res = await API.get('/cache/latest');
    
    // Update stats cards
    if (res.snapshot) {
      document.getElementById('cache-size').textContent = formatBytes(res.snapshot.total_size_bytes);
      document.getElementById('cache-files').textContent = res.snapshot.total_files.toLocaleString() + ' files';
      if (res.snapshot.taken_at) {
        document.getElementById('cache-last-scan').textContent = timeAgo(res.snapshot.taken_at);
      } else {
        document.getElementById('cache-last-scan').textContent = 'Completed';
      }
    } else {
      document.getElementById('cache-size').textContent = 'No scans yet';
      document.getElementById('cache-files').textContent = '0 files';
      document.getElementById('cache-last-scan').textContent = 'Never';
    }

    // Render cached games table
    const tableContainer = document.getElementById('cache-games-table');
    if (tableContainer) {
      let items = [];
      let isDiskCache = false;
      
      if (res.snapshot && res.snapshot.details_json) {
        try {
          const details = JSON.parse(res.snapshot.details_json);
          if (details && details.items && details.items.length > 0) {
            items = details.items;
            isDiskCache = true;
          }
        } catch (e) {
          console.error('Failed to parse cache details JSON:', e);
        }
      }
      
      if (!isDiskCache) {
        // Fallback to traffic log statistics
        items = (res.games || []).map(g => ({
          name: g.name,
          service: g.service,
          app_id: g.app_id,
          size_bytes: g.total_bytes,
          file_count: null,
          hit_rate: g.total_bytes > 0 ? (g.hit_bytes / g.total_bytes) * 100 : 0,
          last_accessed: g.last_downloaded,
        }));
      } else {
        // Sort disk cache items by size descending
        items.sort((a, b) => b.size_bytes - a.size_bytes);
        items = items.map(i => {
          const displayName = i.game_name || i.download_id || i.service.toUpperCase();
          // Cross-reference with traffic log stats for hit rate and last accessed
          const match = (res.games || []).find(g => 
            g.service === i.service && 
            (g.name === i.game_name || (i.app_id && g.app_id === i.app_id) || (i.download_id && g.download_id === i.download_id))
          );
          return {
            name: displayName,
            service: i.service,
            app_id: i.app_id,
            size_bytes: i.size_bytes,
            file_count: i.file_count,
            hit_rate: match ? (match.total_bytes > 0 ? (match.hit_bytes / match.total_bytes) * 100 : 0) : null,
            last_accessed: match ? match.last_downloaded : null,
          };
        });
      }

      if (items.length === 0) {
        tableContainer.innerHTML = '<div class="empty-state"><div class="icon">💾</div>No cached files or games recorded yet</div>';
      } else {
        tableContainer.innerHTML = `
          <table>
            <thead>
              <tr>
                <th>Game / Platform</th>
                <th>Service</th>
                <th>Size ${isDiskCache ? 'on Disk' : 'Cached'}</th>
                <th>Details</th>
                <th>Hit Rate</th>
                <th>Last Accessed</th>
              </tr>
            </thead>
            <tbody>
              ${items.map(g => {
                const detailsText = g.file_count !== null ? `${g.file_count.toLocaleString()} files` : 'Traffic log';
                return `
                  <tr>
                    <td>
                      <div style="display:flex;align-items:center;gap:12px">
                        ${getGameArtHtml(g.service, g.name, g.app_id, null)}
                        <div style="display:flex;flex-direction:column;gap:1px;align-items:flex-start">
                          <span style="font-weight:700;color:var(--text-primary);font-size:0.95rem;text-align:left">${escapeHtml(g.name)}</span>
                        </div>
                      </div>
                    </td>
                    <td>${serviceBadge(g.service)}</td>
                    <td><span style="font-weight:700;color:var(--text-primary)">${formatBytes(g.size_bytes)}</span></td>
                    <td style="color:var(--text-muted);font-size:0.8rem">${detailsText}</td>
                    <td>${g.hit_rate !== null ? hitRateBadge(g.hit_rate) : '<span class="badge badge-service badge-other">—</span>'}</td>
                    <td>${g.last_accessed ? timeAgo(g.last_accessed) : '—'}</td>
                  </tr>
                `;
              }).join('')}
            </tbody>
          </table>
        `;
      }
    }
  } catch (e) {
    console.error('Failed to load cache data:', e);
    const sizeEl = document.getElementById('cache-size');
    if (sizeEl) sizeEl.textContent = 'Error';
    const tableContainer = document.getElementById('cache-games-table');
    if (tableContainer) {
      tableContainer.innerHTML = '<div class="empty-state"><div class="icon">❌</div>Failed to load cache contents</div>';
    }
  }
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
              <td style="font-family:var(--font-mono)">${escapeHtml(c.ip)}</td>
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

// ── Page: Prefill ────────────────────────────────────────────────────

function updateNextRunDisplay() {
  const timeInput = document.getElementById('prefill-schedule-time');
  const steamInput = document.getElementById('prefill-steam-enabled');
  const bnetInput = document.getElementById('prefill-battlenet-enabled');
  const epicInput = document.getElementById('prefill-epic-enabled');
  
  const el = document.getElementById('prefill-scheduler-next-run');
  if (!el || !timeInput) return;
  
  const time = timeInput.value.trim();
  const steam = steamInput ? steamInput.checked : false;
  const bnet = bnetInput ? bnetInput.checked : false;
  const epic = epicInput ? epicInput.checked : false;
  
  if (!steam && !bnet && !epic) {
    el.textContent = 'Scheduler is inactive (no platforms enabled)';
    return;
  }
  
  const timeRegex = /^([01]\d|2[0-3]):([0-5]\d)$/;
  if (!timeRegex.test(time)) {
    el.textContent = 'Invalid time format';
    return;
  }
  
  const [hours, minutes] = time.split(':').map(Number);
  const now = new Date();
  const scheduledTime = new Date();
  scheduledTime.setHours(hours, minutes, 0, 0);
  
  if (scheduledTime <= now) {
    scheduledTime.setDate(scheduledTime.getDate() + 1);
  }
  
  const options = { weekday: 'short', hour: '2-digit', minute: '2-digit' };
  el.innerHTML = `⏰ Next run: <strong style="color:var(--accent-primary-light)">${scheduledTime.toLocaleDateString(undefined, options)}</strong>`;
}

async function renderPrefill(container) {
  container.innerHTML = `
    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header">
        <div class="card-title">Cache Prefill Manager</div>
      </div>
      <p style="color:var(--text-secondary);margin-bottom:16px">
        Pre-warm your LanCache by downloading game data before your clients need it.
        Integrates directly with SteamPrefill, BattleNetPrefill, and EpicPrefill.
      </p>
    </div>

    <!-- Configuration & Schedule -->
    <div class="grid-3" style="margin-bottom:var(--space-md)">
      <div class="card" style="grid-column: span 3">
        <div class="card-header">
          <div class="card-title">📅 Automatic Prefill Scheduler</div>
        </div>
        <div style="display:grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin-bottom: 20px;">
          <div>
            <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">Trigger Time (HH:MM Local Time)</label>
            <input class="input" id="prefill-schedule-time" type="text" placeholder="e.g. 02:00" value="02:00" style="max-width:150px">
          </div>
          <div>
            <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">Enabled Platforms</label>
            <div style="display:flex; flex-direction:column; gap:8px; margin-top:8px;">
              <label style="display:flex; align-items:center; gap:8px; cursor:pointer; font-size:0.85rem;">
                <input type="checkbox" id="prefill-steam-enabled"> SteamPrefill
              </label>
              <label style="display:flex; align-items:center; gap:8px; cursor:pointer; font-size:0.85rem;">
                <input type="checkbox" id="prefill-battlenet-enabled"> BattleNetPrefill
              </label>
              <label style="display:flex; align-items:center; gap:8px; cursor:pointer; font-size:0.85rem;">
                <input type="checkbox" id="prefill-epic-enabled"> EpicPrefill
              </label>
            </div>
          </div>
          <div>
            <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">Startup Options</label>
            <label style="display:flex; align-items:center; gap:8px; cursor:pointer; font-size:0.85rem; margin-top:8px;">
              <input type="checkbox" id="prefill-startup-enabled"> Run scheduled prefills on container startup
            </label>
          </div>
        </div>
        <div style="display:flex; align-items:center; flex-wrap:wrap; gap:16px;">
          <button class="btn" onclick="savePrefillConfig()" id="btn-save-prefill-config">💾 Save Schedule Configuration</button>
          <span id="prefill-scheduler-next-run" style="font-size: 0.85rem; font-family: var(--font-mono); font-weight: 500;"></span>
          <span id="prefill-config-status" style="font-size: 0.85rem; font-weight:700;"></span>
        </div>
      </div>
    </div>

    <!-- Platforms Grid -->
    <div class="grid-3" id="prefill-platforms">
      <div class="empty-state">⏳ Loading prefill status...</div>
    </div>
  `;

  loadPrefillConfig();
  loadPrefillStatus();
  
  // Attach change listeners to update next run display immediately
  setTimeout(() => {
    const timeEl = document.getElementById('prefill-schedule-time');
    const steamEl = document.getElementById('prefill-steam-enabled');
    const bnetEl = document.getElementById('prefill-battlenet-enabled');
    const epicEl = document.getElementById('prefill-epic-enabled');
    if (timeEl) timeEl.addEventListener('input', updateNextRunDisplay);
    if (steamEl) steamEl.addEventListener('change', updateNextRunDisplay);
    if (bnetEl) bnetEl.addEventListener('change', updateNextRunDisplay);
    if (epicEl) epicEl.addEventListener('change', updateNextRunDisplay);
  }, 100);
  
  // Set up periodic status reload
  window.prefillStatusTimer = setInterval(loadPrefillStatus, 5000);
}

// Ensure timer is cleared when page changes
const originalChangePage = window.changePage;
window.changePage = function(pageId) {
  if (window.prefillStatusTimer) {
    clearInterval(window.prefillStatusTimer);
    window.prefillStatusTimer = null;
  }
  if (originalChangePage) {
    originalChangePage(pageId);
  }
};

async function loadPrefillConfig() {
  try {
    const config = await API.get('/prefill/config');
    document.getElementById('prefill-schedule-time').value = config.cron_schedule || '02:00';
    document.getElementById('prefill-steam-enabled').checked = config.steam_enabled;
    document.getElementById('prefill-battlenet-enabled').checked = config.battlenet_enabled;
    document.getElementById('prefill-epic-enabled').checked = config.epic_enabled;
    document.getElementById('prefill-startup-enabled').checked = config.run_on_startup;
    updateNextRunDisplay();
  } catch (e) {
    console.error('Failed to load prefill config:', e);
  }
}

async function savePrefillConfig() {
  const btn = document.getElementById('btn-save-prefill-config');
  const status = document.getElementById('prefill-config-status');
  if (btn) btn.disabled = true;
  
  const scheduleTime = document.getElementById('prefill-schedule-time').value.trim();
  const timeRegex = /^([01]\d|2[0-3]):([0-5]\d)$/;
  if (!timeRegex.test(scheduleTime)) {
    if (status) {
      status.style.color = 'var(--color-error)';
      status.textContent = '❌ Invalid time format! Use HH:MM (e.g. 02:00)';
    }
    if (btn) btn.disabled = false;
    return;
  }

  const payload = {
    steam_enabled: document.getElementById('prefill-steam-enabled').checked,
    battlenet_enabled: document.getElementById('prefill-battlenet-enabled').checked,
    epic_enabled: document.getElementById('prefill-epic-enabled').checked,
    cron_schedule: scheduleTime,
    run_on_startup: document.getElementById('prefill-startup-enabled').checked,
  };

  try {
    await API.put('/prefill/config', payload);
    if (status) {
      status.style.color = 'var(--color-hit)';
      status.textContent = '✅ Schedule saved successfully!';
    }
    updateNextRunDisplay();
  } catch (e) {
    if (status) {
      status.style.color = 'var(--color-error)';
      status.textContent = `❌ Save failed: ${e.message || e}`;
    }
  } finally {
    if (btn) btn.disabled = false;
    setTimeout(() => { if (status) status.textContent = ''; }, 3000);
  }
}

async function loadPrefillStatus() {
  const grid = document.getElementById('prefill-platforms');
  if (!grid) return;

  try {
    const res = await API.get('/prefill/status');
    const platforms = res.platforms || [];
    
    const platformMeta = {
      steam: { name: 'Steam', folder: 'SteamPrefill' },
      battlenet: { name: 'Battle.net', folder: 'BattleNetPrefill' },
      epic: { name: 'Epic Games', folder: 'EpicPrefill' }
    };

    grid.innerHTML = platforms.map(p => {
      const meta = platformMeta[p.platform] || { name: p.platform.toUpperCase(), folder: p.platform };
      const isConfigured = p.selected_apps && p.selected_apps.length > 0;
      
      let badgeClass = 'badge-miss';
      let badgeText = 'Not configured';
      if (p.running) {
        badgeClass = 'badge-warning';
        badgeText = '⏳ Prefilling...';
      } else if (isConfigured) {
        badgeClass = 'badge-hit';
        badgeText = 'Active';
      }

      const statusDesc = p.running
        ? `⚙️ Run is active in background.`
        : (isConfigured 
            ? `🟢 ${p.selected_apps.length} game(s) selected for prefilling.` 
            : `❌ No games selected yet. Log in via CLI console.`);

      // Dynamic Active App Display
      let activeAppHtml = '';
      if (p.running) {
        if (p.active_app) {
          activeAppHtml = `
            <div style="margin-top: 10px; padding: 10px; background: rgba(245, 158, 11, 0.08); border-left: 3px solid var(--color-warning); border-radius: 4px; font-size: 0.8rem; text-align: left;">
              <div style="font-weight:700; color:var(--color-warning); margin-bottom: 2px;">⚡ Prefilling Game:</div>
              <div style="font-weight:600; color:var(--text-primary); font-family:var(--font-mono); word-break:break-all;">${escapeHtml(p.active_app)}</div>
            </div>
          `;
        }
      }

      // Dynamic History & Summary Display
      let historyHtml = '';
      if (!p.running) {
        if (p.last_run) {
          historyHtml += `
            <div style="margin-top: 10px; font-size: 0.8rem; color:var(--text-secondary); text-align: left;">
              <strong>Last Run:</strong> <span style="font-family:var(--font-mono); color:var(--text-primary);">${escapeHtml(p.last_run)}</span>
            </div>
          `;
        }
        if (p.completed_apps && p.completed_apps.length > 0) {
          historyHtml += `
            <div style="margin-top: 8px; padding: 8px; background: rgba(16, 185, 129, 0.05); border-radius: 4px; font-size: 0.75rem; border: 1px solid rgba(16, 185, 129, 0.1); text-align: left;">
              <div style="font-weight:700; color:var(--color-hit); font-size:0.7rem; text-transform:uppercase; margin-bottom: 3px;">Prefilled in last run:</div>
              <div style="color:var(--text-primary); font-family:var(--font-mono); max-height: 60px; overflow-y: auto; line-height: 1.4;">
                ${p.completed_apps.map(app => escapeHtml(app)).join(', ')}
              </div>
            </div>
          `;
        }
      }

      let summaryHtml = '';
      if (p.last_log_summary) {
        summaryHtml = `
          <div style="margin-top: 6px; font-size: 0.75rem; color:var(--text-muted); font-family:var(--font-mono); word-break:break-word; text-align: left; line-height:1.3;">
            📝 ${escapeHtml(p.last_log_summary)}
          </div>
        `;
      }

      return `
        <div class="card" style="display:flex; flex-direction:column; justify-content:space-between; height: 100%;">
          <div>
            <div class="card-header">
              <div class="card-title">${meta.name}</div>
              <span class="badge ${badgeClass}">${badgeText}</span>
            </div>
            <p style="color:var(--text-secondary);font-size:0.85rem;margin-bottom:12px; min-height:40px;">
              ${statusDesc}
            </p>
            ${activeAppHtml}
            ${historyHtml}
            ${summaryHtml}
          </div>
          <div style="margin-top:auto; display:flex; flex-direction:column; gap:8px; padding-top: 12px;">
            <div style="display:grid; grid-template-columns: 1fr 1fr; gap:8px;">
              <button class="btn btn-ghost" onclick="openInteractiveConsole('${p.platform}')" style="font-size:0.75rem; padding: 8px 6px;" ${p.running ? 'disabled' : ''}>
                🖥️ Setup Console
              </button>
              <button class="btn btn-ghost" onclick="openPrefillLogs('${p.platform}')" style="font-size:0.75rem; padding: 8px 6px;">
                📄 View Logs
              </button>
            </div>
            <button class="btn" onclick="runPrefill('${p.platform}')" id="prefill-btn-${p.platform}" style="width:100%" ${p.running ? 'disabled' : ''}>
              ${p.running ? '⏳ Prefilling...' : '⚡ Run Prefill'}
            </button>
            <div id="prefill-error-${p.platform}" style="color:var(--color-error); font-size:0.75rem; margin-top:8px; display:none; text-align:left; word-break:break-word;"></div>
          </div>
        </div>
      `;
    }).join('');
  } catch (e) {
    grid.innerHTML = `<div class="empty-state" style="color:var(--color-error)">❌ Failed to load prefill platforms status: ${escapeHtml(e.message || e)}</div>`;
  }
}

async function runPrefill(platform) {
  const btn = document.getElementById('prefill-btn-' + platform);
  const errorDiv = document.getElementById('prefill-error-' + platform);
  if (errorDiv) {
    errorDiv.style.display = 'none';
    errorDiv.textContent = '';
  }
  if (btn) {
    btn.textContent = '⏳ Starting...';
    btn.disabled = true;
  }

  try {
    await API.post('/prefill/run/' + platform);
    if (btn) btn.textContent = '⏳ Running...';
    loadPrefillStatus();
    // Open logs automatically so user sees the progress!
    setTimeout(() => openPrefillLogs(platform), 1000);
  } catch (e) {
    if (btn) btn.textContent = '❌ Failed';
    if (errorDiv) {
      errorDiv.textContent = `Error: ${e.message || e.error || e}`;
      errorDiv.style.display = 'block';
    }
  }
}

function openPrefillLogs(platform) {
  const existing = document.getElementById('prefill-logs-modal');
  if (existing) existing.remove();

  const modal = document.createElement('div');
  modal.id = 'prefill-logs-modal';
  modal.className = 'modal-overlay';
  modal.innerHTML = `
    <div class="modal-card" style="max-width: 900px;">
      <div class="card-header" style="padding:16px 24px; border-bottom:1px solid var(--border-subtle)">
        <div class="card-title">📄 Prefill Logs: ${platform.toUpperCase()}</div>
      </div>
      <div class="modal-body" style="background:#02040a;">
        <pre id="prefill-logs-content" class="terminal-window log-mode">⏳ Fetching logs...</pre>
      </div>
      <div class="modal-footer">
        <button class="btn btn-ghost" onclick="fetchLastPrefillLog('${platform}')">🔄 Refresh</button>
        <button class="btn" onclick="document.getElementById('prefill-logs-modal').remove()">Close</button>
      </div>
    </div>
  `;
  document.body.appendChild(modal);

  fetchLastPrefillLog(platform);

  const intervalId = setInterval(() => {
    const modalEl = document.getElementById('prefill-logs-modal');
    if (!modalEl) {
      clearInterval(intervalId);
      return;
    }
    fetchLastPrefillLog(platform);
  }, 2000);
}

async function fetchLastPrefillLog(platform) {
  const logsPre = document.getElementById('prefill-logs-content');
  if (!logsPre) return;

  try {
    const res = await API.get('/prefill/log/' + platform);
    logsPre.textContent = res.log || 'No logs recorded.';
    logsPre.scrollTop = logsPre.scrollHeight;
  } catch (e) {
    logsPre.textContent = `Error reading log file: ${e.message || e}`;
  }
}

function openInteractiveConsole(platform) {
  const existing = document.getElementById('prefill-interactive-modal');
  if (existing) existing.remove();

  const modal = document.createElement('div');
  modal.id = 'prefill-interactive-modal';
  modal.className = 'modal-overlay';
  modal.innerHTML = `
    <div class="modal-card">
      <div class="card-header" style="padding:16px 24px; border-bottom:1px solid var(--border-subtle)">
        <div class="card-title">🖥️ Interactive Setup Console: ${platform.toUpperCase()}</div>
      </div>
      <div class="modal-body">
        <div class="alert alert-info" style="font-size:0.75rem; margin-bottom:12px; padding: 8px 12px; background:rgba(59, 130, 246, 0.08); border:1px solid rgba(59, 130, 246, 0.2)">
          💡 Type inputs (like username/password/Steam Guard) in the box below and press Enter. 
          Use the controller buttons to navigate menus (requires arrow keys/space to check games).
        </div>
        <pre id="interactive-terminal" class="terminal-window">⏳ Connecting to console session...</pre>
        
        <div class="terminal-input-wrapper">
          <input class="input" id="terminal-input" type="text" placeholder="Type input here..." autofocus style="background:#0c1020;">
          <button class="btn" id="terminal-send-btn">Send</button>
        </div>
        
        <div class="terminal-controls">
          <button class="terminal-ctrl-btn" onclick="sendTerminalKey('\\\\x1B[A')">⬆️ Up</button>
          <button class="terminal-ctrl-btn" onclick="sendTerminalKey('\\\\x1B[B')">⬇️ Down</button>
          <button class="terminal-ctrl-btn" onclick="sendTerminalKey(' ')">⎵ Space (Select)</button>
          <button class="terminal-ctrl-btn" onclick="sendTerminalKey('\\\\r\\\\n')">↵ Enter (Confirm)</button>
        </div>
      </div>
      <div class="modal-footer">
        <button class="btn" onclick="closeInteractiveConsole()">Close Console</button>
      </div>
    </div>
  `;
  document.body.appendChild(modal);

  setTimeout(() => {
    const input = document.getElementById('terminal-input');
    if (input) input.focus();
  }, 100);

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const wsUrl = `${protocol}//${window.location.host}/api/v1/prefill/interactive/${platform}`;
  
  const ws = new WebSocket(wsUrl);
  window.prefillWs = ws;
  const term = document.getElementById('interactive-terminal');

  ws.onopen = () => {
    term.textContent = '';
    term.textContent += '[Connected to prefill setup session]\\n';
  };

  ws.onmessage = (event) => {
    term.textContent += event.data;
    term.scrollTop = term.scrollHeight;
  };

  ws.onerror = (event) => {
    term.textContent += '\\n[WebSocket error encountered]\\n';
  };

  ws.onclose = () => {
    term.textContent += '\\n[Session disconnected]\\n';
  };

  const inputEl = document.getElementById('terminal-input');
  inputEl.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      sendTerminalInput();
    }
  });

  const sendBtn = document.getElementById('terminal-send-btn');
  sendBtn.addEventListener('click', sendTerminalInput);
}

function sendTerminalInput() {
  const inputEl = document.getElementById('terminal-input');
  if (!inputEl || !window.prefillWs || window.prefillWs.readyState !== WebSocket.OPEN) return;
  
  const text = inputEl.value;
  window.prefillWs.send(text);
  inputEl.value = '';
}

function sendTerminalKey(key) {
  if (window.prefillWs && window.prefillWs.readyState === WebSocket.OPEN) {
    const parsedKey = key.replace(/\\\\x1B/g, '\\x1B').replace(/\\\\r/g, '\\r').replace(/\\\\n/g, '\\n');
    window.prefillWs.send(parsedKey);
  }
}

function closeInteractiveConsole() {
  if (window.prefillWs) {
    window.prefillWs.close();
    window.prefillWs = null;
  }
  const modal = document.getElementById('prefill-interactive-modal');
  if (modal) modal.remove();
  loadPrefillStatus();
}

// ── Page: Settings ───────────────────────────────────────────────────

async function renderSettings(container) {
  let config = {};
  try { config = await API.get('/config'); } catch (e) { /* defaults */ }

  container.innerHTML = `
    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header"><div class="card-title">API Keys & Access</div></div>
      <div style="margin-bottom:16px">
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">Steam Web API Key</label>
        <input class="input" id="setting-steam-key" type="password"
          placeholder="Enter your Steam Web API Key..."
          value="${config.steam_api_key_set ? '••••••••••••••••' : ''}">
        <p style="color:var(--text-muted);font-size:0.75rem;margin-top:4px">
          Get one at <a href="https://steamcommunity.com/dev/apikey" target="_blank" style="color:var(--accent-primary-light)">steamcommunity.com/dev/apikey</a>
        </p>
      </div>
      <div style="margin-bottom:16px">
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">GravityUI API Access Key</label>
        <input class="input" id="setting-gravity-key" type="password"
          placeholder="Enter API Key if configured via environment variable..."
          value="${escapeHtml(localStorage.getItem('gravity_api_key') || '')}">
        <p style="color:var(--text-muted);font-size:0.75rem;margin-top:4px">
          Must match the <code>API_KEY</code> environment variable configured in your container.
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
      <div style="margin-bottom:16px">
        <label style="display:block;color:var(--text-secondary);font-size:0.85rem;margin-bottom:6px">Log Scan History (days)</label>
        <input class="input" id="setting-log-scan-days" type="number" min="0"
          value="${config.log_scan_days !== undefined ? config.log_scan_days : 7}">
        <p style="color:var(--text-muted);font-size:0.75rem;margin-top:4px">
          How many days of history the log parser will scan on initial run or reset. Set to 0 to scan the entire log file (not recommended for files > 1GB).
        </p>
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

    <div class="card" style="margin-bottom:var(--space-md)">
      <div class="card-header"><div class="card-title">Maintenance & Tools</div></div>
      <div style="display:flex;gap:20px;flex-wrap:wrap">
        <div style="flex:1;min-width:280px">
          <button class="btn btn-primary" id="btn-update-mappings" style="background:#0284c7;width:100%;justify-content:center">🔄 Update Steam Depot Mappings</button>
          <div style="margin-top:8px;font-size:0.8rem;color:var(--text-secondary);font-weight:700">
            Status: <span style="color:${config.steam_mappings_count > 0 ? 'var(--color-hit)' : 'var(--color-error)'}">${config.steam_mappings_count ? config.steam_mappings_count.toLocaleString() : '0'}</span> Mappings loaded
          </div>
          <p style="color:var(--text-muted);font-size:0.75rem;margin-top:8px">
            Downloads and updates the local Steam Depot mapping database (~7.8 MB CSV file) from GitHub to resolve depot IDs into game names.
          </p>
        </div>
        <div style="flex:1;min-width:280px">
          <button class="btn btn-ghost" id="btn-reset-offset" style="color:var(--color-miss);border-color:var(--color-miss);width:100%;justify-content:center">⚠️ Reset Log Parser Offset</button>
          <p style="color:var(--text-muted);font-size:0.75rem;margin-top:8px">
            Resets the log parser to the beginning of access.log. Use this to parse historical downloads that were not logged or if you cleared your database.
          </p>
        </div>
      </div>
    </div>

    <div style="display:flex;gap:12px">
      <button class="btn btn-primary" id="settings-save">💾 Save Settings</button>
      <button class="btn btn-ghost" id="settings-check">🔧 Run Setup Check</button>
    </div>
  `;

  document.getElementById('settings-save').addEventListener('click', saveSettings);
  document.getElementById('settings-check').addEventListener('click', runSetupCheck);

  document.getElementById('btn-update-mappings').addEventListener('click', async () => {
    const btn = document.getElementById('btn-update-mappings');
    btn.textContent = '⏳ Starting...';
    btn.disabled = true;
    try {
      const res = await API.post('/tools/update_mappings');
      btn.textContent = '✅ Started!';
      alert(res.message || 'Mapping update started in background.');
    } catch (e) {
      btn.textContent = '❌ Error';
      console.error(e);
    }
    setTimeout(() => {
      btn.textContent = '🔄 Update Steam Depot Mappings';
      btn.disabled = false;
    }, 3000);
  });

  document.getElementById('btn-reset-offset').addEventListener('click', async () => {
    if (!confirm('Are you sure you want to reset the log parser offset? This will parse your access.log from the beginning.')) return;
    const btn = document.getElementById('btn-reset-offset');
    btn.textContent = '⏳ Resetting...';
    btn.disabled = true;
    try {
      const res = await API.post('/tools/reset_offset');
      btn.textContent = '✅ Reset!';
      alert(res.message || 'Offset reset successfully. The log parser is rewinding.');
    } catch (e) {
      btn.textContent = '❌ Error';
      console.error(e);
    }
    setTimeout(() => {
      btn.textContent = '⚠️ Reset Log Parser Offset';
      btn.disabled = false;
    }, 3000);
  });
}

async function saveSettings() {
  const btn = document.getElementById('settings-save');
  btn.textContent = '⏳ Saving...';
  try {
    const steam_api_key = document.getElementById('setting-steam-key').value;
    const gravity_key = document.getElementById('setting-gravity-key').value.trim();
    const cache_scan_interval_secs = parseInt(document.getElementById('setting-scan-interval').value, 10) || 0;
    const db_path = document.getElementById('setting-db-path').value.trim();
    const log_retention_days = parseInt(document.getElementById('setting-retention').value, 10) || 90;
    const raw_log_scan_days = parseInt(document.getElementById('setting-log-scan-days').value, 10);
    const log_scan_days = isNaN(raw_log_scan_days) ? 7 : raw_log_scan_days;
    const excluded_ips = document.getElementById('setting-excluded-ips').value
      .split(',')
      .map(ip => ip.trim())
      .filter(Boolean);

    await API.put('/config', {
      steam_api_key,
      cache_scan_interval_secs,
      db_path,
      log_retention_days,
      log_scan_days,
      excluded_ips,
    });

    if (gravity_key) {
      localStorage.setItem('gravity_api_key', gravity_key);
    } else {
      localStorage.removeItem('gravity_api_key');
    }

    btn.textContent = '✅ Saved!';
  } catch (e) {
    btn.textContent = '❌ Error';
    console.error('Failed to save settings:', e);
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

function populateLiveFeed(downloads) {
  const feed = document.getElementById('live-feed');
  if (!feed) return;

  if (!downloads || downloads.length === 0) {
    feed.innerHTML = '<div class="empty-state"><div class="icon">📡</div>Waiting for activity...</div>';
    return;
  }

  feed.innerHTML = downloads.map(d => {
    return `<div style="padding:10px 0;border-bottom:1px solid var(--border-subtle);font-size:0.85rem;display:flex;justify-content:space-between;align-items:center">
      <div style="display:flex;align-items:center;gap:8px">
        ${serviceBadge(d.service)}
        <span style="color:var(--text-secondary);font-family:var(--font-mono)">${escapeHtml(d.client_ip)}</span>
      </div>
      <div style="display:flex;flex-direction:column;align-items:flex-end">
        <span style="font-weight:700;color:var(--text-primary)">${formatBytes(d.total_bytes)}</span>
        <span style="font-size:0.75rem;color:var(--text-muted)">${timeAgo(d.ended_at)}</span>
      </div>
    </div>`;
  }).join('');
}

let dashboardReloadTimeout = null;
function throttledLoadDashboard() {
  if (dashboardReloadTimeout) return;
  if (currentPage === 'dashboard') {
    loadDashboardData();
  }
  dashboardReloadTimeout = setTimeout(() => {
    dashboardReloadTimeout = null;
  }, 2000);
}

function setupLiveFeed() {
  ws.on((data) => {
    // Update dashboard stats on specific events
    if (currentPage === 'dashboard') {
      if (data.type === 'initial_state') {
        loadDashboardData();
      } else if (data.type === 'new_download') {
        throttledLoadDashboard();
      }
    }

    // Add to live feed if on dashboard
    if (data.type === 'new_download' && currentPage === 'dashboard') {
      const feed = document.getElementById('live-feed');
      if (!feed) return;

      // Clear empty state
      if (feed.querySelector('.empty-state')) feed.innerHTML = '';

      const item = document.createElement('div');
      item.className = 'fade-in';
      item.style.cssText = 'padding:10px 0;border-bottom:1px solid var(--border-subtle);font-size:0.85rem;display:flex;justify-content:space-between;align-items:center';
      item.innerHTML = `
        <div style="display:flex;align-items:center;gap:8px">
          ${serviceBadge(data.service)}
          <span style="color:var(--text-secondary);font-family:var(--font-mono)">${escapeHtml(data.client_ip)}</span>
        </div>
        <div style="display:flex;flex-direction:column;align-items:flex-end">
          <span style="font-weight:700;color:var(--text-primary)">${formatBytes(data.bytes)}</span>
          <span style="font-size:0.75rem;color:var(--text-muted)">Just now</span>
        </div>
      `;

      feed.insertBefore(item, feed.firstChild);

      // Keep max 50 items
      while (feed.children.length > 50) feed.removeChild(feed.lastChild);
    }

    if (data.type === 'network_traffic') {
      updateNetTrafficChart(data.interfaces);
    }

    if (data.type === 'cache_update') {
      if (currentPage === 'cache') {
        const sizeEl = document.getElementById('cache-size');
        const filesEl = document.getElementById('cache-files');
        const scanEl = document.getElementById('cache-last-scan');
        if (sizeEl) sizeEl.textContent = formatBytes(data.total_size_bytes);
        if (filesEl) filesEl.textContent = data.total_files.toLocaleString() + ' files';
        if (scanEl) scanEl.textContent = 'Just now';
      }
    }

    if (data.type === 'parser_status') {
      updateParserProgress(data);
    }
  });
}

// ── Network Traffic Chart ──────────────────────────────────────────

let netTrafficHistory = [];

function updateNetTrafficChart(interfaces) {
  const select = document.getElementById('net-interface-select');
  const currentVal = localStorage.getItem('selected_net_interface') || 'all';

  if (select) {
    // Populate dropdown with detected interface names if it has only "All Interfaces"
    if (select.options.length <= 1) {
      select.innerHTML = '<option value="all">All Interfaces</option>';
      for (const name of Object.keys(interfaces)) {
        const opt = document.createElement('option');
        opt.value = name;
        opt.textContent = name;
        if (name === currentVal) opt.selected = true;
        select.appendChild(opt);
      }
      
      select.addEventListener('change', (e) => {
        localStorage.setItem('selected_net_interface', e.target.value);
        netTrafficHistory = []; // Reset history so graph scaling adjusts smoothly
      });
    }
  }

  let totalRx = 0;
  let totalTx = 0;
  let activeInts = [];

  const selectedInterface = localStorage.getItem('selected_net_interface') || 'all';

  if (selectedInterface === 'all') {
    for (const [name, data] of Object.entries(interfaces)) {
      totalRx += data.rx_bytes_sec;
      totalTx += data.tx_bytes_sec;
      activeInts.push(`${name}: rx ${formatBytes(data.rx_bytes_sec)}/s, tx ${formatBytes(data.tx_bytes_sec)}/s`);
    }
  } else {
    const data = interfaces[selectedInterface];
    if (data) {
      totalRx = data.rx_bytes_sec;
      totalTx = data.tx_bytes_sec;
      activeInts.push(`${selectedInterface}: rx ${formatBytes(totalRx)}/s, tx ${formatBytes(totalTx)}/s`);
    } else {
      activeInts.push(`${selectedInterface} (offline)`);
    }
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
    ctx.lineWidth = 3;
    ctx.stroke();

    ctx.lineTo(getX(netTrafficHistory.length - 1), h);
    ctx.lineTo(getX(0), h);
    ctx.closePath();
    ctx.fillStyle = fillColor;
    ctx.fill();
  };

  drawLine('rx', 'rgba(99, 102, 241, 0.85)', 'rgba(99, 102, 241, 0.08)');
  drawLine('tx', 'rgba(168, 85, 247, 0.85)', 'rgba(168, 85, 247, 0.04)');

  ctx.font = '12px monospace';
  ctx.textAlign = 'right';

  const currentRx = netTrafficHistory[netTrafficHistory.length - 1].rx;
  const currentTx = netTrafficHistory[netTrafficHistory.length - 1].tx;

  ctx.fillStyle = '#818cf8';
  ctx.fillText(`📥 Down: ${formatBytes(currentRx)}/s`, w - 12, 22);
  ctx.fillStyle = '#c084fc';
  ctx.fillText(`📤 Up: ${formatBytes(currentTx)}/s`, w - 12, 38);
  ctx.fillStyle = 'rgba(255, 255, 255, 0.4)';
  ctx.fillText(`Max: ${formatBytes(maxVal)}/s`, w - 12, 54);
}

let logsInterval = null;

async function renderLogs(container) {
  if (logsInterval) clearInterval(logsInterval);

  let isLogsPaused = false;

  container.innerHTML = `
    <div class="card" style="display:flex;flex-direction:column;height:calc(100vh - 160px)">
      <div class="card-header" style="justify-content:space-between">
        <div class="card-title">📜 Backend System Logs</div>
        <div style="display:flex;gap:8px">
          <button class="btn btn-ghost" id="btn-pause-logs">⏸️ Pause</button>
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
    if (isLogsPaused) return;

    try {
      const logs = await API.get('/logs');
      const consoleBox = document.getElementById('logs-console');
      if (!consoleBox) return;

      if (!logs || logs.length === 0) {
        consoleBox.innerHTML = '<div style="color:var(--text-muted)">No logs recorded yet.</div>';
        return;
      }

      // Check if user is scrolled near bottom (within 50px)
      const isNearBottom = consoleBox.scrollHeight - consoleBox.scrollTop - consoleBox.clientHeight < 50;

      consoleBox.innerHTML = logs.map(line => {
        let color = '#e2e8f0';
        if (line.includes('ERROR') || line.includes('error')) color = '#f87171';
        else if (line.includes('WARN') || line.includes('warn')) color = '#fbbf24';
        else if (line.includes('INFO') || line.includes('info')) color = '#60a5fa';
        else if (line.includes('DEBUG') || line.includes('debug')) color = '#c084fc';

        return `<div style="color:${color};margin-bottom:2px">${escapeHtml(line)}</div>`;
      }).join('');

      if (isNearBottom) {
        consoleBox.scrollTop = consoleBox.scrollHeight;
      }
    } catch (e) {
      const consoleBox = document.getElementById('logs-console');
      if (consoleBox) consoleBox.innerHTML = `<div style="color:var(--color-miss)">Failed to load logs: ${e.message}</div>`;
    }
  }

  fetchLogs();
  logsInterval = setInterval(fetchLogs, 3000);

  document.getElementById('btn-refresh-logs').addEventListener('click', () => {
    const wasPaused = isLogsPaused;
    isLogsPaused = false;
    fetchLogs().then(() => {
      isLogsPaused = wasPaused;
    });
  });

  document.getElementById('btn-pause-logs').addEventListener('click', (e) => {
    isLogsPaused = !isLogsPaused;
    const btn = e.target;
    if (isLogsPaused) {
      btn.textContent = '▶️ Resume';
      btn.style.color = 'var(--color-miss)';
    } else {
      btn.textContent = '⏸️ Pause';
      btn.style.color = '';
      fetchLogs(); // Fetch immediately on resume
    }
  });

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
  // Mobile menu toggle
  const menuToggle = document.getElementById('mobile-menu-toggle');
  const sidebar = document.getElementById('sidebar');
  const overlay = document.getElementById('sidebar-overlay');

  if (menuToggle && sidebar && overlay) {
    const toggleMenu = () => {
      sidebar.classList.toggle('open');
      overlay.classList.toggle('active');
    };
    menuToggle.addEventListener('click', toggleMenu);
    overlay.addEventListener('click', toggleMenu);
    
    // Auto-close sidebar on menu link click (mobile)
    document.querySelectorAll('.nav-item').forEach(el => {
      el.addEventListener('click', () => {
        if (sidebar.classList.contains('open')) {
          toggleMenu();
        }
      });
    });
  }

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

  // Fetch and display dynamic application version from API
  API.get('/health').then(res => {
    if (res.version) {
      const versionSidebar = document.getElementById('app-version-sidebar');
      const versionLogo = document.getElementById('app-version-logo');
      if (versionSidebar) versionSidebar.textContent = 'v' + res.version;
      if (versionLogo) versionLogo.textContent = 'v' + res.version.split('.').slice(0, 2).join('.');
    }
  }).catch(() => {});

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
