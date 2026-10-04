<script>
  import { api } from '../api.js'
  import { setSession } from '../store.js'

  let username = ''
  let password = ''
  let error = ''
  let loading = false

  async function submit() {
    error = ''
    loading = true
    try {
      const data = await api('/login', { method: 'POST', body: { username, password } })
      setSession(data.token, data.user)
      location.hash = data.user.role === 'staff' ? '#/staff' : '#/my'
    } catch (e) {
      error = e.message
    } finally {
      loading = false
    }
  }
</script>

<main style="max-width: 420px; margin-top: 8vh;">
  <div class="card">
    <h2 style="text-align:center; font-size:20px;">🕊️ 宠物殡葬服务系统</h2>
    <p class="muted" style="text-align:center;">遗物封存 · 骨灰交付 · 全程可溯</p>
    {#if error}<div class="alert error">{error}</div>{/if}
    <form on:submit|preventDefault={submit}>
      <label class="field">用户名
        <input bind:value={username} autocomplete="username" required />
      </label>
      <label class="field">密码
        <input type="password" bind:value={password} autocomplete="current-password" required />
      </label>
      <button style="width:100%" disabled={loading}>{loading ? '登录中…' : '登录'}</button>
    </form>
    <p class="muted" style="margin-top:14px;">
      演示账号 — 员工：staff / staff123；客户：chen / 123456、liu / 123456
    </p>
  </div>
</main>
