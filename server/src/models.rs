use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub role: String,
    pub name: String,
    pub phone: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Commission {
    pub id: i64,
    pub code: String,
    pub customer_id: i64,
    pub pet_name: String,
    pub pet_species: String,
    pub pickup_address: String,
    pub pickup_time: String,
    pub pickup_contact: String,
    pub pickup_notes: String,
    pub urn_seal_no: String,
    pub service_status: String,
    pub delivery_status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServiceItem {
    pub id: i64,
    pub commission_id: i64,
    pub name: String,
    pub price_cents: i64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ServiceVersion {
    pub id: i64,
    pub commission_id: i64,
    pub version_no: i64,
    pub confirmed_by: String,
    pub confirmed_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ServiceVersionItem {
    pub id: i64,
    pub version_id: i64,
    pub name: String,
    pub price_cents: i64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Relic {
    pub id: i64,
    pub commission_id: i64,
    pub name: String,
    pub description: String,
    pub photo_path: String,
    pub holder: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Seal {
    pub id: i64,
    pub commission_id: i64,
    pub seal_no: String,
    pub item_desc: String,
    pub sealed_by: String,
    pub sealed_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Transfer {
    pub id: i64,
    pub commission_id: i64,
    pub item_label: String,
    pub from_holder: String,
    pub to_holder: String,
    pub signed_by: String,
    pub signed_at: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Proxy {
    pub id: i64,
    pub commission_id: i64,
    pub name: String,
    pub id_no: String,
    pub phone: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Delivery {
    pub id: i64,
    pub commission_id: i64,
    pub receiver_name: String,
    pub receiver_type: String,
    pub receiver_id_no: String,
    pub urn_seal_no: String,
    pub delivered_by: String,
    pub delivered_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Notification {
    pub id: i64,
    pub commission_id: Option<i64>,
    pub recipient: String,
    pub content: String,
    pub sent_at: String,
}
