export const SERVICE_STATUS = {
  pending: '待接运',
  picked_up: '已接运',
  in_service: '服务中',
  completed: '服务已完成',
};
export const SERVICE_NEXT = {
  pending: 'picked_up',
  picked_up: 'in_service',
  in_service: 'completed',
};
export const DELIVERY_STATUS = { none: '未交付', delivered: '已交付' };
export const ITEM_TYPE = { relic: '遗物', urn: '骨灰罐' };
export const ITEM_STATUS = { in_store: '在店保管', in_transit: '移交中', delivered: '已交付' };
export const TRANSFER_TYPE = {
  pickup: '接运',
  intake: '入店',
  service: '服务流转',
  storage: '封存',
  delivery: '交付',
};

export function fenToYuan(cents) {
  return '¥' + (cents / 100).toFixed(2);
}
