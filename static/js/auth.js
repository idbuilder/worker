// IDBuilder Admin Console - Authentication Page

import { getToken, setToken, verifyToken, showMessage, clearMessage } from './api.js';

document.addEventListener('DOMContentLoaded', init);

async function init() {
  // Check if already authenticated
  const token = getToken();
  if (token) {
    const valid = await verifyToken(token);
    if (valid) {
      window.location.href = 'manage.html';
      return;
    }
  }

  // Setup form handler
  const form = document.getElementById('auth-form');
  form.addEventListener('submit', handleLogin);
}

async function handleLogin(e) {
  e.preventDefault();

  const form = e.target;
  const tokenInput = document.getElementById('token-input');
  const submitBtn = document.getElementById('submit-btn');
  const token = tokenInput.value.trim();

  if (!token) {
    showMessage(form, 'Please enter the admin token');
    return;
  }

  // Disable form during verification
  tokenInput.disabled = true;
  submitBtn.disabled = true;
  submitBtn.innerHTML = '<span class="loading"></span> Verifying...';
  clearMessage(form);

  try {
    const valid = await verifyToken(token);

    if (valid) {
      setToken(token);
      window.location.href = 'manage.html';
    } else {
      showMessage(form, 'Invalid admin token');
      tokenInput.disabled = false;
      submitBtn.disabled = false;
      submitBtn.textContent = 'Login';
      tokenInput.focus();
      tokenInput.select();
    }
  } catch (error) {
    showMessage(form, 'Failed to connect to server');
    tokenInput.disabled = false;
    submitBtn.disabled = false;
    submitBtn.textContent = 'Login';
  }
}
