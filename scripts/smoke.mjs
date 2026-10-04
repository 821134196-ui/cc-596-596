// 端到端冒烟测试：走完真实业务流程并验证拦截规则
const BASE = 'http://localhost:8080/api';
let passed = 0, failed = 0;

function check(name, cond, extra = '') {
  if (cond) { passed++; console.log(`  ✓ ${name}`); }
  else { failed++; console.log(`  ✗ ${name} ${extra}`); }
}

async function call(method, path, { token, body } = {}) {
  const res = await fetch(BASE + path, {
    method,
    headers: {
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(body ? { 'Content-Type': 'application/json' } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  return { status: res.status, data: await res.json().catch(() => ({})) };
}

const login = async (u, p) =>
  (await call('POST', '/auth/login', { body: { username: u, password: p } })).data.token;

const zhang = await login('zhang', 'staff123');
const liu = await login('liu', 'staff123');
const chen = await login('chen', 'cust123');
const wang = await login('wang', 'cust123');
console.log('✓ 四个演示账号登录成功');

// 1. 员工创建委托
let r = await call('POST', '/commissions', {
  token: zhang,
  body: {
    customer_username: 'chen', customer_name: '陈静', pet_name: '咪咪', pet_species: '英短猫',
    pickup_address: '海淀区知春路 1 号', pickup_contact: '陈静', pickup_phone: '13800000001',
    pickup_time: '2026-10-04 15:00',
  },
});
const cid = r.data.commission.id, cno = r.data.commission.commission_no;
check('创建委托单', r.status === 200 && cno.startsWith('WT'));

// 2. 服务项目 + 客户确认 v1 → 门店变更 → 客户确认 v2
await call('POST', `/commissions/${cid}/service-items`, { token: zhang, body: { name: '遗体接运', price_cents: 30000 } });
await call('POST', `/commissions/${cid}/service-items`, { token: zhang, body: { name: '单独火化', price_cents: 120000 } });
r = await call('POST', `/commissions/${cid}/service-list/confirm`, { token: chen });
check('客户确认服务清单 v1', r.status === 200 && r.data.version.version_no === 1);
const items1 = (await call('GET', `/commissions/${cid}/service-items`, { token: zhang })).data;
await call('DELETE', `/service-items/${items1[0].id}`, { token: zhang });
await call('POST', `/commissions/${cid}/service-items`, { token: zhang, body: { name: '树葬服务', price_cents: 200000 } });
r = await call('POST', `/commissions/${cid}/service-list/confirm`, { token: chen });
check('变更后客户确认 v2', r.status === 200 && r.data.version.version_no === 2);
r = await call('GET', `/commissions/${cid}/service-list/versions`, { token: chen });
const v1 = JSON.parse(r.data[0].items_json);
check('历史版本 v1 仍可查询且未被覆盖', r.data.length === 2 && v1.length === 2 && v1[0].name === '遗体接运');

// 3. 登记遗物与骨灰罐
r = await call('POST', `/commissions/${cid}/items`, { token: zhang, body: { item_type: 'relic', name: '猫抓板', seal_no: 'FS-SMOKE-1' } });
const relicId = r.data.item.id;
r = await call('POST', `/commissions/${cid}/items`, { token: zhang, body: { item_type: 'urn', name: '骨灰罐', seal_no: 'FG-SMOKE-1' } });
const urnId = r.data.item.id;
check('登记遗物与骨灰罐', !!relicId && !!urnId);

// 4. 移交：张雯 → 刘洋，刘洋本人签收
const staffList = (await call('GET', '/staff', { token: zhang })).data;
const liuId = staffList.find((s) => s.username === 'liu').id;
r = await call('POST', `/items/${relicId}/transfers`, { token: zhang, body: { to_holder_id: liuId, transfer_type: 'service', notes: '移交火化组' } });
const tid = r.data.transfer.id;
let bad = await call('POST', `/transfers/${tid}/sign`, { token: zhang });
r = await call('POST', `/transfers/${tid}/sign`, { token: liu });
check('移交须接收人本人签收', bad.status === 403 && r.status === 200);

// 5. 推进服务状态至完成
for (const st of ['picked_up', 'in_service', 'completed'])
  r = await call('POST', `/commissions/${cid}/status`, { token: zhang, body: { service_status: st } });
check('服务状态推进至「服务已完成」', r.status === 200 && r.data.commission.service_status === 'completed');

// 6. 出库申请 + 重复申请拦截
r = await call('POST', `/commissions/${cid}/outbound-requests`, { token: zhang });
const dup = await call('POST', `/commissions/${cid}/outbound-requests`, { token: liu });
check('重复出库申请被拦截并说明原因', r.status === 200 && dup.status === 409 && dup.data.error.includes('不能重复申请出库'));

// 7. 错误封签交付被拦截
const deliverBody = (seals, extra = {}) => ({
  commission_no: cno, recipient_type: 'self', recipient_name: '陈静',
  recipient_id_no: '110101199001011234',
  seal_checks: [{ item_id: relicId, seal_no: seals[0] }, { item_id: urnId, seal_no: seals[1] }],
  ...extra,
});
r = await call('POST', `/commissions/${cid}/deliver`, { token: zhang, body: deliverBody(['FS-SMOKE-1', 'FG-错误封罐号']) });
check('错误封罐号被拦截', r.status === 409 && r.data.error.includes('封罐号核对不一致'), r.data.error);

// 8. 撤销代领后交付被拦截
r = await call('POST', `/commissions/${cid}/proxy`, { token: chen, body: { proxy_name: '韩梅', proxy_id_no: 'ID-HANMEI-9' } });
const proxyId = r.data.proxy.id;
await call('POST', `/proxy/${proxyId}/revoke`, { token: chen });
r = await call('POST', `/commissions/${cid}/deliver`, {
  token: zhang,
  body: deliverBody(['FS-SMOKE-1', 'FG-SMOKE-1'], { recipient_type: 'proxy', recipient_name: '韩梅', recipient_id_no: 'ID-HANMEI-9', proxy_auth_id: proxyId }),
});
check('已撤销代领授权被拦截', r.status === 403 && r.data.error.includes('撤销'), r.data.error);

// 9. 并发交付：两名员工同时操作，仅一次成功
const [ra, rb] = await Promise.all([
  call('POST', `/commissions/${cid}/deliver`, { token: zhang, body: deliverBody(['FS-SMOKE-1', 'FG-SMOKE-1']) }),
  call('POST', `/commissions/${cid}/deliver`, { token: liu, body: deliverBody(['FS-SMOKE-1', 'FG-SMOKE-1']) }),
]);
const codes = [ra.status, rb.status].sort();
check('并发交付恰好一次成功', codes[0] === 200 && codes[1] === 409, JSON.stringify(codes));

// 10. 状态核对：服务已完成 ≠ 物品已交付（现已交付）
r = await call('GET', `/commissions/${cid}`, { token: zhang });
check('交付后状态：服务已完成 + 物品已交付',
  r.data.commission.service_status === 'completed' && r.data.commission.delivery_status === 'delivered');
r = await call('GET', `/commissions/${cid}/delivery`, { token: zhang });
check('交付记录含封签核对留档', JSON.parse(r.data.delivery.seal_checks_json).length === 2);

// 11. 客户隔离
r = await call('GET', `/commissions/${cid}`, { token: wang });
check('其他客户无法查看该委托', r.status === 403);

// 12. 通知留档
r = await call('GET', `/commissions/${cid}/notifications`, { token: chen });
check('模拟通知已留档', r.data.length >= 2);

console.log(`\n结果：${passed} 通过，${failed} 失败`);
process.exit(failed ? 1 : 0);
