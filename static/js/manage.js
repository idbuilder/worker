// IDBuilder Admin Console - Management Page

import {
  isAuthenticated,
  clearToken,
  apiGet,
  apiPost,
  handleApiError,
  showMessage,
  clearMessage,
  copyToClipboard,
} from './api.js';

// State
let currentTab = 'configs';
let configList = [];
let currentPage = 0;
let pageSize = 20;
let hasMore = false;
let selectedConfig = null;

document.addEventListener('DOMContentLoaded', init);

function init() {
  // Auth guard
  if (!isAuthenticated()) {
    window.location.href = './';
    return;
  }

  // Setup event handlers
  document.getElementById('logout-btn').addEventListener('click', handleLogout);

  // Tabs
  document.querySelectorAll('.tab').forEach(tab => {
    tab.addEventListener('click', () => switchTab(tab.dataset.tab));
  });

  // Config list
  document.getElementById('config-filter').addEventListener('change', () => {
    currentPage = 0;
    loadConfigs();
  });
  document.getElementById('prev-page').addEventListener('click', () => changePage(-1));
  document.getElementById('next-page').addEventListener('click', () => changePage(1));

  // Create config
  document.getElementById('create-type').addEventListener('change', handleTypeChange);
  document.getElementById('create-form').addEventListener('submit', handleCreateConfig);

  // Token management
  document.getElementById('get-token-btn').addEventListener('click', handleGetToken);
  document.getElementById('reset-token-btn').addEventListener('click', handleResetToken);
  document.getElementById('copy-token-btn').addEventListener('click', handleCopyToken);

  // Edit config
  document.getElementById('save-config-btn').addEventListener('click', handleSaveConfig);
  document.getElementById('cancel-edit-btn').addEventListener('click', cancelEdit);

  // Load initial data
  loadConfigs();
}

function handleLogout() {
  clearToken();
  window.location.href = './';
}

function switchTab(tab) {
  currentTab = tab;
  document.querySelectorAll('.tab').forEach(t => {
    t.classList.toggle('active', t.dataset.tab === tab);
  });
  document.querySelectorAll('.tab-content').forEach(c => {
    c.classList.toggle('hidden', c.id !== `${tab}-tab`);
  });
}

// Config List
async function loadConfigs() {
  const filter = document.getElementById('config-filter').value;
  const tbody = document.getElementById('config-tbody');
  const emptyState = document.getElementById('config-empty');

  tbody.innerHTML = '<tr><td colspan="4" class="text-center"><span class="loading"></span></td></tr>';

  const result = await apiGet('/api/config/list', {
    key: filter || undefined,
    from: currentPage > 0 ? configList[configList.length - 1]?.key : undefined,
    size: pageSize + 1,
  });

  if (!result.ok) {
    handleApiError(result.status);
    tbody.innerHTML = '<tr><td colspan="4" class="text-center text-muted">Failed to load configs</td></tr>';
    return;
  }

  const items = result.data.data?.items || [];
  hasMore = items.length > pageSize;
  configList = items.slice(0, pageSize);

  if (configList.length === 0) {
    tbody.innerHTML = '';
    emptyState.classList.remove('hidden');
  } else {
    emptyState.classList.add('hidden');
    tbody.innerHTML = configList.map(config => `
      <tr>
        <td>${escapeHtml(config.key)}</td>
        <td><span class="badge badge-${config.id_type}">${config.id_type}</span></td>
        <td>${formatDate(config.created_at)}</td>
        <td>
          <button class="btn btn-sm btn-secondary" onclick="window.editConfig('${escapeHtml(config.key)}', '${config.id_type}')">
            Edit
          </button>
        </td>
      </tr>
    `).join('');
  }

  updatePagination();
}

function updatePagination() {
  document.getElementById('prev-page').disabled = currentPage === 0;
  document.getElementById('next-page').disabled = !hasMore;
  document.getElementById('page-info').textContent = `Page ${currentPage + 1}`;
}

function changePage(delta) {
  currentPage += delta;
  loadConfigs();
}

// Edit Config
window.editConfig = async function(key, type) {
  const section = document.getElementById('edit-section');
  const form = document.getElementById('edit-form');

  section.classList.remove('hidden');
  document.getElementById('edit-key').value = key;
  document.getElementById('edit-type').textContent = type;

  // Hide all type-specific fields
  document.querySelectorAll('.edit-fields').forEach(f => f.classList.add('hidden'));

  // Fetch config details
  const result = await apiGet(`/api/config/${type}`, { key });

  if (!result.ok) {
    handleApiError(result.status);
    showMessage(form, 'Failed to load config');
    return;
  }

  const config = result.data.data;
  selectedConfig = { key, type, config };

  // Show and populate type-specific fields
  const fields = document.getElementById(`edit-${type}-fields`);
  fields.classList.remove('hidden');

  if (type === 'increment') {
    document.getElementById('edit-inc-start').value = config.start || 1;
    document.getElementById('edit-inc-step').value = config.step || 1;
    document.getElementById('edit-inc-min').value = config.min || 0;
    document.getElementById('edit-inc-max').value = config.max || '';
  } else if (type === 'snowflake') {
    document.getElementById('edit-sf-epoch').value = config.epoch_ts || '';
    document.getElementById('edit-sf-worker-bits').value = config.worker_id_bits || 10;
    document.getElementById('edit-sf-seq-bits').value = config.sequence_bits || 12;
  } else if (type === 'formatted') {
    document.getElementById('edit-fmt-pattern').value = config.pattern || '';
  }

  section.scrollIntoView({ behavior: 'smooth' });
};

function cancelEdit() {
  document.getElementById('edit-section').classList.add('hidden');
  selectedConfig = null;
}

async function handleSaveConfig(e) {
  e.preventDefault();

  if (!selectedConfig) return;

  const { key, type } = selectedConfig;
  const btn = document.getElementById('save-config-btn');
  const form = document.getElementById('edit-form');

  btn.disabled = true;
  btn.innerHTML = '<span class="loading"></span> Saving...';
  clearMessage(form);

  let body = { key };

  if (type === 'increment') {
    body.start = parseInt(document.getElementById('edit-inc-start').value) || 1;
    body.step = parseInt(document.getElementById('edit-inc-step').value) || 1;
    body.min = parseInt(document.getElementById('edit-inc-min').value) || 0;
    const max = document.getElementById('edit-inc-max').value;
    if (max) body.max = parseInt(max);
  } else if (type === 'snowflake') {
    body.epoch_ts = parseInt(document.getElementById('edit-sf-epoch').value) || undefined;
    body.worker_id_bits = parseInt(document.getElementById('edit-sf-worker-bits').value) || 10;
    body.sequence_bits = parseInt(document.getElementById('edit-sf-seq-bits').value) || 12;
  } else if (type === 'formatted') {
    body.pattern = document.getElementById('edit-fmt-pattern').value;
  }

  const result = await apiPost(`/api/config/${type}`, body);

  btn.disabled = false;
  btn.textContent = 'Save';

  if (result.ok) {
    showMessage(form, 'Config saved successfully', 'success');
    loadConfigs();
  } else {
    handleApiError(result.status);
    showMessage(form, result.data.message || 'Failed to save config');
  }
}

// Create Config
function handleTypeChange() {
  const type = document.getElementById('create-type').value;
  document.querySelectorAll('.create-fields').forEach(f => f.classList.add('hidden'));
  if (type) {
    document.getElementById(`create-${type}-fields`).classList.remove('hidden');
  }
}

async function handleCreateConfig(e) {
  e.preventDefault();

  const form = e.target;
  const type = document.getElementById('create-type').value;
  const key = document.getElementById('create-key').value.trim();

  if (!type || !key) {
    showMessage(form, 'Please select a type and enter a key');
    return;
  }

  const btn = form.querySelector('button[type="submit"]');
  btn.disabled = true;
  btn.innerHTML = '<span class="loading"></span> Creating...';
  clearMessage(form);

  let body = { key };

  if (type === 'increment') {
    body.start = parseInt(document.getElementById('create-inc-start').value) || 1;
    body.step = parseInt(document.getElementById('create-inc-step').value) || 1;
    body.min = parseInt(document.getElementById('create-inc-min').value) || 0;
    const max = document.getElementById('create-inc-max').value;
    if (max) body.max = parseInt(max);
  } else if (type === 'snowflake') {
    body.epoch_ts = parseInt(document.getElementById('create-sf-epoch').value) || undefined;
    body.worker_id_bits = parseInt(document.getElementById('create-sf-worker-bits').value) || 10;
    body.sequence_bits = parseInt(document.getElementById('create-sf-seq-bits').value) || 12;
  } else if (type === 'formatted') {
    body.pattern = document.getElementById('create-fmt-pattern').value;
  }

  const result = await apiPost(`/api/config/${type}`, body);

  btn.disabled = false;
  btn.textContent = 'Create Config';

  if (result.ok) {
    showMessage(form, 'Config created successfully', 'success');
    form.reset();
    handleTypeChange();
    loadConfigs();
  } else {
    handleApiError(result.status);
    showMessage(form, result.data.message || 'Failed to create config');
  }
}

// Token Management
async function handleGetToken() {
  const keyInput = document.getElementById('token-key');
  const key = keyInput.value.trim();
  const section = document.getElementById('tokens-tab');

  if (!key) {
    showMessage(section, 'Please enter a key name');
    return;
  }

  const btn = document.getElementById('get-token-btn');
  btn.disabled = true;
  btn.innerHTML = '<span class="loading"></span>';
  clearMessage(section);

  const result = await apiGet('/api/auth/token', { key });

  btn.disabled = false;
  btn.textContent = 'Get Token';

  if (result.ok) {
    displayToken(result.data.data?.token || result.data.data);
  } else {
    handleApiError(result.status);
    showMessage(section, result.data.message || 'Failed to get token');
  }
}

async function handleResetToken() {
  const keyInput = document.getElementById('token-key');
  const key = keyInput.value.trim();
  const section = document.getElementById('tokens-tab');

  if (!key) {
    showMessage(section, 'Please enter a key name');
    return;
  }

  if (!confirm(`Are you sure you want to reset the token for "${key}"? The old token will stop working.`)) {
    return;
  }

  const btn = document.getElementById('reset-token-btn');
  btn.disabled = true;
  btn.innerHTML = '<span class="loading"></span>';
  clearMessage(section);

  const result = await apiGet('/api/auth/tokenreset', { key });

  btn.disabled = false;
  btn.textContent = 'Reset Token';

  if (result.ok) {
    displayToken(result.data.data?.token || result.data.data);
    showMessage(section, 'Token has been reset', 'success');
  } else {
    handleApiError(result.status);
    showMessage(section, result.data.message || 'Failed to reset token');
  }
}

function displayToken(token) {
  const display = document.getElementById('token-display');
  const value = document.getElementById('token-value');
  display.classList.remove('hidden');
  value.textContent = token;
}

async function handleCopyToken() {
  const token = document.getElementById('token-value').textContent;
  const success = await copyToClipboard(token);
  const btn = document.getElementById('copy-token-btn');
  const original = btn.textContent;
  btn.textContent = success ? 'Copied!' : 'Failed';
  setTimeout(() => btn.textContent = original, 2000);
}

// Utilities
function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function formatDate(dateStr) {
  if (!dateStr) return '-';
  try {
    return new Date(dateStr).toLocaleDateString();
  } catch {
    return dateStr;
  }
}
