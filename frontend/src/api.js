const TOKEN_KEY = 'pm_token';

export function getToken() {
  return localStorage.getItem(TOKEN_KEY) || '';
}
export function setToken(t) {
  if (t) localStorage.setItem(TOKEN_KEY, t);
  else localStorage.removeItem(TOKEN_KEY);
}

export class ApiError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

async function parse(res) {
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new ApiError(res.status, data.error || `请求失败 (${res.status})`);
  return data;
}

export async function api(path, { method = 'GET', body } = {}) {
  const headers = {};
  if (getToken()) headers['Authorization'] = 'Bearer ' + getToken();
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  const res = await fetch('/api' + path, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  return parse(res);
}

export async function uploadPhoto(itemId, file) {
  const fd = new FormData();
  fd.append('file', file);
  const res = await fetch(`/api/items/${itemId}/photo`, {
    method: 'POST',
    headers: { Authorization: 'Bearer ' + getToken() },
    body: fd,
  });
  return parse(res);
}
