<script>
  import { onMount } from 'svelte';
  import { api, uploadPhoto } from '../api.js';
  import { user } from '../stores.js';
  import {
    SERVICE_STATUS, SERVICE_NEXT, DELIVERY_STATUS, ITEM_TYPE, ITEM_STATUS,
    TRANSFER_TYPE, fenToYuan,
  } from '../util.js';

  export let id;

  let commission = null;
  let customerName = '';
  let serviceItems = [];
  let versions = [];
  let items = [];
  let transfers = [];
  let proxies = [];
  let outbounds = [];
  let delivery = null;
  let notifications = [];
  let staffList = [];
  let error = '';
  let toast = '';

  // 表单状态
  let newService = { name: '', price: '', note: '' };
  let newItem = { item_type: 'relic', name: '', seal_no: '', description: '' };
  let transferForms = {}; // item_id -> {to_holder_id, transfer_type, notes}
  let newProxy = { proxy_name: '', proxy_id_no: '', proxy_phone: '' };
  let pickupEdit = null;

  $: isStaff = $user && $user.role === 'staff';
  $: isOwner = commission && $user && commission.customer_id === $user.id;

  async function reload() {
    error = '';
    try {
      const [c, si, ver, it, tr, px, ob, dl, nt] = await Promise.all([
        api(`/commissions/${id}`),
        api(`/commissions/${id}/service-items`),
        api(`/commissions/${id}/service-list/versions`),
        api(`/commissions/${id}/items`),
        api(`/commissions/${id}/transfers`),
        api(`/commissions/${id}/proxy`),
        api(`/commissions/${id}/outbound-requests`),
        api(`/commissions/${id}/delivery`),
        api(`/commissions/${id}/notifications`),
      ]);
      commission = c.commission;
      customerName = c.customer_name;
      serviceItems = si;
      versions = ver;
      items = it;
      transfers = tr;
      proxies = px;
      outbounds = ob;
      delivery = dl.delivery;
      notifications = nt;
      if (isStaff && staffList.length === 0) staffList = await api('/staff');
    } catch (e) {
      error = e.message;
    }
  }

  onMount(reload);

  function flash(msg) {
    toast = msg;
    setTimeout(() => (toast = ''), 3000);
  }

  async function run(fn, okMsg) {
    error = '';
    try {
      await fn();
      if (okMsg) flash(okMsg);
      await reload();
    } catch (e) {
      error = e.message;
      window.scrollTo(0, 0);
    }
  }

  const advanceStatus = () => run(
    () => api(`/commissions/${id}/status`, {
      method: 'POST',
      body: { service_status: SERVICE_NEXT[commission.service_status] },
    }),
    '服务状态已推进'
  );

  const savePickup = () => run(async () => {
    await api(`/commissions/${id}/pickup`, { method: 'PUT', body: pickupEdit });
    pickupEdit = null;
  }, '接运信息已保存');

  const addService = () => run(async () => {
    await api(`/commissions/${id}/service-items`, {
      method: 'POST',
      body: {
        name: newService.name,
        price_cents: Math.round(parseFloat(newService.price || '0') * 100),
        note: newService.note,
      },
    });
    newService = { name: '', price: '', note: '' };
  }, '已添加服务项目');

  const removeService = (itemId) => run(
    () => api(`/service-items/${itemId}`, { method: 'DELETE' }),
    '已删除服务项目'
  );

  const confirmList = () => run(
    () => api(`/commissions/${id}/service-list/confirm`, { method: 'POST' }),
    '服务清单已确认并存档为不可变版本'
  );

  const addItem = () => run(async () => {
    await api(`/commissions/${id}/items`, { method: 'POST', body: newItem });
    newItem = { item_type: 'relic', name: '', seal_no: '', description: '' };
  }, '物品已登记');

  const doUpload = (itemId, ev) => run(async () => {
    const file = ev.target.files[0];
    if (file) await uploadPhoto(itemId, file);
  }, '照片已上传');

  const createTransfer = (itemId) => run(async () => {
    const f = transferForms[itemId] || {};
    await api(`/items/${itemId}/transfers`, {
      method: 'POST',
      body: {
        to_holder_id: Number(f.to_holder_id),
        transfer_type: f.transfer_type || 'service',
        notes: f.notes || '',
      },
    });
    transferForms[itemId] = {};
  }, '移交已发起，等待接收人签收');

  const signTransfer = (transferId) => run(
    () => api(`/transfers/${transferId}/sign`, { method: 'POST' }),
    '已签收，物品保管人已更新'
  );

  const addProxy = () => run(async () => {
    await api(`/commissions/${id}/proxy`, { method: 'POST', body: newProxy });
    newProxy = { proxy_name: '', proxy_id_no: '', proxy_phone: '' };
  }, '代领授权已登记');

  const revokeProxy = (pid) => run(
    () => api(`/proxy/${pid}/revoke`, { method: 'POST' }),
    '代领授权已撤销'
  );

  const createOutbound = () => run(
    () => api(`/commissions/${id}/outbound-requests`, { method: 'POST' }),
    '出库申请已创建'
  );

  function startEditPickup() {
    pickupEdit = {
      pickup_address: commission.pickup_address,
      pickup_contact: commission.pickup_contact,
      pickup_phone: commission.pickup_phone,
      pickup_time: commission.pickup_time,
      pickup_notes: commission.pickup_notes,
    };
  }
</script>

{#if error}<div class="alert error">{error}</div>{/if}
{#if toast}<div class="alert ok">{toast}</div>{/if}

{#if commission}
  <div class="card">
    <h2>
      委托 {commission.commission_no}
      <span class="badge {commission.service_status === 'completed' ? 'svc-completed' : 'svc-progress'}">
        服务：{SERVICE_STATUS[commission.service_status]}
      </span>
      <span class="badge {commission.delivery_status === 'delivered' ? 'dlv-done' : 'dlv-none'}">
        物品：{DELIVERY_STATUS[commission.delivery_status]}
      </span>
      <span style="flex:1"></span>
      <a href="#/" class="muted" style="font-size:13px;">← 返回列表</a>
    </h2>
    <p>
      宠物：<strong>{commission.pet_name}</strong>（{commission.pet_species || '未填写'}） ·
      客户：{customerName} · 建立于 {commission.created_at}
    </p>
    {#if isStaff}
      <div class="row">
        {#if SERVICE_NEXT[commission.service_status]}
          <button class="small" onclick={advanceStatus}>
            推进服务状态 → {SERVICE_STATUS[SERVICE_NEXT[commission.service_status]]}
          </button>
        {/if}
        {#if commission.service_status === 'completed' && commission.delivery_status === 'none'}
          <a href="#/commissions/{id}/deliver"><button class="small">前往领取核对页 →</button></a>
        {/if}
      </div>
    {/if}
  </div>

  <!-- 接运信息 -->
  <div class="card">
    <h2>接运信息
      <span style="flex:1"></span>
      {#if isStaff && !pickupEdit}
        <button class="ghost small" onclick={startEditPickup}>编辑</button>
      {/if}
    </h2>
    {#if pickupEdit}
      <div class="form-grid">
        <label class="field">接运地址 <input bind:value={pickupEdit.pickup_address} /></label>
        <label class="field">联系人 <input bind:value={pickupEdit.pickup_contact} /></label>
        <label class="field">联系电话 <input bind:value={pickupEdit.pickup_phone} /></label>
        <label class="field">预约时间 <input bind:value={pickupEdit.pickup_time} /></label>
      </div>
      <label class="field">备注 <textarea rows="2" bind:value={pickupEdit.pickup_notes}></textarea></label>
      <div class="row">
        <button class="small" onclick={savePickup}>保存</button>
        <button class="ghost small" onclick={() => (pickupEdit = null)}>取消</button>
      </div>
    {:else}
      <table>
        <tbody>
          <tr><th style="width:110px">地址</th><td>{commission.pickup_address || '—'}</td></tr>
          <tr><th>联系人</th><td>{commission.pickup_contact || '—'} {commission.pickup_phone}</td></tr>
          <tr><th>预约时间</th><td>{commission.pickup_time || '—'}</td></tr>
          <tr><th>备注</th><td>{commission.pickup_notes || '—'}</td></tr>
        </tbody>
      </table>
    {/if}
  </div>

  <!-- 服务项目 -->
  <div class="card">
    <h2>服务项目 <span class="sub">当前工作清单，门店可调整</span></h2>
    <table>
      <thead><tr><th>项目</th><th>价格</th><th>备注</th>{#if isStaff}<th></th>{/if}</tr></thead>
      <tbody>
        {#each serviceItems as s}
          <tr>
            <td>{s.name}</td>
            <td>{fenToYuan(s.price_cents)}</td>
            <td class="muted">{s.note}</td>
            {#if isStaff}
              <td><button class="ghost small" onclick={() => removeService(s.id)}>删除</button></td>
            {/if}
          </tr>
        {/each}
        {#if serviceItems.length === 0}
          <tr><td colspan="4" class="muted">尚未添加服务项目</td></tr>
        {/if}
      </tbody>
    </table>
    {#if isStaff}
      <div class="row" style="margin-top:10px;">
        <input style="width:200px" placeholder="项目名称" bind:value={newService.name} />
        <input style="width:110px" placeholder="价格（元）" type="number" min="0" step="0.01" bind:value={newService.price} />
        <input style="width:180px" placeholder="备注" bind:value={newService.note} />
        <button class="small" onclick={addService} disabled={!newService.name}>添加项目</button>
      </div>
    {/if}
    {#if isOwner}
      <div style="margin-top:10px;">
        <button class="small" onclick={confirmList} disabled={serviceItems.length === 0}>
          ✓ 我确认当前服务清单（生成不可变存档版本）
        </button>
      </div>
    {/if}
  </div>

  <!-- 历史确认版本 -->
  <div class="card">
    <h2>客户确认的服务清单版本 <span class="sub">独立存档，后续变更不会覆盖</span></h2>
    {#if versions.length === 0}
      <p class="muted">客户尚未确认过服务清单。</p>
    {/if}
    {#each versions as v}
      <details style="margin-bottom:8px;">
        <summary>
          <strong>第 {v.version_no} 版</strong> · 合计 {fenToYuan(v.total_cents)} · 确认于 {v.confirmed_at}
        </summary>
        <table style="margin-top:8px;">
          <thead><tr><th>项目</th><th>价格</th><th>备注</th></tr></thead>
          <tbody>
            {#each JSON.parse(v.items_json) as it}
              <tr><td>{it.name}</td><td>{fenToYuan(it.price_cents)}</td><td class="muted">{it.note}</td></tr>
            {/each}
          </tbody>
        </table>
      </details>
    {/each}
  </div>

  <!-- 遗物与骨灰罐 -->
  <div class="card">
    <h2>遗物与骨灰罐 <span class="sub">每件物品均有唯一封签号，当前保管人实时可查</span></h2>
    <table>
      <thead>
        <tr><th>照片</th><th>物品</th><th>封签号 / 封罐号</th><th>状态</th><th>当前保管人</th>{#if isStaff}<th>操作</th>{/if}</tr>
      </thead>
      <tbody>
        {#each items as row}
          <tr>
            <td>
              {#if row.item.photo_path}
                <img class="photo-thumb" src="/photos/{row.item.photo_path}" alt={row.item.name} />
              {:else}
                <span class="muted">无照片</span>
              {/if}
            </td>
            <td>
              <span class="badge">{ITEM_TYPE[row.item.item_type]}</span> {row.item.name}
              <div class="muted">{row.item.description}</div>
            </td>
            <td><code>{row.item.seal_no}</code></td>
            <td><span class="badge {row.item.status === 'delivered' ? 'dlv-done' : ''}">{ITEM_STATUS[row.item.status]}</span></td>
            <td>{row.holder_name || '—'}</td>
            {#if isStaff}
              <td>
                {#if row.item.status !== 'delivered'}
                  <label class="muted" style="font-size:12px;">上传照片
                    <input type="file" accept="image/*" style="font-size:12px;" onchange={(e) => doUpload(row.item.id, e)} />
                  </label>
                  {#if row.item.status === 'in_store' && row.item.current_holder_id === $user.id}
                    <div class="row" style="margin-top:6px;">
                      <select style="width:110px"
                        onchange={(e) => { transferForms[row.item.id] = { ...(transferForms[row.item.id] || {}), to_holder_id: e.target.value }; }}>
                        <option value="">移交给…</option>
                        {#each staffList.filter((s) => s.id !== $user.id) as s}
                          <option value={s.id}>{s.name}</option>
                        {/each}
                      </select>
                      <select style="width:110px"
                        onchange={(e) => { transferForms[row.item.id] = { ...(transferForms[row.item.id] || {}), transfer_type: e.target.value }; }}>
                        <option value="service">服务流转</option>
                        <option value="intake">入店</option>
                        <option value="storage">封存</option>
                        <option value="pickup">接运</option>
                      </select>
                      <button class="small" onclick={() => createTransfer(row.item.id)}
                        disabled={!(transferForms[row.item.id] && transferForms[row.item.id].to_holder_id)}>
                        发起移交
                      </button>
                    </div>
                  {/if}
                {/if}
              </td>
            {/if}
          </tr>
        {/each}
        {#if items.length === 0}
          <tr><td colspan="6" class="muted">尚未登记遗物或骨灰罐</td></tr>
        {/if}
      </tbody>
    </table>
    {#if isStaff && commission.delivery_status === 'none'}
      <div class="row" style="margin-top:10px;">
        <select style="width:110px" bind:value={newItem.item_type}>
          <option value="relic">遗物</option>
          <option value="urn">骨灰罐</option>
        </select>
        <input style="width:160px" placeholder="物品名称" bind:value={newItem.name} />
        <input style="width:150px" placeholder={newItem.item_type === 'urn' ? '封罐号' : '封签号'} bind:value={newItem.seal_no} />
        <input style="width:180px" placeholder="描述" bind:value={newItem.description} />
        <button class="small" onclick={addItem} disabled={!newItem.name || !newItem.seal_no}>登记物品</button>
      </div>
    {/if}
  </div>

  <!-- 移交记录 -->
  <div class="card">
    <h2>移交记录 <span class="sub">每次移交由接收人签收后生效</span></h2>
    {#if transfers.length === 0}
      <p class="muted">暂无移交记录。</p>
    {:else}
      <ul class="timeline">
        {#each transfers as t}
          <li>
            <strong>{t.item_name}</strong> · {TRANSFER_TYPE[t.transfer_type] || t.transfer_type} ·
            {t.transfer.from_holder || '门店'} → {t.transfer.to_holder_name}
            {#if t.transfer.status === 'signed'}
              <span class="badge active">已签收 {t.transfer.signed_at}</span>
            {:else}
              <span class="badge svc-progress">待签收</span>
              {#if isStaff && t.transfer.to_holder_id === $user.id}
                <button class="small" onclick={() => signTransfer(t.transfer.id)}>本人签收</button>
              {/if}
            {/if}
            <div class="muted">{t.transfer.notes} · 发起于 {t.transfer.created_at}</div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <!-- 代领授权 -->
  <div class="card">
    <h2>代领授权 <span class="sub">客户可授权他人代领，也可随时撤销</span></h2>
    {#if proxies.length === 0}
      <p class="muted">暂无代领授权。</p>
    {:else}
      <table>
        <thead><tr><th>代领人</th><th>证件号</th><th>电话</th><th>状态</th>{#if isOwner}<th></th>{/if}</tr></thead>
        <tbody>
          {#each proxies as p}
            <tr>
              <td>{p.proxy_name}</td>
              <td><code>{p.proxy_id_no}</code></td>
              <td>{p.proxy_phone || '—'}</td>
              <td>
                {#if p.status === 'active'}
                  <span class="badge active">有效</span>
                {:else}
                  <span class="badge revoked">已撤销 {p.revoked_at}</span>
                {/if}
              </td>
              {#if isOwner}
                <td>
                  {#if p.status === 'active'}
                    <button class="danger small" onclick={() => revokeProxy(p.id)}>撤销授权</button>
                  {/if}
                </td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {#if isOwner && commission.delivery_status === 'none'}
      <div class="row" style="margin-top:10px;">
        <input style="width:140px" placeholder="代领人姓名" bind:value={newProxy.proxy_name} />
        <input style="width:190px" placeholder="代领人证件号" bind:value={newProxy.proxy_id_no} />
        <input style="width:140px" placeholder="代领人电话" bind:value={newProxy.proxy_phone} />
        <button class="small" onclick={addProxy} disabled={!newProxy.proxy_name || !newProxy.proxy_id_no}>登记授权</button>
      </div>
    {/if}
  </div>

  <!-- 出库与交付 -->
  <div class="card">
    <h2>出库与交付 <span class="sub">服务完成后可申请出库；交付在领取核对页完成</span></h2>
    {#if delivery}
      <div class="alert ok">
        已于 {delivery.delivered_at} 交付给 {delivery.recipient_name}
        （{delivery.recipient_type === 'self' ? '客户本人' : '授权代领人'}），交付单号 #{delivery.id}
      </div>
    {:else}
      {#if outbounds.length > 0}
        <table>
          <thead><tr><th>申请编号</th><th>状态</th><th>申请时间</th></tr></thead>
          <tbody>
            {#each outbounds as o}
              <tr>
                <td>#{o.id}</td>
                <td>
                  <span class="badge {o.status === 'open' ? 'svc-progress' : o.status === 'fulfilled' ? 'dlv-done' : ''}">
                    {o.status === 'open' ? '待出库' : o.status === 'fulfilled' ? '已出库' : '已取消'}
                  </span>
                </td>
                <td class="muted">{o.created_at}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
      {#if isStaff}
        <div class="row" style="margin-top:10px;">
          <button class="small" onclick={createOutbound}
            disabled={commission.service_status !== 'completed' || outbounds.some((o) => o.status === 'open')}>
            申请出库
          </button>
          {#if outbounds.some((o) => o.status === 'open')}
            <span class="muted">已存在未完成的出库申请，不能重复申请</span>
          {/if}
          {#if commission.service_status === 'completed'}
            <a href="#/commissions/{id}/deliver"><button class="small">前往领取核对页 →</button></a>
          {:else}
            <span class="muted">服务完成后才能申请出库与交付</span>
          {/if}
        </div>
      {/if}
    {/if}
  </div>

  <!-- 通知记录 -->
  <div class="card">
    <h2>通知记录 <span class="sub">模拟接口发送，仅作留档</span></h2>
    {#if notifications.length === 0}
      <p class="muted">暂无通知。</p>
    {:else}
      <table>
        <thead><tr><th>时间</th><th>收件人</th><th>内容</th><th>状态</th></tr></thead>
        <tbody>
          {#each notifications as n}
            <tr>
              <td class="muted">{n.created_at}</td>
              <td>{n.recipient}</td>
              <td>{n.content}</td>
              <td><span class="badge active">模拟已发送</span></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
{/if}
