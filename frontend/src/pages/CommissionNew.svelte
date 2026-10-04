<script>
  import { api } from '../api.js';

  let form = {
    customer_username: '',
    customer_name: '',
    customer_phone: '',
    customer_id_no: '',
    pet_name: '',
    pet_species: '',
    pickup_address: '',
    pickup_contact: '',
    pickup_phone: '',
    pickup_time: '',
    pickup_notes: '',
  };
  let error = '';
  let loading = false;

  async function submit() {
    error = '';
    loading = true;
    try {
      const res = await api('/commissions', { method: 'POST', body: form });
      location.hash = '#/commissions/' + res.commission.id;
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }
</script>

<div class="card">
  <h2>新建委托单 <span class="sub">登记客户与接运信息；客户账号不存在时将自动创建（初始密码 123456）</span></h2>
  {#if error}<div class="alert error">{error}</div>{/if}
  <form onsubmit={(e) => { e.preventDefault(); submit(); }}>
    <div class="form-grid">
      <label class="field">客户账号 *
        <input bind:value={form.customer_username} required placeholder="如 chen" />
      </label>
      <label class="field">客户姓名 *
        <input bind:value={form.customer_name} required />
      </label>
      <label class="field">客户电话
        <input bind:value={form.customer_phone} />
      </label>
      <label class="field">客户证件号（交付时核对）
        <input bind:value={form.customer_id_no} />
      </label>
      <label class="field">宠物名字 *
        <input bind:value={form.pet_name} required />
      </label>
      <label class="field">宠物种类
        <input bind:value={form.pet_species} placeholder="如 金毛犬 / 英短猫" />
      </label>
      <label class="field">接运地址
        <input bind:value={form.pickup_address} />
      </label>
      <label class="field">接运联系人
        <input bind:value={form.pickup_contact} />
      </label>
      <label class="field">接运联系电话
        <input bind:value={form.pickup_phone} />
      </label>
      <label class="field">预约接运时间
        <input bind:value={form.pickup_time} placeholder="2026-10-05 14:00" />
      </label>
    </div>
    <label class="field">接运备注
      <textarea rows="2" bind:value={form.pickup_notes}></textarea>
    </label>
    <button disabled={loading}>{loading ? '提交中…' : '创建委托单'}</button>
    <a href="#/" style="margin-left: 12px;">返回列表</a>
  </form>
</div>
