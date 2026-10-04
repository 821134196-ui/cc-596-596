#!/usr/bin/env node
/**
 * 端到端验收脚本：验证系统的关键业务拦截与数据完整性。
 * 用法: node scripts/verify.mjs [baseURL]   （默认 http://localhost:8080）
 */
const BASE = process.argv[2] || 'http://localhost:8080'

let passed = 0, failed = 0
function check(name, cond, detail = '') {
  if (cond) { passed++; console.log(`  ✔ ${name}`) }
  else { failed++; console.log(`  ✘ ${name} ${detail}`) }
}

async function api(path, { method = 'GET', token, body } = {}) {
  const res = await fetch(BASE + '/api' + path, {
    method,
    headers: {
      ...(body ? { 'Content-Type': 'application/json' } : {}),
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  })
  const data = await res.json().catch(() => ({}))
  return { status: res.status, data }
}

async function login(username, password) {
  const r = await api('/login', { method: 'POST', body: { username, password } })
  if (r.status !== 200) throw new Error(`登录失败 ${username}: ${JSON.stringify(r.data)}`)
  return r.data.token
}

const suffix = Date.now().toString(36)
const CUSTOMER = `cust_${suffix}`

async function main() {
  console.log(`\n=== 宠物殡葬交付系统验收（${BASE}）===\n`)
  const staff = await login('staff', 'staff123')
  const chen = await login('chen', '123456')

  // ---------- 1. 建单 + 接运信息 + 封罐号 ----------
  console.log('▶ 1. 创建委托并登记接运信息')
  const created = await api('/staff/commissions', {
    method: 'POST', token: staff,
    body: {
      customer_username: CUSTOMER, customer_name: '测试客户', customer_phone: '13900000000',
      pet_name: '豆豆', pet_species: '猫',
      pickup_address: '上海市静安区示例路 1 号', pickup_time: '2026-10-04 10:00',
      pickup_contact: '测试客户', pickup_notes: '上门接运',
    },
  })
  check('创建委托成功', created.status === 200 && created.data.code, JSON.stringify(created.data))
  const cid = created.data.id
  const code = created.data.code

  const pickup = await api(`/staff/commissions/${cid}/pickup`, {
    method: 'PUT', token: staff,
    body: { pickup_address: '上海市静安区示例路 1 号', pickup_time: '2026-10-04 10:00', pickup_contact: '测试客户', pickup_notes: '', urn_seal_no: `URN-${suffix}` },
  })
  check('登记封罐号成功', pickup.status === 200)

  // ---------- 2. 服务清单版本不可覆盖 ----------
  console.log('▶ 2. 服务清单确认与历史版本')
  await api(`/staff/commissions/${cid}/items`, {
    method: 'PUT', token: staff,
    body: { items: [{ name: '遗体接运', price_cents: 30000 }, { name: '单独火化', price_cents: 80000, note: '全程可视' }] },
  })
  const v1 = await api(`/staff/commissions/${cid}/confirm`, { method: 'POST', token: staff, body: { confirmed_by: '测试客户' } })
  check('第 1 版确认成功', v1.status === 200 && v1.data.version_no === 1)

  // 客户确认后变更清单（加价项目），再次确认为第 2 版
  await api(`/staff/commissions/${cid}/items`, {
    method: 'PUT', token: staff,
    body: { items: [{ name: '遗体接运', price_cents: 30000 }, { name: '单独火化', price_cents: 80000 }, { name: '告别仪式', price_cents: 120000 }] },
  })
  const v2 = await api(`/staff/commissions/${cid}/confirm`, { method: 'POST', token: staff, body: { confirmed_by: '测试客户' } })
  check('第 2 版确认成功', v2.status === 200 && v2.data.version_no === 2)

  const detail = await api(`/staff/commissions/${cid}`, { token: staff })
  const ver1 = detail.data.versions.find((v) => v.version.version_no === 1)
  const ver2 = detail.data.versions.find((v) => v.version.version_no === 2)
  check('历史第 1 版仍可查询', !!ver1)
  check('第 1 版内容未被后续变更覆盖（仍 2 项、无告别仪式）',
    ver1 && ver1.items.length === 2 && !ver1.items.some((i) => i.name === '告别仪式'))
  check('第 2 版包含新增项目（3 项）', ver2 && ver2.items.length === 3)

  // ---------- 3. 遗物 / 封签 / 移交 ----------
  console.log('▶ 3. 遗物登记与移交签收')
  const relic = await api(`/staff/commissions/${cid}/relics`, { method: 'POST', token: staff, body: { name: '项圈', description: '红色皮质项圈', holder: '门店前台' } })
  check('遗物登记成功', relic.status === 200)
  await api(`/staff/commissions/${cid}/seals`, { method: 'POST', token: staff, body: { seal_no: `SEAL-${suffix}`, item_desc: '遗物袋', sealed_by: '王店员' } })
  const tr = await api(`/staff/commissions/${cid}/transfers`, { method: 'POST', token: staff, body: { item_label: '项圈', from_holder: '门店前台', to_holder: '火化组', signed_by: '李火化师' } })
  check('移交签收登记成功', tr.status === 200)
  const afterTr = await api(`/staff/commissions/${cid}`, { token: staff })
  check('移交后遗物保管人已更新为接收方', afterTr.data.relics[0]?.holder === '火化组')

  // ---------- 4. 服务未完成时禁止交付 ----------
  console.log('▶ 4. 服务未完成时交付被拦截')
  const early = await api('/deliveries', {
    method: 'POST', token: staff,
    body: { commission_code: code, urn_seal_no: `URN-${suffix}`, receiver_type: 'customer', receiver_name: '测试客户' },
  })
  check('返回 409 + SERVICE_NOT_COMPLETED', early.status === 409 && early.data.error?.code === 'SERVICE_NOT_COMPLETED', JSON.stringify(early.data))

  await api(`/staff/commissions/${cid}/complete-service`, { method: 'POST', token: staff })

  // ---------- 5. 错误封签被拦截 ----------
  console.log('▶ 5. 错误封罐号交付被拦截')
  const badSeal = await api('/deliveries', {
    method: 'POST', token: staff,
    body: { commission_code: code, urn_seal_no: 'URN-WRONG-999', receiver_type: 'customer', receiver_name: '测试客户' },
  })
  check('返回 409 + SEAL_MISMATCH', badSeal.status === 409 && badSeal.data.error?.code === 'SEAL_MISMATCH', JSON.stringify(badSeal.data))
  check('错误信息说明封签不一致', (badSeal.data.error?.message || '').includes('封签核对不一致'))

  // ---------- 6. 撤销代领后被拦截 ----------
  console.log('▶ 6. 撤销代领授权后交付被拦截')
  const px = await api(`/staff/commissions/${cid}/proxies`, { method: 'POST', token: staff, body: { name: '代领人甲', id_no: '310101199001010000', phone: '13700000000' } })
  const proxyId = px.data.id
  await api(`/staff/commissions/${cid}/proxies/${proxyId}/revoke`, { method: 'POST', token: staff })
  const revoked = await api('/deliveries', {
    method: 'POST', token: staff,
    body: { commission_code: code, urn_seal_no: `URN-${suffix}`, receiver_type: 'proxy', receiver_name: '代领人甲' },
  })
  check('返回 409 + PROXY_REVOKED', revoked.status === 409 && revoked.data.error?.code === 'PROXY_REVOKED', JSON.stringify(revoked.data))
  check('错误信息说明授权已撤销', (revoked.data.error?.message || '').includes('代领授权已撤销'))

  const noProxy = await api('/deliveries', {
    method: 'POST', token: staff,
    body: { commission_code: code, urn_seal_no: `URN-${suffix}`, receiver_type: 'proxy', receiver_name: '陌生人乙' },
  })
  check('无授权代领人返回 PROXY_NOT_FOUND', noProxy.status === 409 && noProxy.data.error?.code === 'PROXY_NOT_FOUND')

  // ---------- 7. 并发交付：只有一单成功 ----------
  console.log('▶ 7. 两名员工同时交付，仅一单成功')
  const [r1, r2] = await Promise.all([
    api('/deliveries', { method: 'POST', token: staff, body: { commission_code: code, urn_seal_no: `URN-${suffix}`, receiver_type: 'customer', receiver_name: '测试客户' } }),
    api('/deliveries', { method: 'POST', token: staff, body: { commission_code: code, urn_seal_no: `URN-${suffix}`, receiver_type: 'customer', receiver_name: '测试客户' } }),
  ])
  const oks = [r1, r2].filter((r) => r.status === 200).length
  const conflicts = [r1, r2].filter((r) => r.status === 409 && r.data.error?.code === 'ALREADY_DELIVERED').length
  check('恰好一单交付成功', oks === 1, `成功数=${oks}`)
  check('另一单被 ALREADY_DELIVERED 拦截', conflicts === 1, `拦截数=${conflicts}`)

  const third = await api('/deliveries', {
    method: 'POST', token: staff,
    body: { commission_code: code, urn_seal_no: `URN-${suffix}`, receiver_type: 'customer', receiver_name: '测试客户' },
  })
  check('交付后再次申请出库被拦截（重复出库）', third.status === 409 && third.data.error?.code === 'ALREADY_DELIVERED')

  const finalDetail = await api(`/staff/commissions/${cid}`, { token: staff })
  check('服务状态=已完成 且 交付状态=已交付（两个状态独立）',
    finalDetail.data.commission.service_status === 'completed' && finalDetail.data.commission.delivery_status === 'delivered')
  check('交付记录已生成（经办人/领取人/封罐号齐全）',
    !!finalDetail.data.delivery && finalDetail.data.delivery.delivered_by === '王店员' && finalDetail.data.delivery.urn_seal_no === `URN-${suffix}`)

  // ---------- 8. 客户权限：只能看自己的委托 ----------
  console.log('▶ 8. 客户数据隔离')
  const custToken = await login(CUSTOMER, '123456')
  const mine = await api('/my/commissions', { token: custToken })
  check('新客户能看到自己的委托', mine.status === 200 && mine.data.commissions.some((c) => c.id === cid))
  const mineDetail = await api(`/my/commissions/${cid}`, { token: custToken })
  check('客户可查看本人委托的历史服务清单', mineDetail.status === 200 && mineDetail.data.versions.length === 2)
  const stranger = await api(`/my/commissions/${cid}`, { token: chen })
  check('其他客户访问被 403 拒绝', stranger.status === 403, `status=${stranger.status}`)
  const staffOnly = await api('/staff/commissions', { token: custToken })
  check('客户访问员工接口被 403 拒绝', staffOnly.status === 403)

  // ---------- 汇总 ----------
  console.log(`\n=== 结果：${passed} 项通过，${failed} 项失败 ===\n`)
  process.exit(failed ? 1 : 0)
}

main().catch((e) => { console.error('验收脚本异常:', e); process.exit(1) })
