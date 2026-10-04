use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Commission {
    pub id: i64,
    pub commission_no: String,
    pub customer_id: i64,
    pub pet_name: String,
    pub pet_species: String,
    pub pickup_address: String,
    pub pickup_contact: String,
    pub pickup_phone: String,
    pub pickup_time: String,
    pub pickup_staff_id: Option<i64>,
    pub pickup_notes: String,
    pub service_status: String,
    pub delivery_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ServiceItem {
    pub id: i64,
    pub commission_id: i64,
    pub name: String,
    pub price_cents: i64,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ServiceListVersion {
    pub id: i64,
    pub commission_id: i64,
    pub version_no: i64,
    pub items_json: String,
    pub total_cents: i64,
    pub confirmed_by: i64,
    pub confirmed_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Item {
    pub id: i64,
    pub commission_id: i64,
    pub item_type: String, // relic 遗物 | urn 骨灰罐
    pub name: String,
    pub description: String,
    pub seal_no: String,
    pub photo_path: String,
    pub status: String, // in_store | in_transit | delivered
    pub current_holder_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Transfer {
    pub id: i64,
    pub commission_id: i64,
    pub item_id: i64,
    pub from_holder: String,
    pub to_holder_id: Option<i64>,
    pub to_holder_name: String,
    pub transfer_type: String,
    pub status: String, // pending | signed
    pub signed_by: Option<i64>,
    pub signed_at: Option<String>,
    pub notes: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProxyAuth {
    pub id: i64,
    pub commission_id: i64,
    pub proxy_name: String,
    pub proxy_id_no: String,
    pub proxy_phone: String,
    pub status: String, // active | revoked
    pub created_by: i64,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OutboundRequest {
    pub id: i64,
    pub commission_id: i64,
    pub requested_by: i64,
    pub status: String, // open | fulfilled | cancelled
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Delivery {
    pub id: i64,
    pub commission_id: i64,
    pub outbound_request_id: Option<i64>,
    pub recipient_type: String, // self | proxy
    pub recipient_name: String,
    pub recipient_id_no: String,
    pub proxy_auth_id: Option<i64>,
    pub seal_checks_json: String,
    pub delivered_by: i64,
    pub delivered_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Notification {
    pub id: i64,
    pub commission_id: Option<i64>,
    pub recipient: String,
    pub channel: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

// ---------- 请求 DTO ----------

#[derive(Debug, Deserialize)]
pub struct CreateCommissionReq {
    pub customer_username: String,
    pub customer_name: String,
    pub customer_phone: Option<String>,
    pub customer_id_no: Option<String>,
    pub pet_name: String,
    pub pet_species: Option<String>,
    pub pickup_address: Option<String>,
    pub pickup_contact: Option<String>,
    pub pickup_phone: Option<String>,
    pub pickup_time: Option<String>,
    pub pickup_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePickupReq {
    pub pickup_address: String,
    pub pickup_contact: String,
    pub pickup_phone: String,
    pub pickup_time: String,
    pub pickup_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusReq {
    pub service_status: String,
}

#[derive(Debug, Deserialize)]
pub struct AddServiceItemReq {
    pub name: String,
    pub price_cents: Option<i64>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddItemReq {
    pub item_type: String, // relic | urn
    pub name: String,
    pub description: Option<String>,
    pub seal_no: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTransferReq {
    pub to_holder_id: i64,
    pub transfer_type: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProxyReq {
    pub proxy_name: String,
    pub proxy_id_no: String,
    pub proxy_phone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SealCheck {
    pub item_id: i64,
    pub seal_no: String,
}

#[derive(Debug, Deserialize)]
pub struct DeliverReq {
    pub commission_no: String, // 员工现场输入委托号复核
    pub outbound_request_id: Option<i64>,
    pub recipient_type: String, // self | proxy
    pub recipient_name: String,
    pub recipient_id_no: String,
    pub proxy_auth_id: Option<i64>,
    pub seal_checks: Vec<SealCheck>,
}
