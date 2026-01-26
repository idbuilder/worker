// IDBuilder Admin Console - API Utilities

const TOKEN_KEY = 'idbuilder_admin_token';

/**
 * Get the stored admin token.
 * @returns {string|null}
 */
export function getToken() {
  return localStorage.getItem(TOKEN_KEY);
}

/**
 * Store the admin token.
 * @param {string} token
 */
export function setToken(token) {
  localStorage.setItem(TOKEN_KEY, token);
}

/**
 * Clear the stored token.
 */
export function clearToken() {
  localStorage.removeItem(TOKEN_KEY);
}

/**
 * Check if user is authenticated.
 * @returns {boolean}
 */
export function isAuthenticated() {
  return !!getToken();
}

/**
 * Make an authenticated GET request.
 * @param {string} path - API path
 * @param {Record<string, string>} [params] - Query parameters
 * @returns {Promise<{ok: boolean, status: number, data: any}>}
 */
export async function apiGet(path, params = {}) {
  const token = getToken();
  if (!token) {
    return { ok: false, status: 401, data: { code: 2001, message: 'Not authenticated' } };
  }

  const url = new URL(path, window.location.origin);
  Object.entries(params).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== '') {
      url.searchParams.set(key, value);
    }
  });

  try {
    const response = await fetch(url, {
      method: 'GET',
      headers: {
        'Authorization': `Bearer ${token}`,
      },
    });

    const data = await response.json();
    return { ok: response.ok, status: response.status, data };
  } catch (error) {
    return { ok: false, status: 0, data: { code: 5001, message: error.message } };
  }
}

/**
 * Make an authenticated POST request.
 * @param {string} path - API path
 * @param {any} body - Request body
 * @returns {Promise<{ok: boolean, status: number, data: any}>}
 */
export async function apiPost(path, body) {
  const token = getToken();
  if (!token) {
    return { ok: false, status: 401, data: { code: 2001, message: 'Not authenticated' } };
  }

  try {
    const response = await fetch(path, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${token}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(body),
    });

    const data = await response.json();
    return { ok: response.ok, status: response.status, data };
  } catch (error) {
    return { ok: false, status: 0, data: { code: 5001, message: error.message } };
  }
}

/**
 * Verify token with the server.
 * @param {string} token
 * @returns {Promise<boolean>}
 */
export async function verifyToken(token) {
  try {
    const response = await fetch('/api/auth/verify', {
      method: 'GET',
      headers: {
        'Authorization': `Bearer ${token}`,
      },
    });
    return response.ok;
  } catch {
    return false;
  }
}

/**
 * Handle API error - redirect to login if unauthorized.
 * @param {number} status
 */
export function handleApiError(status) {
  if (status === 401) {
    clearToken();
    window.location.href = './';
  }
}

/**
 * Copy text to clipboard.
 * @param {string} text
 * @returns {Promise<boolean>}
 */
export async function copyToClipboard(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Fallback for older browsers
    const textarea = document.createElement('textarea');
    textarea.value = text;
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    try {
      document.execCommand('copy');
      return true;
    } catch {
      return false;
    } finally {
      document.body.removeChild(textarea);
    }
  }
}

/**
 * Show a temporary message.
 * @param {HTMLElement} container
 * @param {string} message
 * @param {'error'|'success'|'warning'} type
 */
export function showMessage(container, message, type = 'error') {
  const existing = container.querySelector('.alert');
  if (existing) {
    existing.remove();
  }

  const alert = document.createElement('div');
  alert.className = `alert alert-${type}`;
  alert.textContent = message;
  container.prepend(alert);

  if (type === 'success') {
    setTimeout(() => alert.remove(), 3000);
  }
}

/**
 * Clear messages from container.
 * @param {HTMLElement} container
 */
export function clearMessage(container) {
  const alert = container.querySelector('.alert');
  if (alert) {
    alert.remove();
  }
}
