//! 集成测试：验证交付守卫与历史清单查询
//! 每个测试使用独立的临时数据库与独立服务实例，互不干扰。

use pet_memorial::{build_router, init_pool, seed, AppState};
use serde_json::{json, Value};
use tempfile::TempDir;

struct TestApp {
    base: String,
    client: reqwest::Client,
    _tmp: TempDir,
}

async fn setup() -> TestApp {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test.db").to_string_lossy().to_string();
    let photo_dir = tmp.path().join("photos");
    std::fs::create_dir_all(&photo_dir).unwrap();

    let pool = init_pool(&db_path).await.unwrap();
    seed::seed(&pool).await.unwrap();
    let state = AppState { pool, photo_dir };
    let app = build_router(state, tmp.path().join("dist"));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    TestApp {
        base: format!("http://127.0.0.1:{port}"),
        client: reqwest::Client::new(),
        _tmp: tmp,
    }
}

impl TestApp {
    async fn login(&self, username: &str, password: &str) -> String {
        let res = self
            .client
            .post(format!("{}/api/auth/login", self.base))
            .json(&json!({"username": username, "password": password}))
            .send()
            .await
            .unwrap();
        let status = res.status();
        assert_eq!(status, 200, "登录失败: {}", res.text().await.unwrap_or_default());
        let body: Value = res.json().await.unwrap();
        body["token"].as_str().unwrap().to_string()
    }

    async fn post(&self, path: &str, token: &str, body: &Value) -> (u16, Value) {
        let res = self
            .client
            .post(format!("{}/api{}", self.base, path))
            .bearer_auth(token)
            .json(body)
            .send()
            .await
            .unwrap();
        let status = res.status().as_u16();
        let json: Value = res.json().await.unwrap_or(Value::Null);
        (status, json)
    }

    async fn get(&self, path: &str, token: &str) -> (u16, Value) {
        let res = self
            .client
            .get(format!("{}/api{}", self.base, path))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        let status = res.status().as_u16();
        let json: Value = res.json().await.unwrap_or(Value::Null);
        (status, json)
    }

    /// 建立一条「服务已完成、含遗物+骨灰罐」的委托，返回 (id, 委托号, [(物品id, 封签号)])
    async fn new_ready_commission(&self, staff: &str, tag: &str) -> (i64, String, Vec<(i64, String)>) {
        let (s, body) = self
            .post(
                "/commissions",
                staff,
                &json!({
                    "customer_username": "chen",
                    "customer_name": "陈静",
                    "pet_name": format!("测试宠物{tag}"),
                    "pet_species": "猫",
                }),
            )
            .await;
        assert_eq!(s, 200, "创建委托失败: {body}");
        let id = body["commission"]["id"].as_i64().unwrap();
        let no = body["commission"]["commission_no"].as_str().unwrap().to_string();

        let (s, _) = self
            .post(
                &format!("/commissions/{id}/service-items"),
                staff,
                &json!({"name": "单独火化", "price_cents": 120000}),
            )
            .await;
        assert_eq!(s, 200);

        let seals = vec![
            (format!("FS-{tag}-A"), "relic", "项圈"),
            (format!("FG-{tag}-A"), "urn", "骨灰罐"),
        ];
        let mut items = Vec::new();
        for (seal, ty, name) in &seals {
            let (s, body) = self
                .post(
                    &format!("/commissions/{id}/items"),
                    staff,
                    &json!({"item_type": ty, "name": name, "seal_no": seal}),
                )
                .await;
            assert_eq!(s, 200, "登记物品失败: {body}");
            items.push((body["item"]["id"].as_i64().unwrap(), seal.clone()));
        }

        for st in ["picked_up", "in_service", "completed"] {
            let (s, body) = self
                .post(
                    &format!("/commissions/{id}/status"),
                    staff,
                    &json!({"service_status": st}),
                )
                .await;
            assert_eq!(s, 200, "推进状态失败: {body}");
        }
        (id, no, items)
    }

    fn self_deliver_body(no: &str, items: &[(i64, String)]) -> Value {
        json!({
            "commission_no": no,
            "recipient_type": "self",
            "recipient_name": "陈静",
            "recipient_id_no": "110101199001011234",
            "seal_checks": items.iter().map(|(id, seal)| json!({"item_id": id, "seal_no": seal})).collect::<Vec<_>>(),
        })
    }
}

// ---------- 1. 错误封签必须被拦截 ----------

#[tokio::test]
async fn wrong_seal_blocks_delivery() {
    let app = setup().await;
    let staff = app.login("zhang", "staff123").await;
    let (id, no, items) = app.new_ready_commission(&staff, "WS1").await;

    // 骨灰罐封罐号故意填错
    let mut body = TestApp::self_deliver_body(&no, &items);
    body["seal_checks"][1]["seal_no"] = json!("FG-WRONG-999");

    let (status, resp) = app
        .post(&format!("/commissions/{id}/deliver"), &staff, &body)
        .await;
    assert_eq!(status, 409, "错误封签应返回 409: {resp}");
    let msg = resp["error"].as_str().unwrap();
    assert!(msg.contains("封罐号核对不一致"), "应说明封签不一致原因: {msg}");
    assert!(msg.contains("骨灰罐"), "应指出具体物品: {msg}");

    // 委托仍处于未交付状态
    let (_, detail) = app.get(&format!("/commissions/{id}"), &staff).await;
    assert_eq!(detail["commission"]["delivery_status"], "none");
}

// ---------- 2. 撤销代领授权后必须拦截交付 ----------

#[tokio::test]
async fn revoked_proxy_blocks_delivery() {
    let app = setup().await;
    let staff = app.login("zhang", "staff123").await;
    let customer = app.login("chen", "cust123").await;
    let (id, no, items) = app.new_ready_commission(&staff, "RP1").await;

    // 客户登记代领授权
    let (s, body) = app
        .post(
            &format!("/commissions/{id}/proxy"),
            &customer,
            &json!({"proxy_name": "李雷", "proxy_id_no": "ID-LILEI-001", "proxy_phone": "13711111111"}),
        )
        .await;
    assert_eq!(s, 200);
    let proxy_id = body["proxy"]["id"].as_i64().unwrap();

    // 客户撤销授权
    let (s, _) = app.post(&format!("/proxy/{proxy_id}/revoke"), &customer, &json!({})).await;
    assert_eq!(s, 200);

    // 员工尝试按已撤销的代领授权交付 → 必须拦截
    let body = json!({
        "commission_no": no,
        "recipient_type": "proxy",
        "recipient_name": "李雷",
        "recipient_id_no": "ID-LILEI-001",
        "proxy_auth_id": proxy_id,
        "seal_checks": items.iter().map(|(i, s)| json!({"item_id": i, "seal_no": s})).collect::<Vec<_>>(),
    });
    let (status, resp) = app
        .post(&format!("/commissions/{id}/deliver"), &staff, &body)
        .await;
    assert_eq!(status, 403, "已撤销代领应返回 403: {resp}");
    let msg = resp["error"].as_str().unwrap();
    assert!(msg.contains("撤销"), "应说明授权已撤销: {msg}");

    let (_, detail) = app.get(&format!("/commissions/{id}"), &staff).await;
    assert_eq!(detail["commission"]["delivery_status"], "none");
}

// ---------- 3. 同一委托重复申请出库必须被拦截 ----------

#[tokio::test]
async fn duplicate_outbound_blocked() {
    let app = setup().await;
    let staff = app.login("zhang", "staff123").await;
    let (id, _no, _items) = app.new_ready_commission(&staff, "DO1").await;

    let (s, _) = app
        .post(&format!("/commissions/{id}/outbound-requests"), &staff, &json!({}))
        .await;
    assert_eq!(s, 200, "首次出库申请应成功");

    let (status, resp) = app
        .post(&format!("/commissions/{id}/outbound-requests"), &staff, &json!({}))
        .await;
    assert_eq!(status, 409, "重复出库应返回 409: {resp}");
    let msg = resp["error"].as_str().unwrap();
    assert!(msg.contains("不能重复申请出库"), "应说明重复出库原因: {msg}");

    // 服务未完成的委托也不能申请出库
    let (s, body) = app
        .post(
            "/commissions",
            &staff,
            &json!({"customer_username": "chen", "customer_name": "陈静", "pet_name": "未完成"}),
        )
        .await;
    assert_eq!(s, 200);
    let id2 = body["commission"]["id"].as_i64().unwrap();
    let (status, resp) = app
        .post(&format!("/commissions/{id2}/outbound-requests"), &staff, &json!({}))
        .await;
    assert_eq!(status, 409);
    assert!(resp["error"].as_str().unwrap().contains("服务尚未完成"));
}

// ---------- 4. 重复交付与并发交付：只能成功一次 ----------

#[tokio::test]
async fn duplicate_delivery_blocked() {
    let app = setup().await;
    let staff = app.login("zhang", "staff123").await;
    let (id, no, items) = app.new_ready_commission(&staff, "DD1").await;

    let body = TestApp::self_deliver_body(&no, &items);
    let (s, resp) = app.post(&format!("/commissions/{id}/deliver"), &staff, &body).await;
    assert_eq!(s, 200, "首次交付应成功: {resp}");

    let (status, resp) = app.post(&format!("/commissions/{id}/deliver"), &staff, &body).await;
    assert_eq!(status, 409, "重复交付应返回 409: {resp}");
    assert!(resp["error"].as_str().unwrap().contains("不能重复交付"));
}

#[tokio::test]
async fn concurrent_delivery_only_one_succeeds() {
    let app = setup().await;
    let staff_a = app.login("zhang", "staff123").await;
    let staff_b = app.login("liu", "staff123").await;
    let (id, no, items) = app.new_ready_commission(&staff_a, "CC1").await;

    // 两名员工同时提交交付
    let body = TestApp::self_deliver_body(&no, &items);
    let url = format!("{}/api/commissions/{id}/deliver", app.base);
    let (ra, rb) = {
        let ca = app.client.clone();
        let cb = app.client.clone();
        let ta = staff_a.clone();
        let tb = staff_b.clone();
        let ba = body.clone();
        let bb = body.clone();
        let ua = url.clone();
        tokio::join!(
            async move { ca.post(ua).bearer_auth(ta).json(&ba).send().await.unwrap() },
            async move { cb.post(url).bearer_auth(tb).json(&bb).send().await.unwrap() },
        )
    };
    let sa = ra.status().as_u16();
    let sb = rb.status().as_u16();
    let statuses = [sa, sb];
    assert!(
        statuses.contains(&200) && statuses.contains(&409),
        "并发交付必须恰好一次成功一次冲突，实际: {statuses:?}"
    );

    // 数据库中仅有一条交付记录
    let (_, detail) = app.get(&format!("/commissions/{id}/delivery"), &staff_a).await;
    assert!(detail["delivery"].is_object(), "应存在交付记录");
    let (_, c) = app.get(&format!("/commissions/{id}"), &staff_a).await;
    assert_eq!(c["commission"]["delivery_status"], "delivered");
}

// ---------- 5. 历史服务清单版本可查询且不被覆盖 ----------

#[tokio::test]
async fn service_list_versions_preserved() {
    let app = setup().await;
    let staff = app.login("zhang", "staff123").await;
    let customer = app.login("chen", "cust123").await;

    let (s, body) = app
        .post(
            "/commissions",
            &staff,
            &json!({"customer_username": "chen", "customer_name": "陈静", "pet_name": "版本测试"}),
        )
        .await;
    assert_eq!(s, 200);
    let id = body["commission"]["id"].as_i64().unwrap();

    // 初始两个项目，客户确认 → v1
    app.post(&format!("/commissions/{id}/service-items"), &staff, &json!({"name": "遗体接运", "price_cents": 30000})).await;
    app.post(&format!("/commissions/{id}/service-items"), &staff, &json!({"name": "单独火化", "price_cents": 120000})).await;
    let (s, body) = app
        .post(&format!("/commissions/{id}/service-list/confirm"), &customer, &json!({}))
        .await;
    assert_eq!(s, 200);
    assert_eq!(body["version"]["version_no"], 1);
    assert_eq!(body["version"]["total_cents"], 150000);

    // 门店后续变更工作清单：删除一项、新增一项
    let (_, items) = app.get(&format!("/commissions/{id}/service-items"), &staff).await;
    let first_id = items[0]["id"].as_i64().unwrap();
    let res = app
        .client
        .delete(format!("{}/api/service-items/{first_id}", app.base))
        .bearer_auth(&staff)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    app.post(&format!("/commissions/{id}/service-items"), &staff, &json!({"name": "告别仪式升级", "price_cents": 50000})).await;

    // 客户再次确认 → v2
    let (s, body) = app
        .post(&format!("/commissions/{id}/service-list/confirm"), &customer, &json!({}))
        .await;
    assert_eq!(s, 200);
    assert_eq!(body["version"]["version_no"], 2);

    // 历史版本仍可查询，且 v1 快照未被后续变更覆盖
    let (s, versions) = app
        .get(&format!("/commissions/{id}/service-list/versions"), &customer)
        .await;
    assert_eq!(s, 200);
    assert_eq!(versions.as_array().unwrap().len(), 2);
    let v1_items: Vec<Value> = serde_json::from_str(versions[0]["items_json"].as_str().unwrap()).unwrap();
    assert_eq!(v1_items.len(), 2, "v1 应保留最初的两个项目");
    assert_eq!(v1_items[0]["name"], "遗体接运");
    assert_eq!(versions[0]["total_cents"], 150000);
    let v2_items: Vec<Value> = serde_json::from_str(versions[1]["items_json"].as_str().unwrap()).unwrap();
    assert_eq!(v2_items.len(), 2, "v2 反映变更后的清单");
    assert!(v2_items.iter().any(|i| i["name"] == "告别仪式升级"));
}

// ---------- 6. 客户只能查看自己的委托 ----------

#[tokio::test]
async fn customer_isolation() {
    let app = setup().await;
    let staff = app.login("zhang", "staff123").await;
    let chen = app.login("chen", "cust123").await;
    let wang = app.login("wang", "cust123").await;

    let (id, _no, _items) = app.new_ready_commission(&staff, "CI1").await;

    // wang 查看 chen 的委托 → 拒绝
    let (status, _) = app.get(&format!("/commissions/{id}"), &wang).await;
    assert_eq!(status, 403, "客户不能查看他人委托");

    // wang 的列表为空，chen 的列表包含该委托
    let (_, list_wang) = app.get("/commissions", &wang).await;
    assert_eq!(list_wang.as_array().unwrap().len(), 0);
    let (_, list_chen) = app.get("/commissions", &chen).await;
    assert!(list_chen.as_array().unwrap().iter().any(|c| c["commission"]["id"] == id));

    // 客户不能执行员工操作
    let (status, _) = app
        .post(&format!("/commissions/{id}/status"), &chen, &json!({"service_status": "completed"}))
        .await;
    assert_eq!(status, 403);
}

// ---------- 7. 移交必须由接收人本人签收 ----------

#[tokio::test]
async fn transfer_requires_receiver_signature() {
    let app = setup().await;
    let zhang = app.login("zhang", "staff123").await;
    let liu = app.login("liu", "staff123").await;
    let (id, _no, items) = app.new_ready_commission(&zhang, "TR1").await;
    let item_id = items[0].0;

    // 张雯把物品移交给刘洋
    let liu_id = {
        let (_, staff_list) = app.get("/staff", &zhang).await;
        staff_list
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["username"] == "liu")
            .unwrap()["id"]
            .as_i64()
            .unwrap()
    };
    let (s, body) = app
        .post(
            &format!("/items/{item_id}/transfers"),
            &zhang,
            &json!({"to_holder_id": liu_id, "transfer_type": "service", "notes": "火化后转交"}),
        )
        .await;
    assert_eq!(s, 200);
    let transfer_id = body["transfer"]["id"].as_i64().unwrap();

    // 张雯（非接收人）不能代签
    let (status, _) = app
        .post(&format!("/transfers/{transfer_id}/sign"), &zhang, &json!({}))
        .await;
    assert_eq!(status, 403);

    // 刘洋本人签收 → 保管人变更
    let (s, _) = app
        .post(&format!("/transfers/{transfer_id}/sign"), &liu, &json!({}))
        .await;
    assert_eq!(s, 200);
    let (_, item_list) = app.get(&format!("/commissions/{id}/items"), &zhang).await;
    let item = &item_list.as_array().unwrap()[0];
    assert_eq!(item["holder_name"], "刘洋");
    assert_eq!(item["item"]["status"], "in_store");
}
