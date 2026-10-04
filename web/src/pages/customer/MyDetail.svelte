<script>
  import { onMount } from 'svelte'
  import { api, fmtPrice } from '../../api.js'

  export let id

  let d = null
  let error = ''

  onMount(async () => {
    try {
      d = await api(`/my/commissions/${id}`)
    } catch (e) { error = e.message }
  })
</script>

{#if error}
  <div class="alert error">{error}</div>
  <a href="#/my">← 返回我的委托</a>
{/if}

{#if d}
  {@const c = d.commission}
  <div class="card">
    <div class="row">
      <h2 style="margin:0">委托 <span class="mono">{c.code}</span> · {c.pet_name}</h2>
      <span class="spacer"></span>
      {#if c.service_status === 'completed'}<span class="badge ok">服务已完成</span>{:else}<span class="badge warn">服务进行中</span>{/if}
      {#if c.delivery_status === 'delivered'}<span class="badge done">物品已交付</span>{:else}<span class="badge">待领取</span>{/if}
    </div>
    <p class="muted">受理时间：{c.created_at}</p>
    {#if d.delivery}
      <div class="alert success">
        您的物品已于 {d.delivery.delivered_at} 交付给 {d.delivery.receiver_name}
        （{d.delivery.receiver_type === 'proxy' ? '代领人' : '本人'}）。
      </div>
    {/if}
  </div>

  <div class="card">
    <h2>接运信息</h2>
    <table>
      <tbody>
        <tr><th style="width:110px">接运地址</th><td>{c.pickup_address || '—'}</td></tr>
        <tr><th>接运时间</th><td>{c.pickup_time || '—'}</td></tr>
        <tr><th>接运联系人</th><td>{c.pickup_contact || '—'}</td></tr>
        <tr><th>备注</th><td>{c.pickup_notes || '—'}</td></tr>
      </tbody>
    </table>
  </div>

  <div class="card">
    <h2>已确认的服务清单</h2>
    {#each d.versions as v}
      <div class="card" style="margin:8px 0; background:#faf7f1">
        <div class="row">
          <strong>第 {v.version.version_no} 版</strong>
          <span class="muted">确认人：{v.version.confirmed_by} · {v.version.confirmed_at}</span>
        </div>
        <table style="margin-top:6px">
          <tbody>
            {#each v.items as it}
              <tr><td>{it.name}</td><td>{fmtPrice(it.price_cents)}</td><td class="muted">{it.note}</td></tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <p class="muted">服务清单待确认。</p>
    {/each}
  </div>

  <div class="card">
    <h2>遗物保管（{d.relics.length} 件）</h2>
    <table>
      <thead><tr><th>名称</th><th>描述</th><th>当前保管人</th><th>照片</th></tr></thead>
      <tbody>
        {#each d.relics as r}
          <tr>
            <td>{r.name}</td>
            <td class="muted">{r.description}</td>
            <td><span class="badge">{r.holder}</span></td>
            <td>{#if r.photo_path}<a href={r.photo_path} target="_blank"><img class="relic-photo" src={r.photo_path} alt={r.name} /></a>{:else}<span class="muted">—</span>{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <div class="card">
    <h2>移交记录</h2>
    <ul class="timeline">
      {#each d.transfers as t}
        <li>
          <strong>{t.item_label}</strong>：{t.from_holder} → {t.to_holder}
          <span class="muted"> · 签收人 {t.signed_by} · {t.signed_at}</span>
        </li>
      {:else}
        <li class="muted">暂无移交记录</li>
      {/each}
    </ul>
  </div>

  <div class="card">
    <h2>门店通知</h2>
    <ul class="timeline">
      {#each d.notifications as n}
        <li><span class="muted">{n.sent_at}</span><br />{n.content}</li>
      {:else}
        <li class="muted">暂无通知</li>
      {/each}
    </ul>
  </div>
{/if}
