<script>
  import { onMount } from 'svelte'
  import { api } from '../../api.js'

  let commissions = []
  let error = ''

  onMount(async () => {
    try {
      const data = await api('/my/commissions')
      commissions = data.commissions
    } catch (e) { error = e.message }
  })
</script>

{#if error}<div class="alert error">{error}</div>{/if}

<div class="card">
  <h2>我的委托</h2>
  <table>
    <thead><tr><th>委托号</th><th>宠物</th><th>服务进度</th><th>物品交付</th><th>受理时间</th><th></th></tr></thead>
    <tbody>
      {#each commissions as c}
        <tr>
          <td class="mono">{c.code}</td>
          <td>{c.pet_name}{c.pet_species ? `（${c.pet_species}）` : ''}</td>
          <td>
            {#if c.service_status === 'completed'}<span class="badge ok">服务已完成</span>
            {:else}<span class="badge warn">服务进行中</span>{/if}
          </td>
          <td>
            {#if c.delivery_status === 'delivered'}<span class="badge done">已交付</span>
            {:else}<span class="badge">待领取</span>{/if}
          </td>
          <td class="muted">{c.created_at}</td>
          <td><a href="#/my/{c.id}">查看进度 →</a></td>
        </tr>
      {:else}
        <tr><td colspan="6" class="muted">您还没有委托记录</td></tr>
      {/each}
    </tbody>
  </table>
</div>
