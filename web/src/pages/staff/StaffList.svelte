<script>
  import { onMount } from 'svelte'
  import { api } from '../../api.js'

  let commissions = []
  let error = ''
  let showCreate = false
  let msg = ''

  let form = {
    customer_username: '', customer_name: '', customer_phone: '',
    pet_name: '', pet_species: '',
    pickup_address: '', pickup_time: '', pickup_contact: '', pickup_notes: '',
  }

  async function load() {
    try {
      const data = await api('/staff/commissions')
      commissions = data.commissions
    } catch (e) { error = e.message }
  }

  async function create() {
    error = ''; msg = ''
    try {
      const data = await api('/staff/commissions', { method: 'POST', body: form })
      msg = `委托已创建：${data.code}`
      showCreate = false
      form = { customer_username: '', customer_name: '', customer_phone: '', pet_name: '', pet_species: '', pickup_address: '', pickup_time: '', pickup_contact: '', pickup_notes: '' }
      await load()
    } catch (e) { error = e.message }
  }

  onMount(load)
</script>

<div class="card">
  <div class="row">
    <h2 style="margin:0">委托单列表</h2>
    <span class="spacer"></span>
    <button on:click={() => (showCreate = !showCreate)}>{showCreate ? '收起' : '＋ 新建委托'}</button>
  </div>
  {#if msg}<div class="alert success">{msg}</div>{/if}
  {#if error}<div class="alert error">{error}</div>{/if}

  {#if showCreate}
    <form on:submit|preventDefault={create} style="margin-top:14px; border-top:1px dashed var(--line); padding-top:14px;">
      <h3>客户信息（新客户将自动创建账号，初始密码 123456）</h3>
      <div class="form-row">
        <label class="field">客户登录名 *<input bind:value={form.customer_username} required /></label>
        <label class="field">客户姓名<input bind:value={form.customer_name} /></label>
        <label class="field">联系电话<input bind:value={form.customer_phone} /></label>
      </div>
      <h3>宠物信息</h3>
      <div class="form-row">
        <label class="field">宠物名字 *<input bind:value={form.pet_name} required /></label>
        <label class="field">种类<input bind:value={form.pet_species} placeholder="猫 / 犬 / …" /></label>
      </div>
      <h3>接运信息</h3>
      <div class="form-row">
        <label class="field">接运地址<input bind:value={form.pickup_address} /></label>
        <label class="field">接运时间<input bind:value={form.pickup_time} placeholder="2026-10-05 14:00" /></label>
        <label class="field">接运联系人<input bind:value={form.pickup_contact} /></label>
        <label class="field">备注<input bind:value={form.pickup_notes} /></label>
      </div>
      <button type="submit">创建委托单</button>
    </form>
  {/if}
</div>

<div class="card">
  <table>
    <thead>
      <tr><th>委托号</th><th>宠物</th><th>客户</th><th>服务状态</th><th>交付状态</th><th>创建时间</th><th></th></tr>
    </thead>
    <tbody>
      {#each commissions as c}
        <tr>
          <td class="mono">{c.code}</td>
          <td>{c.pet_name}{c.pet_species ? `（${c.pet_species}）` : ''}</td>
          <td>{c.customer_name}</td>
          <td>
            {#if c.service_status === 'completed'}<span class="badge ok">服务已完成</span>
            {:else}<span class="badge warn">服务进行中</span>{/if}
          </td>
          <td>
            {#if c.delivery_status === 'delivered'}<span class="badge done">物品已交付</span>
            {:else}<span class="badge">待交付</span>{/if}
          </td>
          <td class="muted">{c.created_at}</td>
          <td><a href="#/staff/commissions/{c.id}">详情 →</a></td>
        </tr>
      {:else}
        <tr><td colspan="7" class="muted">暂无委托</td></tr>
      {/each}
    </tbody>
  </table>
</div>
