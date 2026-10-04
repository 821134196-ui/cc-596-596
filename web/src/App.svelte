<script>
  import { onMount } from 'svelte'
  import { user, clearSession } from './store.js'
  import { api } from './api.js'
  import Login from './pages/Login.svelte'
  import StaffList from './pages/staff/StaffList.svelte'
  import StaffDetail from './pages/staff/StaffDetail.svelte'
  import Delivery from './pages/staff/Delivery.svelte'
  import MyList from './pages/customer/MyList.svelte'
  import MyDetail from './pages/customer/MyDetail.svelte'

  let hash = location.hash || '#/'
  let route = { name: 'home' }

  $: route = parseHash(hash)
  $: currentUser = $user

  function parseHash(h) {
    const path = h.replace(/^#/, '')
    let m
    if ((m = path.match(/^\/staff\/commissions\/(\d+)$/))) return { name: 'staff-detail', id: +m[1] }
    if (path === '/staff/delivery') return { name: 'staff-delivery' }
    if (path === '/staff') return { name: 'staff' }
    if ((m = path.match(/^\/my\/(\d+)$/))) return { name: 'my-detail', id: +m[1] }
    if (path === '/my') return { name: 'my' }
    return { name: 'home' }
  }

  function homeFor(u) {
    return u?.role === 'staff' ? '#/staff' : '#/my'
  }

  $: if (currentUser && route.name === 'home') location.hash = homeFor(currentUser)

  async function logout() {
    try { await api('/logout', { method: 'POST' }) } catch {}
    clearSession()
    location.hash = '#/'
  }

  onMount(() => {
    const h = () => (hash = location.hash || '#/')
    window.addEventListener('hashchange', h)
    return () => window.removeEventListener('hashchange', h)
  })
</script>

{#if !currentUser}
  <Login />
{:else}
  <header class="topbar">
    <span class="brand">🕊️ 宠物殡葬 · 遗物封存与骨灰交付</span>
    <nav>
      {#if currentUser.role === 'staff'}
        <a href="#/staff" class:active={route.name === 'staff'}>委托管理</a>
        <a href="#/staff/delivery" class:active={route.name === 'staff-delivery'}>领取交付</a>
      {:else}
        <a href="#/my" class:active={route.name === 'my'}>我的委托</a>
      {/if}
    </nav>
    <span class="who">{currentUser.name}（{currentUser.role === 'staff' ? '员工' : '客户'}）</span>
    <button class="ghost" on:click={logout}>退出</button>
  </header>
  <main>
    {#if route.name === 'staff'}
      <StaffList />
    {:else if route.name === 'staff-detail'}
      <StaffDetail id={route.id} />
    {:else if route.name === 'staff-delivery'}
      <Delivery />
    {:else if route.name === 'my'}
      <MyList />
    {:else if route.name === 'my-detail'}
      <MyDetail id={route.id} />
    {/if}
  </main>
{/if}
