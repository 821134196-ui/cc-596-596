<script>
  import { api } from '../../api.js'

  let code = ''
  let info = null
  let loadError = ''

  // 核对项
  let checkedCode = false
  let sealInput = ''
  let receiverType = 'customer'
  let receiverName = ''
  let receiverIdNo = ''

  let result = null   // { ok, message }

  async function lookup() {
    loadError = ''; info = null; result = null
    resetChecks()
    try {
      info = await api(`/staff/delivery-info/${encodeURIComponent(code.trim())}`)
    } catch (e) { loadError = e.message }
  }

  function resetChecks() {
    checkedCode = false; sealInput = ''; receiverType = 'customer'; receiverName = ''; receiverIdNo = ''
  }

  $: sealReady = sealInput.trim().length > 0
  $: receiverReady = receiverName.trim().length > 0
  $: allChecked = checkedCode && sealReady && receiverReady
  $: canDeliver = info && allChecked &&
    info.commission.service_status === 'completed' &&
    info.commission.delivery_status !== 'delivered'

  async function submit() {
    result = null
    try {
      await api('/deliveries', {
        method: 'POST',
        body: {
          commission_code: info.commission.code,
          urn_seal_no: sealInput.trim(),
          receiver_type: receiverType,
          receiver_name: receiverName.trim(),
          receiver_id_no: receiverIdNo.trim(),
        },
      })
      result = { ok: true, message: `交付成功：委托 ${info.commission.code} 的骨灰与遗物已出库交付给 ${receiverName}` }
      await lookup() // 刷新状态
    } catch (e) {
      result = { ok: false, message: e.message, code: e.code }
    }
  }
</script>

<div class="card">
  <h2>领取交付 · 出库核对</h2>
  <p class="muted">交付前请逐项核对：委托号、封罐号、领取人身份。系统会在服务端再次校验并阻止异常交付。</p>
  <div class="row">
    <input style="max-width:260px" class="mono" bind:value={code} placeholder="输入委托号，如 WT-20261004-0001"
      on:keydown={(e) => e.key === 'Enter' && lookup()} />
    <button on:click={lookup} disabled={!code.trim()}>查询委托</button>
  </div>
  {#if loadError}<div class="alert error">{loadError}</div>{/if}
</div>

{#if info}
  {@const c = info.commission}
  <div class="card">
    <div class="row">
      <h2 style="margin:0">委托 <span class="mono">{c.code}</span> · {c.pet_name}</h2>
      <span class="spacer"></span>
      {#if c.service_status === 'completed'}<span class="badge ok">服务已完成</span>{:else}<span class="badge warn">服务进行中</span>{/if}
      {#if c.delivery_status === 'delivered'}<span class="badge done">物品已交付</span>{:else}<span class="badge">待交付</span>{/if}
    </div>
    <p class="muted">客户：{info.customer.name}（{info.customer.phone || '未留电话'}）</p>

    {#if c.service_status !== 'completed'}
      <div class="alert error">该委托服务尚未完成，不能交付。请先在委托详情中完成服务。</div>
    {/if}
    {#if info.delivery}
      <div class="alert info">
        本委托已于 {info.delivery.delivered_at} 由 {info.delivery.delivered_by} 经办交付给
        {info.delivery.receiver_name}（{info.delivery.receiver_type === 'proxy' ? '代领人' : '客户本人'}）。
        同一委托不可重复出库。
      </div>
    {/if}

    <h3>逐项核对</h3>

    <div class="check-item" class:checked={checkedCode}>
      <input type="checkbox" id="ck1" bind:checked={checkedCode} />
      <label for="ck1" style="flex:1">
        <strong>① 委托号核对</strong><br />
        <span class="muted">实物单据上的委托号应为 <span class="mono">{c.code}</span>，确认一致后勾选</span>
      </label>
    </div>

    <div class="check-item" class:checked={sealReady}>
      <input type="checkbox" checked={sealReady} disabled />
      <div style="flex:1">
        <strong>② 封罐号核对</strong><br />
        <span class="muted">请从骨灰罐实物封签上读取编号并录入，系统将与委托登记比对（页面不显示登记值，防止照抄）</span><br />
        <input class="mono" style="max-width:240px; margin-top:6px" bind:value={sealInput} placeholder="录入实物封罐号" />
      </div>
    </div>

    <div class="check-item" class:checked={receiverReady}>
      <input type="checkbox" checked={receiverReady} disabled />
      <div style="flex:1">
        <strong>③ 领取人核对</strong><br />
        <span class="muted">客户本人：{info.customer.name}；有效代领人：
          {#each info.proxies.filter((p) => p.status === 'active') as p}{p.name} {/each}
          {#if info.proxies.filter((p) => p.status === 'active').length === 0}（无）{/if}
          {#if info.proxies.some((p) => p.status === 'revoked')}
            · <span style="color:var(--danger)">已撤销：{info.proxies.filter((p) => p.status === 'revoked').map((p) => p.name).join('、')}</span>
          {/if}
        </span>
        <div class="row" style="margin-top:6px">
          <select style="max-width:140px" bind:value={receiverType}>
            <option value="customer">客户本人</option>
            <option value="proxy">代领人</option>
          </select>
          <input style="max-width:160px" bind:value={receiverName} placeholder="领取人姓名 *" />
          <input style="max-width:200px" bind:value={receiverIdNo} placeholder="证件号（可选）" />
        </div>
      </div>
    </div>

    {#if result}
      <div class="alert {result.ok ? 'success' : 'error'}">
        {result.message}
        {#if result.code}<span class="muted">（{result.code}）</span>{/if}
      </div>
    {/if}

    <div class="row">
      <button on:click={submit} disabled={!canDeliver}>确认交付，物品出库</button>
      {#if !allChecked && c.service_status === 'completed' && c.delivery_status !== 'delivered'}
        <span class="muted">请先完成全部三项核对</span>
      {/if}
    </div>
  </div>
{/if}
