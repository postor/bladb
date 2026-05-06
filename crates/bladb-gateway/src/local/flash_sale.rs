use super::{
    auth::AuthSession,
    now_label, value_as_i64, value_as_string, AppApiHandler, AppApiRequest, AppError,
};
use crate::{ExecutionContext, ModuleRuntime, RuntimeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

pub struct FlashSaleModule {
    state: Arc<Mutex<FlashSaleState>>,
    queue: FlashSaleQueueConfig,
    allow_anonymous_app_access: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashSaleModuleConfig {
    pub item: FlashSaleItemConfig,
    pub stock: i64,
    #[serde(default)]
    pub wallets: HashMap<String, i64>,
    #[serde(default)]
    pub orders: Vec<FlashSaleOrderConfig>,
    #[serde(default)]
    pub queue: FlashSaleQueueConfig,
    #[serde(default)]
    pub allow_anonymous_app_access: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashSaleItemConfig {
    pub id: String,
    #[serde(default = "default_flash_sale_sku")]
    pub sku: String,
    pub title: String,
    pub price: i64,
    pub starts_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashSaleOrderConfig {
    pub id: String,
    pub uid: String,
    pub sku: String,
    pub status: String,
    pub quantity: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashSaleQueueConfig {
    #[serde(default = "default_queue_position_delay_ms")]
    pub position_delay_ms: u64,
    #[serde(default = "default_queue_processing_delay_ms")]
    pub processing_delay_ms: u64,
}

struct FlashSaleState {
    item: FlashSaleItemConfig,
    redis: FlashSaleRedisState,
    db: FlashSaleDbState,
    worker: FlashSaleWorkerState,
    default_wallet_balance: i64,
}

struct FlashSaleRedisState {
    stock: i64,
    wallets: HashMap<String, i64>,
}

struct FlashSaleDbState {
    orders: Vec<FlashSaleOrderConfig>,
}

struct FlashSaleWorkerState {
    tickets: HashMap<String, QueueTicket>,
    next_ticket_id: u64,
}

#[derive(Clone)]
struct QueueTicket {
    id: String,
    uid: String,
    sku: String,
    quantity: i64,
    sequence: u64,
    status: QueueStatus,
    order_id: Option<String>,
    message: String,
    created_at: String,
    updated_at: String,
    steps: Vec<CollaborationStep>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollaborationStep {
    role: String,
    action: String,
    detail: String,
    at: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QueueStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

impl Default for FlashSaleModuleConfig {
    fn default() -> Self {
        Self {
            item: FlashSaleItemConfig {
                id: "item_001".into(),
                sku: default_flash_sale_sku(),
                title: "Camera Pro".into(),
                price: 499,
                starts_at: "2026-05-04T20:00:00Z".into(),
            },
            stock: 420,
            wallets: HashMap::from([("u_2001_wallet".into(), 1280)]),
            orders: vec![
                FlashSaleOrderConfig {
                    id: "ord_0001".into(),
                    uid: "u_2001".into(),
                    sku: "camera-pro".into(),
                    status: "paid".into(),
                    quantity: 1,
                    created_at: "2026-05-04T18:42:00Z".into(),
                },
                FlashSaleOrderConfig {
                    id: "ord_0002".into(),
                    uid: "u_2001".into(),
                    sku: "camera-pro".into(),
                    status: "pending".into(),
                    quantity: 1,
                    created_at: "2026-05-04T18:55:00Z".into(),
                },
            ],
            queue: FlashSaleQueueConfig::default(),
            allow_anonymous_app_access: true,
        }
    }
}

impl Default for FlashSaleQueueConfig {
    fn default() -> Self {
        Self {
            position_delay_ms: default_queue_position_delay_ms(),
            processing_delay_ms: default_queue_processing_delay_ms(),
        }
    }
}

impl FlashSaleModule {
    pub fn new() -> Self {
        Self::from_config(FlashSaleModuleConfig::default())
    }

    pub fn from_config(config: FlashSaleModuleConfig) -> Self {
        let default_wallet_balance = config
            .wallets
            .values()
            .copied()
            .max()
            .unwrap_or(1280);

        Self {
            state: Arc::new(Mutex::new(FlashSaleState {
                item: config.item,
                redis: FlashSaleRedisState {
                    stock: config.stock,
                    wallets: config.wallets,
                },
                db: FlashSaleDbState {
                    orders: config.orders,
                },
                worker: FlashSaleWorkerState {
                    tickets: HashMap::new(),
                    next_ticket_id: 1,
                },
                default_wallet_balance,
            })),
            queue: config.queue,
            allow_anonymous_app_access: config.allow_anonymous_app_access,
        }
    }

    pub(crate) fn enqueue_purchase(
        &self,
        session: &AuthSession,
        sku: &str,
        quantity: i64,
    ) -> Result<Value, AppError> {
        if sku.trim().is_empty() || quantity <= 0 {
            return Err(AppError::invalid_request(
                "queue purchase requires sku and quantity > 0",
            ));
        }

        let uid = session.user.uid.clone();
        let (ticket_id, position) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| AppError::internal("flash-sale state lock poisoned"))?;
            let sequence = state.worker.next_ticket_id;
            let ticket_id = format!("ticket_{sequence:04}");
            state.worker.next_ticket_id += 1;
            ensure_wallet_balance(&mut state, &uid);
            let position = state
                .worker
                .tickets
                .values()
                .filter(|ticket| {
                    matches!(ticket.status, QueueStatus::Queued | QueueStatus::Processing)
                })
                .count() as u64
                + 1;

            let now = now_label();
            state.worker.tickets.insert(
                ticket_id.clone(),
                QueueTicket {
                    id: ticket_id.clone(),
                    uid: uid.clone(),
                    sku: sku.into(),
                    quantity,
                    sequence,
                    status: QueueStatus::Queued,
                    order_id: None,
                    message: "Queued for worker reservation".into(),
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    steps: vec![step(
                        "worker",
                        "queue",
                        format!("Accepted queue request for {uid}"),
                        now,
                    )],
                },
            );

            (ticket_id, position)
        };

        let state = Arc::clone(&self.state);
        let queue = self.queue.clone();
        let spawned_ticket_id = ticket_id.clone();
        thread::spawn(move || {
            process_ticket(state, queue, &spawned_ticket_id, position);
        });

        self.ticket_details(&uid, &ticket_id)
    }

    pub(crate) fn ticket_details(&self, uid: &str, ticket_id: &str) -> Result<Value, AppError> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("flash-sale state lock poisoned"))?;
        let ticket = state
            .worker
            .tickets
            .get(ticket_id)
            .ok_or_else(|| AppError::invalid_request("queue ticket not found"))?;
        if ticket.uid != uid {
            return Err(AppError::unauthorized(
                "queue ticket does not belong to current user",
            ));
        }

        Ok(render_ticket(ticket, &state))
    }

    pub(crate) fn list_tickets(&self, uid: &str) -> Value {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return Value::Array(vec![]),
        };
        let mut tickets = state
            .worker
            .tickets
            .values()
            .filter(|ticket| ticket.uid == uid)
            .cloned()
            .collect::<Vec<_>>();
        tickets.sort_by(|left, right| right.sequence.cmp(&left.sequence));
        Value::Array(
            tickets
                .iter()
                .map(|ticket| render_ticket(ticket, &state))
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn summary(&self, session: &AuthSession) -> Result<Value, AppError> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("flash-sale state lock poisoned"))?;
        Ok(render_summary(session, &state))
    }

    fn require_session<'a>(&self, request: &'a AppApiRequest) -> Result<&'a AuthSession, AppError> {
        let session = request.session.as_ref().ok_or_else(|| {
            if self.allow_anonymous_app_access {
                AppError::internal(
                    "flash-sale anonymous identity was not resolved by the gateway",
                )
            } else {
                AppError::unauthorized("missing bearer token")
            }
        })?;

        if session.user.app != "flash-sale" {
            return Err(AppError::unauthorized(
                "flash-sale queue requires a flash-sale session",
            ));
        }

        Ok(session)
    }
}

impl ModuleRuntime for FlashSaleModule {
    fn handles_cluster(&self, cluster: &str) -> bool {
        cluster.starts_with("flashsale.")
    }

    fn execute(&self, context: &ExecutionContext) -> Result<Value, RuntimeError> {
        let policy = context.policy_name();
        let body = &context.routed.body;
        match policy {
            "flashsale.items.find" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                Ok(json!({
                    "id": state.item.id,
                    "title": state.item.title,
                    "price": state.item.price,
                    "startsAt": state.item.starts_at
                }))
            }
            "flashsale.stock.read" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                Ok(json!(state.redis.stock))
            }
            "flashsale.stock.decr" => {
                let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
                let amount = body.amount.unwrap_or(1);
                state.redis.stock = (state.redis.stock - amount).max(0);
                Ok(json!(state.redis.stock))
            }
            "flashsale.wallet.read" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let key = body
                    .name
                    .as_ref()
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("wallet key is missing"))?;
                Ok(json!(state.redis.wallets.get(key).copied().unwrap_or(0)))
            }
            "flashsale.orders.read-mine" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let uid = value_as_string(body.values.first(), "uid")?;
                let sku = value_as_string(body.values.get(1), "sku")?;
                let limit = value_as_i64(body.values.get(2), "limit").unwrap_or(10) as usize;

                let rows: Vec<Value> = state
                    .db
                    .orders
                    .iter()
                    .filter(|order| order.uid == uid && order.sku == sku)
                    .take(limit)
                    .map(|order| {
                        json!({
                            "id": order.id,
                            "status": order.status,
                            "quantity": order.quantity,
                            "createdAt": order.created_at
                        })
                    })
                    .collect();

                Ok(Value::Array(rows))
            }
            "flashsale.orders.create" => {
                let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
                let uid = value_as_string(body.values.first(), "uid")?;
                let sku = value_as_string(body.values.get(1), "sku")?;
                let quantity = value_as_i64(body.values.get(2), "quantity")?;
                let status = value_as_string(body.values.get(3), "status")?;
                let next_index = state.db.orders.len() + 1;

                state.db.orders.insert(
                    0,
                    FlashSaleOrderConfig {
                        id: format!("ord_{next_index:04}"),
                        uid,
                        sku,
                        status,
                        quantity,
                        created_at: now_label(),
                    },
                );

                Ok(json!({ "created": true }))
            }
            _ => Err(RuntimeError::internal(format!(
                "unsupported flash-sale policy `{policy}`"
            ))),
        }
    }
}

impl AppApiHandler for FlashSaleModule {
    fn can_handle(&self, method: &str, path: &str) -> bool {
        (matches!(method, "GET" | "POST") && path.starts_with("/apps/flash-sale/queue"))
            || (method == "GET" && path == "/apps/flash-sale/summary")
    }

    fn handle(&self, request: AppApiRequest) -> Result<Value, AppError> {
        let session = self.require_session(&request)?.clone();
        let uid = session.user.uid.as_str();

        match (request.method.as_str(), request.path.as_str()) {
            ("POST", "/apps/flash-sale/queue") => {
                let payload = request
                    .body
                    .ok_or_else(|| AppError::invalid_request("request body is required"))?;
                let sku = payload
                    .get("sku")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::invalid_request("missing or invalid field `sku`"))?;
                let quantity =
                    payload
                        .get("quantity")
                        .and_then(Value::as_i64)
                        .ok_or_else(|| {
                            AppError::invalid_request("missing or invalid field `quantity`")
                        })?;
                self.enqueue_purchase(&session, sku, quantity)
            }
            ("GET", "/apps/flash-sale/summary") => self.summary(&session),
            ("GET", "/apps/flash-sale/queue") => Ok(self.list_tickets(uid)),
            ("GET", path) if path.starts_with("/apps/flash-sale/queue/") => {
                let ticket_id = path.trim_start_matches("/apps/flash-sale/queue/");
                self.ticket_details(uid, ticket_id)
            }
            _ => Err(AppError::not_found("route not found")),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn render_ticket(ticket: &QueueTicket, state: &FlashSaleState) -> Value {
    json!({
        "ticketId": ticket.id,
        "sku": ticket.sku,
        "quantity": ticket.quantity,
        "status": ticket.status.as_str(),
        "queuePosition": ticket.current_position(state),
        "orderId": ticket.order_id,
        "message": ticket.message,
        "createdAt": ticket.created_at,
        "updatedAt": ticket.updated_at,
        "steps": ticket.steps,
        "runtime": {
            "queueCluster": "flashsale.workflow-workers",
            "redisCluster": "flashsale.stock-redis",
            "dbCluster": "flashsale.orders-sql"
        }
    })
}

fn render_summary(session: &AuthSession, state: &FlashSaleState) -> Value {
    let uid = session.user.uid.as_str();
    let orders: Vec<Value> = state
        .db
        .orders
        .iter()
        .filter(|order| order.uid == uid && order.sku == state.item.sku)
        .map(|order| {
            json!({
                "id": order.id,
                "status": order.status,
                "quantity": order.quantity,
                "createdAt": order.created_at
            })
        })
        .collect();
    let wallet = state
        .redis
        .wallets
        .get(&wallet_key(uid))
        .copied()
        .unwrap_or(state.default_wallet_balance);

    json!({
        "identity": {
            "app": session.user.app,
            "uid": session.user.uid,
            "tenantId": session.user.tenant_id,
            "displayName": session.user.display_name,
            "email": session.user.email,
            "roles": session.user.roles,
            "anonymous": session.user.anonymous,
            "sessionKind": session.kind.as_str(),
        },
        "item": {
            "id": state.item.id,
            "sku": state.item.sku,
            "title": state.item.title,
            "price": state.item.price,
            "startsAt": state.item.starts_at
        },
        "stock": state.redis.stock,
        "wallet": wallet,
        "orders": orders,
        "runtime": {
            "readPath": [
                { "role": "redis", "action": "read-stock", "cluster": "flashsale.stock-redis" },
                { "role": "redis", "action": "read-wallet", "cluster": "flashsale.stock-redis" },
                { "role": "db", "action": "read-orders", "cluster": "flashsale.orders-sql" }
            ],
            "writePath": [
                { "role": "worker", "action": "queue", "cluster": "flashsale.workflow-workers" },
                { "role": "redis", "action": "reserve-stock", "cluster": "flashsale.stock-redis" },
                { "role": "db", "action": "insert-order", "cluster": "flashsale.orders-sql" },
                { "role": "worker", "action": "complete", "cluster": "flashsale.workflow-workers" }
            ]
        }
    })
}

impl QueueTicket {
    fn current_position(&self, state: &FlashSaleState) -> Option<u64> {
        match self.status {
            QueueStatus::Completed | QueueStatus::Failed => None,
            QueueStatus::Processing => Some(1),
            QueueStatus::Queued => Some(
                state
                    .worker
                    .tickets
                    .values()
                    .filter(|ticket| {
                        matches!(ticket.status, QueueStatus::Queued | QueueStatus::Processing)
                            && ticket.sequence <= self.sequence
                    })
                    .count() as u64,
            ),
        }
    }
}

impl QueueStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

fn process_ticket(state: Arc<Mutex<FlashSaleState>>, queue: FlashSaleQueueConfig, ticket_id: &str, initial_position: u64) {
    let queue_delay =
        Duration::from_millis(initial_position.saturating_sub(1) * queue.position_delay_ms);
    thread::sleep(queue_delay);

    {
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        if let Some(ticket) = state.worker.tickets.get_mut(ticket_id) {
            ticket.status = QueueStatus::Processing;
            ticket.message = "Worker is reserving stock and charging the wallet".into();
            ticket.updated_at = now_label();
            ticket.steps.push(step(
                "worker",
                "claim",
                "Worker claimed the queued reservation".into(),
                ticket.updated_at.clone(),
            ));
        } else {
            return;
        }
    }

    thread::sleep(Duration::from_millis(queue.processing_delay_ms));

    let mut state = match state.lock() {
        Ok(state) => state,
        Err(_) => return,
    };
    let Some((uid, quantity, sku)) = state
        .worker
        .tickets
        .get(ticket_id)
        .map(|ticket| (ticket.uid.clone(), ticket.quantity, ticket.sku.clone()))
    else {
        return;
    };

    let wallet = ensure_wallet_balance(&mut state, &uid);
    let price = state.item.price * quantity;
    let now = now_label();
    let next_index = state.db.orders.len() + 1;
    let result = if state.redis.stock < quantity {
        Err("Sold out before your turn reached the head of the queue".to_string())
    } else if wallet < price {
        Err("Wallet balance was too low when the worker attempted settlement".to_string())
    } else {
        state.redis.stock -= quantity;
        if let Some(balance) = state.redis.wallets.get_mut(&wallet_key(&uid)) {
            *balance -= price;
        }

        let order_id = format!("ord_{next_index:04}");
        state.db.orders.insert(
            0,
            FlashSaleOrderConfig {
                id: order_id.clone(),
                uid: uid.clone(),
                sku,
                status: "queued-confirmed".into(),
                quantity,
                created_at: now.clone(),
            },
        );

        Ok(order_id)
    };

    if let Some(ticket) = state.worker.tickets.get_mut(ticket_id) {
        ticket.updated_at = now.clone();
        ticket.steps.push(step(
            "redis",
            "read-wallet",
            format!("Checked wallet for {}", ticket.uid),
            now.clone(),
        ));
        ticket.steps.push(step(
            "redis",
            "read-stock",
            "Checked remaining sale inventory".into(),
            now.clone(),
        ));
        match result {
            Ok(order_id) => {
                ticket.status = QueueStatus::Completed;
                ticket.order_id = Some(order_id);
                ticket.message = "Worker reserved stock, charged wallet, and inserted the order".into();
                ticket.steps.push(step(
                    "redis",
                    "reserve-stock",
                    "Reserved stock in the hot counter".into(),
                    now.clone(),
                ));
                ticket.steps.push(step(
                    "redis",
                    "debit-wallet",
                    "Debited the anonymous buyer wallet".into(),
                    now.clone(),
                ));
                ticket.steps.push(step(
                    "db",
                    "insert-order",
                    "Inserted the final order row".into(),
                    now.clone(),
                ));
                ticket.steps.push(step(
                    "worker",
                    "complete",
                    "Marked the queue ticket as completed".into(),
                    now,
                ));
            }
            Err(message) => {
                ticket.status = QueueStatus::Failed;
                ticket.message = message;
                ticket.steps.push(step(
                    "worker",
                    "fail",
                    "Worker marked the reservation as failed".into(),
                    now,
                ));
            }
        }
    }
}

fn ensure_wallet_balance(state: &mut FlashSaleState, uid: &str) -> i64 {
    let balance = state
        .redis
        .wallets
        .entry(wallet_key(uid))
        .or_insert(state.default_wallet_balance);
    *balance
}

fn wallet_key(uid: &str) -> String {
    format!("{uid}_wallet")
}

fn step(role: &str, action: &str, detail: String, at: String) -> CollaborationStep {
    CollaborationStep {
        role: role.into(),
        action: action.into(),
        detail,
        at,
    }
}

fn default_queue_position_delay_ms() -> u64 {
    350
}

fn default_queue_processing_delay_ms() -> u64 {
    900
}

fn default_flash_sale_sku() -> String {
    "camera-pro".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::{InMemoryAuthService, InMemoryUserConfig};
    use serde_json::json;
    use std::{thread, time::Duration};

    fn seed_user() -> InMemoryUserConfig {
        InMemoryUserConfig {
            app: "flash-sale".into(),
            uid: "u_2001".into(),
            tenant_id: "tenant_flashsale".into(),
            email: "buyer@flash-sale.demo".into(),
            password: "demo123".into(),
            display_name: "Flash Buyer".into(),
            roles: vec!["buyer".into()],
        }
    }

    fn anonymous_session() -> AuthSession {
        InMemoryAuthService::from_user_configs(vec![seed_user()])
            .ensure_anonymous_session("flash-sale")
            .expect("anonymous flash-sale session")
    }

    #[test]
    fn flash_sale_summary_uses_resolved_anonymous_identity() {
        let module = FlashSaleModule::new();
        let session = anonymous_session();
        let response = module
            .handle(AppApiRequest {
                method: "GET".into(),
                path: "/apps/flash-sale/summary".into(),
                body: None,
                session: Some(session.clone()),
            })
            .expect("anonymous summary");

        assert_eq!(response["identity"]["uid"], session.user.uid);
        assert_eq!(response["identity"]["anonymous"], true);
        assert_eq!(response["wallet"], 1280);
        assert_eq!(response["runtime"]["readPath"][0]["role"], "redis");
    }

    #[test]
    fn flash_sale_queue_purchase_tracks_worker_steps() {
        let module = FlashSaleModule::new();
        let session = anonymous_session();
        let ticket = module
            .handle(AppApiRequest {
                method: "POST".into(),
                path: "/apps/flash-sale/queue".into(),
                body: Some(json!({
                    "sku": "camera-pro",
                    "quantity": 1
                })),
                session: Some(session.clone()),
            })
            .expect("anonymous queue");

        let ticket_id = ticket["ticketId"].as_str().expect("ticket id");
        thread::sleep(Duration::from_millis(1600));

        let settled = module
            .handle(AppApiRequest {
                method: "GET".into(),
                path: format!("/apps/flash-sale/queue/{ticket_id}"),
                body: None,
                session: Some(session),
            })
            .expect("anonymous queue ticket");

        assert!(settled["steps"]
            .as_array()
            .is_some_and(|steps| steps.iter().any(|step| step["role"] == "worker")));
        assert!(matches!(
            settled["status"].as_str(),
            Some("completed") | Some("failed")
        ));
    }

    #[test]
    fn queue_tickets_are_scoped_to_the_resolved_identity() {
        let module = FlashSaleModule::new();
        let first_session = anonymous_session();
        let second_session = anonymous_session();
        let ticket = module
            .handle(AppApiRequest {
                method: "POST".into(),
                path: "/apps/flash-sale/queue".into(),
                body: Some(json!({
                    "sku": "camera-pro",
                    "quantity": 1
                })),
                session: Some(first_session.clone()),
            })
            .expect("first queue");
        let ticket_id = ticket["ticketId"].as_str().expect("ticket id");

        let error = module
            .handle(AppApiRequest {
                method: "GET".into(),
                path: format!("/apps/flash-sale/queue/{ticket_id}"),
                body: None,
                session: Some(second_session),
            })
            .expect_err("second anonymous identity should be rejected");

        assert_eq!(error.status, 401);
        assert!(error.message.contains("does not belong"));
    }
}
