<script>
  import { onMount } from 'svelte';
  import { user } from './stores.js';
  import { api, setToken, getToken } from './api.js';
  import Login from './pages/Login.svelte';
  import CommissionList from './pages/CommissionList.svelte';
  import CommissionNew from './pages/CommissionNew.svelte';
  import CommissionDetail from './pages/CommissionDetail.svelte';
  import Deliver from './pages/Deliver.svelte';

  let route = { name: 'list' };
  let ready = false;

  function parse() {
    const parts = (location.hash.slice(1) || '/').split('/').filter(Boolean);
    if (parts[0] === 'commissions' && parts[1] === 'new') route = { name: 'new' };
    else if (parts[0] === 'commissions' && parts[1] && parts[2] === 'deliver')
      route = { name: 'deliver', id: Number(parts[1]) };
    else if (parts[0] === 'commissions' && parts[1]) route = { name: 'detail', id: Number(parts[1]) };
    else route = { name: 'list' };
  }

  onMount(async () => {
    parse();
    window.addEventListener('hashchange', parse);
    if (getToken()) {
      try {
        $user = await api('/auth/me');
      } catch {
        setToken('');
      }
    }
    ready = true;
  });

  async function logout() {
    try { await api('/auth/logout', { method: 'POST' }); } catch { /* 忽略 */ }
    setToken('');
    $user = null;
    location.hash = '#/';
  }
</script>

{#if ready}
  {#if !$user}
    <Login />
  {:else}
    <div class="topbar">
      <span class="logo">🕊️ 宠安堂 · 遗物封存与骨灰交付系统</span>
      <a href="#/">委托列表</a>
      {#if $user.role === 'staff'}
        <a href="#/commissions/new">新建委托</a>
      {/if}
      <span class="spacer"></span>
      <span class="who">{$user.name}（{$user.role === 'staff' ? '门店员工' : '客户'}）</span>
      <button class="ghost small" onclick={logout}>退出登录</button>
    </div>
    <div class="container">
      {#if route.name === 'list'}
        <CommissionList />
      {:else if route.name === 'new'}
        <CommissionNew />
      {:else if route.name === 'detail'}
        <CommissionDetail id={route.id} />
      {:else if route.name === 'deliver'}
        <Deliver id={route.id} />
      {/if}
    </div>
  {/if}
{/if}
