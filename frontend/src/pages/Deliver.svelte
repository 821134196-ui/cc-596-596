<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import { user } from '../stores.js';
  import { SERVICE_STATUS, DELIVERY_STATUS, ITEM_TYPE } from '../util.js';

  export let id;

  let commission = null;
  let customerName = '';
  let items = [];
  let proxies = [];
  let outbounds = [];
  let delivery = null;
  let error = '';
  let success = null;

  // 核对表单
  let confirmNo = '';
  let recipientType = 'self';
  let proxyAuthId = '';
  let recipientName = '';
  let recipientIdNo = '';
  let sealInputs = {}; // item_id -> seal_no
  let sealChecked = {}; // item_id -> bool
  let submitting = false;

  $: isStaff = $user && $user.role === 'staff';
  $: activeProxies = proxies.filter((p) => p.status === 'active');
  $: openOutbound = outbounds.find((o) => o.status === 'open');
  $: allSealsReady =
    items.length > 0 &&
    items.every((row) => (sealInputs[row.item.id] || '').trim() !== '' && sealChecked[row.item.id]);
  $: recipientReady =
    recipientName.trim() !== '' &&
    recipientIdNo.trim() !== '' &&
    (recipientType === 'self' || proxyAuthId !== '');
  $: canSubmit =
    isStaff &&
    commission &&
    commission.service_status === 'completed' &&
    commission.delivery_status === 'none' &&
    confirmNo.trim() === commission.commission_no &&
    allSealsReady &&
    recipientReady &&
    !submitting;

  onMount(async () => {
    try {
      const [c, it, px, ob, dl] = await Promise.all([
        api(`/commissions/${id}`),
        api(`/commissions/${id}/items`),
        api(`/commissions/${id}/proxy`),
        api(`/commissions/${id}/outbound-requests`),
        api(`/commissions/${id}/delivery`),
      ]);
      commission = c.commission;
      customerName = c.customer_name;
      items = it;
      proxies = px;
      outbounds = ob;
      delivery = dl.delivery;
    } catch (e) {
      error = e.message;
    }
  });

  function pickProxy() {
    const p = proxies.find((x) => String(x.id) === String(proxyAuthId));
    if (p) {
      recipientName = p.proxy_name;
      recipientIdNo = p.proxy_id_no;
    }
  }

  async function submit() {
    error = '';
    submitting = true;
    try {
      const res = await api(`/commissions/${id}/deliver`, {
        method: 'POST',
        body: {
          commission_no: confirmNo.trim(),
          outbound_request_id: openOutbound ? openOutbound.id : null,
          recipient_type: recipientType,
          recipient_name: recipientName.trim(),
          recipient_id_no: recipientIdNo.trim(),
          proxy_auth_id: recipientType === 'proxy' ? Number(proxyAuthId) : null,
          seal_checks: items.map((row) => ({
            item_id: row.item.id,
            seal_no: (sealInputs[row.item.id] || '').trim(),
          })),
        },
      });
      success = res;
      commission.delivery_status = 'delivered';
    } catch (e) {
      error = e.message;
      window.scrollTo(0, 0);
    } finally {
      submitting = false;
    }
  }
</script>

{#if error}<div class="alert error">⛔ 交付被阻止：{error}</div>{/if}
{#if success}
  <div class="alert ok">
    ✅ 交付完成：交付单号 #{success.delivery_id}，共交付 {success.delivered_items} 件物品。
    <a href="#/commissions/{id}">返回委托详情</a>
  </div>
{/if}

{#if commission}
  <div class="card">
    <h2>
      领取核对 · 委托 {commission.commission_no}
      <span class="badge {commission.service_status === 'completed' ? 'svc-completed' : 'svc-progress'}">
        服务：{SERVICE_STATUS[commission.service_status]}
      </span>
      <span class="badge {commission.delivery_status === 'delivered' ? 'dlv-done' : 'dlv-none'}">
        物品：{DELIVERY_STATUS[commission.delivery_status]}
      </span>
      <span style="flex:1"></span>
      <a href="#/commissions/{id}" class="muted" style="font-size:13px;">← 返回委托详情</a>
    </h2>
    <p class="muted">
      宠物 {commission.pet_name} · 客户 {customerName}
      {#if openOutbound}· 出库申请 #{openOutbound.id}（待出库）{/if}
    </p>
    <div class="alert info" style="margin-bottom:0;">
      「服务已完成」仅表示殡葬服务环节结束；「物品已交付」表示遗物与骨灰已交予领取人。
      两者相互独立：服务完成后仍需在此逐项核对，才能完成物品交付。
    </div>
  </div>

  {#if delivery}
    <div class="card">
      <h2>交付记录</h2>
      <table>
        <tbody>
          <tr><th style="width:130px">交付单号</th><td>#{delivery.id}</td></tr>
          <tr><th>领取人</th><td>{delivery.recipient_name}（{delivery.recipient_type === 'self' ? '客户本人' : '授权代领人'}）</td></tr>
          <tr><th>证件号</th><td><code>{delivery.recipient_id_no}</code></td></tr>
          <tr><th>交付时间</th><td>{delivery.delivered_at}</td></tr>
          <tr><th>封签核对留档</th><td>
            {#each JSON.parse(delivery.seal_checks_json) as sc}
              <div>✓ {sc.item_name} · <code>{sc.seal_no}</code></div>
            {/each}
          </td></tr>
        </tbody>
      </table>
    </div>
  {:else if !isStaff}
    <div class="card"><p class="muted">物品尚未交付。交付手续由门店员工在领取核对页办理。</p></div>
  {:else if commission.service_status !== 'completed'}
    <div class="card">
      <div class="alert error" style="margin-bottom:0;">
        服务尚未完成（当前：{SERVICE_STATUS[commission.service_status]}），不能交付物品。
      </div>
    </div>
  {:else}
    <div class="card">
      <h2>第一步 · 核对委托号</h2>
      <p class="muted">请对照委托单原件，手工输入委托号进行复核。</p>
      <label class="field" style="max-width:320px;">委托号复核
        <input bind:value={confirmNo} placeholder="请输入 {commission.commission_no}" />
      </label>
      {#if confirmNo && confirmNo.trim() !== commission.commission_no}
        <div class="alert error">委托号不一致，请检查是否拿错单据。</div>
      {/if}
    </div>

    <div class="card">
      <h2>第二步 · 核对领取人</h2>
      <div class="row" style="margin-bottom:12px;">
        <label class="check-line">
          <input type="radio" name="rtype" value="self" bind:group={recipientType} /> 客户本人领取
        </label>
        <label class="check-line">
          <input type="radio" name="rtype" value="proxy" bind:group={recipientType} /> 授权代领人领取
        </label>
      </div>
      {#if recipientType === 'proxy'}
        {#if proxies.length === 0}
          <div class="alert error">该委托没有任何代领授权记录，不能按代领方式交付。</div>
        {:else}
          <label class="field" style="max-width:420px;">选择代领授权
            <select bind:value={proxyAuthId} onchange={pickProxy}>
              <option value="">请选择…</option>
              {#each proxies as p}
                <option value={p.id} disabled={p.status !== 'active'}>
                  {p.proxy_name}（{p.proxy_id_no}）{p.status === 'revoked' ? ' — 已撤销，不可使用' : ''}
                </option>
              {/each}
            </select>
          </label>
          {#if activeProxies.length === 0}
            <div class="alert error">所有代领授权均已被客户撤销，禁止代领交付。</div>
          {/if}
        {/if}
      {/if}
      <div class="form-grid">
        <label class="field">领取人姓名
          <input bind:value={recipientName} readonly={recipientType === 'proxy' && proxyAuthId !== ''} />
        </label>
        <label class="field">领取人证件号
          <input bind:value={recipientIdNo} readonly={recipientType === 'proxy' && proxyAuthId !== ''} />
        </label>
      </div>
    </div>

    <div class="card">
      <h2>第三步 · 逐项核对封签 <span class="sub">对照实物封签，逐件输入并勾选确认</span></h2>
      {#each items as row, i}
        <div class="seal-row">
          <div class="grow">
            <strong>{i + 1}. {row.item.name}</strong>
            <span class="badge">{ITEM_TYPE[row.item.item_type]}</span>
            <div class="muted">{row.item.description}</div>
            <div style="max-width:260px; margin-top:8px;">
              <input
                placeholder={row.item.item_type === 'urn' ? '输入封罐号' : '输入封签号'}
                value={sealInputs[row.item.id] || ''}
                oninput={(e) => (sealInputs[row.item.id] = e.target.value)}
              />
            </div>
          </div>
          <label class="check-line">
            <input type="checkbox"
              checked={!!sealChecked[row.item.id]}
              onchange={(e) => (sealChecked[row.item.id] = e.target.checked)} />
            已核对实物封签
          </label>
        </div>
      {/each}
      {#if items.length === 0}
        <div class="alert error" style="margin-bottom:0;">该委托未登记任何物品，不能交付。</div>
      {/if}
    </div>

    <div class="card">
      <button disabled={!canSubmit} onclick={submit}>
        {submitting ? '提交中…' : '确认交付'}
      </button>
      {#if !canSubmit && !submitting}
        <span class="muted" style="margin-left:10px;">
          需完成：委托号复核、领取人信息、全部封签核对后，才能确认交付
        </span>
      {/if}
    </div>
  {/if}
{/if}
