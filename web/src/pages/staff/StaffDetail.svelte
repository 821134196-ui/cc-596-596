<script>
  import { onMount } from 'svelte'
  import { api, fmtPrice } from '../../api.js'

  export let id

  let d = null
  let error = ''
  let msg = ''

  // 接运信息编辑
  let pickup = { pickup_address: '', pickup_time: '', pickup_contact: '', pickup_notes: '', urn_seal_no: '' }
  // 服务清单编辑
  let items = []
  let confirmedBy = ''
  // 遗物 / 封签 / 移交 / 代领
  let relic = { name: '', description: '', holder: '门店' }
  let seal = { seal_no: '', item_desc: '', sealed_by: '' }
  let transfer = { item_label: '', from_holder: '', to_holder: '', signed_by: '', note: '' }
  let proxy = { name: '', id_no: '', phone: '' }

  async function load() {
    error = ''
    try {
      d = await api(`/staff/commissions/${id}`)
      const c = d.commission
      pickup = {
        pickup_address: c.pickup_address, pickup_time: c.pickup_time,
        pickup_contact: c.pickup_contact, pickup_notes: c.pickup_notes,
        urn_seal_no: c.urn_seal_no,
      }
      items = d.items.map((it) => ({ name: it.name, price_cents: it.price_cents, note: it.note }))
    } catch (e) { error = e.message }
  }

  function flash(m) { msg = m; setTimeout(() => (msg = ''), 4000) }
  async function run(fn, okMsg) {
    error = ''
    try { await fn(); await load(); if (okMsg) flash(okMsg) }
    catch (e) { error = e.message }
  }

  const savePickup = () => run(() => api(`/staff/commissions/${id}/pickup`, { method: 'PUT', body: pickup }), '接运信息已保存')
  const saveItems = () => run(() => api(`/staff/commissions/${id}/items`, { method: 'PUT', body: { items } }), '服务清单已保存')
  const confirmList = () => run(async () => {
    await api(`/staff/commissions/${id}/items`, { method: 'PUT', body: { items } })
    const r = await api(`/staff/commissions/${id}/confirm`, { method: 'POST', body: { confirmed_by: confirmedBy } })
    confirmedBy = ''
    flash(`客户确认完成，已固化为第 ${r.version_no} 版（历史版本不可覆盖）`)
  }, '')
  const addRelic = () => run(() => api(`/staff/commissions/${id}/relics`, { method: 'POST', body: relic }).then(() => (relic = { name: '', description: '', holder: '门店' })), '遗物已登记')
  const addSeal = () => run(() => api(`/staff/commissions/${id}/seals`, { method: 'POST', body: seal }).then(() => (seal = { seal_no: '', item_desc: '', sealed_by: '' })), '封签已登记')
  const addTransfer = () => run(() => api(`/staff/commissions/${id}/transfers`, { method: 'POST', body: transfer }).then(() => (transfer = { item_label: '', from_holder: '', to_holder: '', signed_by: '', note: '' })), '移交已登记，接收人已签收')
  const addProxy = () => run(() => api(`/staff/commissions/${id}/proxies`, { method: 'POST', body: proxy }).then(() => (proxy = { name: '', id_no: '', phone: '' })), '代领授权已添加')
  const revokeProxy = (pid) => run(() => api(`/staff/commissions/${id}/proxies/${pid}/revoke`, { method: 'POST' }), '代领授权已撤销')
  const completeService = () => run(() => api(`/staff/commissions/${id}/complete-service`, { method: 'POST' }), '服务已标记完成')

  async function uploadPhoto(relicId, ev) {
    const file = ev.target.files?.[0]
    if (!file) return
    const fd = new FormData()
    fd.append('file', file)
    await run(() => api(`/staff/commissions/${id}/relics/${relicId}/photo`, { method: 'POST', formData: fd }), '照片已上传')
  }

  function addItemRow() { items = [...items, { name: '', price_cents: 0, note: '' }] }
  function removeItemRow(i) { items = items.filter((_, j) => j !== i) }

  onMount(load)
</script>

{#if error}<div class="alert error">{error}</div>{/if}
{#if msg}<div class="alert success">{msg}</div>{/if}

{#if d}
  {@const c = d.commission}
  <div class="card">
    <div class="row">
      <h2 style="margin:0">委托 <span class="mono">{c.code}</span> · {c.pet_name}{c.pet_species ? `（${c.pet_species}）` : ''}</h2>
      <span class="spacer"></span>
      {#if c.service_status === 'completed'}<span class="badge ok">服务已完成</span>{:else}<span class="badge warn">服务进行中</span>{/if}
      {#if c.delivery_status === 'delivered'}<span class="badge done">物品已交付</span>{:else}<span class="badge">待交付</span>{/if}
    </div>
    <p class="muted">客户：{d.customer.name}（{d.customer.phone || '未留电话'}） · 创建于 {c.created_at}</p>
    {#if c.service_status !== 'completed'}
      <button on:click={completeService}>✔ 标记服务完成</button>
    {:else if c.delivery_status !== 'delivered'}
      <a href="#/staff/delivery"><button>前往领取交付页 →</button></a>
    {/if}
    {#if d.delivery}
      <div class="alert success" style="margin-top:10px">
        交付记录：{d.delivery.delivered_at} 由 {d.delivery.delivered_by} 经办，
        交付给 {d.delivery.receiver_name}（{d.delivery.receiver_type === 'proxy' ? '代领人' : '客户本人'}），
        核对封罐号 <span class="mono">{d.delivery.urn_seal_no}</span>
      </div>
    {/if}
  </div>

  <div class="card">
    <h2>接运信息</h2>
    <div class="form-row">
      <label class="field">接运地址<input bind:value={pickup.pickup_address} /></label>
      <label class="field">接运时间<input bind:value={pickup.pickup_time} /></label>
      <label class="field">接运联系人<input bind:value={pickup.pickup_contact} /></label>
      <label class="field">备注<input bind:value={pickup.pickup_notes} /></label>
    </div>
    <label class="field" style="max-width:280px">骨灰封罐号（交付核对依据）
      <input class="mono" bind:value={pickup.urn_seal_no} placeholder="如 URN-2026-0001" />
    </label>
    <button class="ghost" on:click={savePickup}>保存接运信息</button>
  </div>

  <div class="card">
    <h2>服务项目（当前清单）</h2>
    <table>
      <thead><tr><th>项目</th><th>价格（元）</th><th>备注</th><th></th></tr></thead>
      <tbody>
        {#each items as it, i}
          <tr>
            <td><input bind:value={it.name} placeholder="如：遗体接运" /></td>
            <td style="width:130px"><input type="number" min="0" step="0.01"
              value={it.price_cents / 100}
              on:change={(e) => (it.price_cents = Math.round(parseFloat(e.target.value || '0') * 100))} /></td>
            <td><input bind:value={it.note} /></td>
            <td><button class="ghost" on:click={() => removeItemRow(i)}>删</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="row" style="margin-top:10px">
      <button class="ghost" on:click={addItemRow}>＋ 添加项目</button>
      <button class="ghost" on:click={saveItems}>保存清单</button>
      <span class="spacer"></span>
      <input style="max-width:180px" bind:value={confirmedBy} placeholder="确认人（客户）姓名" />
      <button on:click={confirmList} disabled={!confirmedBy}>客户确认，固化为版本</button>
    </div>

    <h3>客户确认历史（{d.versions.length} 个版本，确认后不可更改）</h3>
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
      <p class="muted">尚未确认过服务清单。</p>
    {/each}
  </div>

  <div class="card">
    <h2>遗物清单（{d.relics.length} 件）</h2>
    <table>
      <thead><tr><th>名称</th><th>描述</th><th>当前保管人</th><th>照片</th><th>上传</th></tr></thead>
      <tbody>
        {#each d.relics as r}
          <tr>
            <td>{r.name}</td>
            <td class="muted">{r.description}</td>
            <td><span class="badge">{r.holder}</span></td>
            <td>{#if r.photo_path}<a href={r.photo_path} target="_blank"><img class="relic-photo" src={r.photo_path} alt={r.name} /></a>{:else}<span class="muted">无</span>{/if}</td>
            <td><input type="file" accept="image/*" on:change={(e) => uploadPhoto(r.id, e)} /></td>
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="row" style="margin-top:10px">
      <input style="max-width:160px" bind:value={relic.name} placeholder="遗物名称 *" />
      <input style="max-width:220px" bind:value={relic.description} placeholder="描述" />
      <input style="max-width:140px" bind:value={relic.holder} placeholder="保管人" />
      <button on:click={addRelic} disabled={!relic.name}>登记遗物</button>
    </div>
  </div>

  <div class="card">
    <h2>封签（{d.seals.length}）</h2>
    <table>
      <thead><tr><th>封签号</th><th>封于何物</th><th>经办</th><th>时间</th></tr></thead>
      <tbody>
        {#each d.seals as s}
          <tr><td class="mono">{s.seal_no}</td><td>{s.item_desc}</td><td>{s.sealed_by}</td><td class="muted">{s.sealed_at}</td></tr>
        {/each}
      </tbody>
    </table>
    <div class="row" style="margin-top:10px">
      <input style="max-width:180px" class="mono" bind:value={seal.seal_no} placeholder="封签号 *" />
      <input style="max-width:200px" bind:value={seal.item_desc} placeholder="封于何物（如骨灰罐）" />
      <input style="max-width:120px" bind:value={seal.sealed_by} placeholder="经办人" />
      <button on:click={addSeal} disabled={!seal.seal_no}>登记封签</button>
    </div>
  </div>

  <div class="card">
    <h2>移交签收记录（{d.transfers.length}）</h2>
    <ul class="timeline">
      {#each d.transfers as t}
        <li>
          <strong>{t.item_label}</strong>：{t.from_holder} → {t.to_holder}
          <span class="muted"> · 签收人 {t.signed_by} · {t.signed_at}{t.note ? ` · ${t.note}` : ''}</span>
        </li>
      {:else}
        <li class="muted">暂无移交记录</li>
      {/each}
    </ul>
    <div class="row" style="margin-top:12px">
      <input style="max-width:150px" bind:value={transfer.item_label} placeholder="移交物品 *" />
      <input style="max-width:110px" bind:value={transfer.from_holder} placeholder="移交方" />
      <input style="max-width:110px" bind:value={transfer.to_holder} placeholder="接收方 *" />
      <input style="max-width:110px" bind:value={transfer.signed_by} placeholder="签收人 *" />
      <input style="max-width:150px" bind:value={transfer.note} placeholder="备注" />
      <button on:click={addTransfer} disabled={!transfer.item_label || !transfer.to_holder || !transfer.signed_by}>登记移交</button>
    </div>
  </div>

  <div class="card">
    <h2>代领授权（{d.proxies.length}）</h2>
    <table>
      <thead><tr><th>姓名</th><th>证件号</th><th>电话</th><th>状态</th><th></th></tr></thead>
      <tbody>
        {#each d.proxies as p}
          <tr>
            <td>{p.name}</td><td class="mono">{p.id_no}</td><td>{p.phone}</td>
            <td>{#if p.status === 'active'}<span class="badge ok">有效</span>{:else}<span class="badge warn">已撤销</span>{/if}</td>
            <td>{#if p.status === 'active'}<button class="danger" on:click={() => revokeProxy(p.id)}>撤销</button>{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="row" style="margin-top:10px">
      <input style="max-width:140px" bind:value={proxy.name} placeholder="代领人姓名 *" />
      <input style="max-width:200px" bind:value={proxy.id_no} placeholder="证件号" />
      <input style="max-width:150px" bind:value={proxy.phone} placeholder="电话" />
      <button on:click={addProxy} disabled={!proxy.name}>添加授权</button>
    </div>
  </div>

  <div class="card">
    <h2>通知记录（模拟接口）</h2>
    <ul class="timeline">
      {#each d.notifications as n}
        <li><span class="muted">{n.sent_at} → {n.recipient}</span><br />{n.content}</li>
      {:else}
        <li class="muted">暂无通知</li>
      {/each}
    </ul>
  </div>
{/if}
