import { writable } from 'svelte/store'

const saved = localStorage.getItem('user')
export const user = writable(saved ? JSON.parse(saved) : null)

export function setSession(token, u) {
  localStorage.setItem('token', token)
  localStorage.setItem('user', JSON.stringify(u))
  user.set(u)
}

export function clearSession() {
  localStorage.removeItem('token')
  localStorage.removeItem('user')
  user.set(null)
}

export function getToken() {
  return localStorage.getItem('token') || ''
}
