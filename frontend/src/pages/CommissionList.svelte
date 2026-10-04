<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import { user } from '../stores.js';
  import { SERVICE_STATUS, DELIVERY_STATUS } from '../util.js';

  let rows = [];
  let error = '';

  onMount(async () => {
    try {
      rows = await api('/commissions');
    } catch (e) {
      error = e.message;
    }
  });
</script>

<div class="card">
  <h2>
    委托单列表
    <span class="sub">{$user.role === 'staff' ? '全部门店委托' : '我的委托'}</span>
    <span style="flex:1"></span>
    {#if $user.role === 'staff'}
      <a href="#/commissions/new"><button class="small">＋ 新建委托</button></a>
    {/if}
  </h2>
  {#if error}<div class="alert error">{error}</div>{/if}
  {#if rows.length === 0 && !error}
    <p class="muted">暂无委托单。</p>
  {:else}
    <table>
      <thead>
        <tr>
          <th>委托号</th><th>宠物</th><th>客户</th><th>服务状态</th><th>交付状态</th><th>建立时间</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as r}
          <tr class="clickable" onclick={() => (location.hash = '#/commissions/' + r.commission.id)}>
            <td><strong>{r.commission.commission_no}</strong></td>
            <td>{r.commission.pet_name} <span class="muted">{r.commission.pet_species}</span></td>
            <td>{r.customer_name}</td>
            <td>
              <span class="badge {r.commission.service_status === 'completed' ? 'svc-completed' : 'svc-progress'}">
                {SERVICE_STATUS[r.commission.service_status]}
              </span>
            </td>
            <td>
              <span class="badge {r.commission.delivery_status === 'delivered' ? 'dlv-done' : 'dlv-none'}">
                {DELIVERY_STATUS[r.commission.delivery_status]}
              </span>
            </td>
            <td class="muted">{r.commission.created_at}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>
