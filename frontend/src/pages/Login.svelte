<script>
  import { api, setToken } from '../api.js';
  import { user } from '../stores.js';

  let username = '';
  let password = '';
  let error = '';
  let loading = false;

  async function submit() {
    error = '';
    loading = true;
    try {
      const res = await api('/auth/login', { method: 'POST', body: { username, password } });
      setToken(res.token);
      $user = res.user;
      location.hash = '#/';
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }
</script>

<div class="container" style="max-width: 420px; margin-top: 8vh;">
  <div class="card">
    <h2>🕊️ 宠安堂</h2>
    <p class="muted">宠物殡葬 · 遗物封存与骨灰交付系统</p>
    {#if error}<div class="alert error">{error}</div>{/if}
    <form onsubmit={(e) => { e.preventDefault(); submit(); }}>
      <label class="field">账号
        <input bind:value={username} required autocomplete="username" />
      </label>
      <label class="field">密码
        <input type="password" bind:value={password} required autocomplete="current-password" />
      </label>
      <button style="width: 100%;" disabled={loading}>{loading ? '登录中…' : '登 录'}</button>
    </form>
    <p class="muted" style="margin-top: 16px; line-height: 1.8;">
      演示账号 —— 员工：zhang / staff123、liu / staff123<br />
      客户：chen / cust123、wang / cust123
    </p>
  </div>
</div>
