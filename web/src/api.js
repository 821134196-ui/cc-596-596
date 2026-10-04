import { getToken } from './store.js'

export class ApiError extends Error {
  constructor(message, code, status) {
    super(message)
    this.code = code
    this.status = status
  }
}

export async function api(path, { method = 'GET', body, formData } = {}) {
  const headers = {}
  const token = getToken()
  if (token) headers['Authorization'] = `Bearer ${token}`
  if (body !== undefined) headers['Content-Type'] = 'application/json'
  const res = await fetch('/api' + path, {
    method,
    headers,
    body: formData ? formData : body !== undefined ? JSON.stringify(body) : undefined,
  })
  const data = await res.json().catch(() => ({}))
  if (!res.ok) {
    const err = data?.error || {}
    throw new ApiError(err.message || `请求失败 (${res.status})`, err.code || 'UNKNOWN', res.status)
  }
  return data
}

export function fmtPrice(cents) {
  return '¥' + (cents / 100).toFixed(2)
}
