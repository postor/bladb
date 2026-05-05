use super::{
    auth::AuthUser, now_label, value_as_i64, value_as_string, AppApiHandler, AppApiRequest,
    AppError,
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
    stock: i64,
    wallets: HashMap<String, i64>,
    orders: Vec<FlashSaleOrderConfig>,
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
        Self {
            state: Arc::new(Mutex::new(FlashSaleState {
                item: config.item,
                stock: config.stock,
                wallets: config.wallets,
                orders: config.orders,
                tickets: HashMap::new(),
                next_ticket_id: 1,
            })),
            queue: config.queue,
        }
    }

    pub(crate) fn enqueue_purchase(
        &self,
        user: &AuthUser,
        sku: &str,
        quantity: i64,
    ) -> Result<Value, AppError> {
        if sku.trim().is_empty() || quantity <= 0 {
            return Err(AppError::invalid_request(
                "queue purchase requires sku and quantity > 0",
            ));
        }

        let (ticket_id, position) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| AppError::internal("flash-sale state lock poisoned"))?;
            let sequence = state.next_ticket_id;
            let ticket_id = format!("ticket_{sequence:04}");
            state.next_ticket_id += 1;
            let position = state
                .tickets
                .values()
                .filter(|ticket| {
                    matches!(ticket.status, QueueStatus::Queued | QueueStatus::Processing)
                })
                .count() as u64
                + 1;

            let now = now_label();
            state.tickets.insert(
                ticket_id.clone(),
                QueueTicket {
                    id: ticket_id.clone(),
                    uid: user.uid.clone(),
                    sku: sku.into(),
                    quantity,
                    sequence,
                    status: QueueStatus::Queued,
                    order_id: None,
                    message: "Waiting for reservation worker".into(),
                    created_at: now.clone(),
                    updated_at: now,
                },
            );

            (ticket_id, position)
        };

        let state = Arc::clone(&self.state);
        let queue = self.queue.clone();
        let spawned_ticket_id = ticket_id.clone();
        let user_uid = user.uid.clone();
        thread::spawn(move || {
            process_ticket(
                state,
                queue,
                spawned_ticket_id.as_str(),
                &user_uid,
                position,
            );
        });

        self.ticket_details(&user.uid, &ticket_id)
    }

    pub(crate) fn ticket_details(&self, uid: &str, ticket_id: &str) -> Result<Value, AppError> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("flash-sale state lock poisoned"))?;
        let ticket = state
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

    pub(crate) fn summary(&self, uid: &str) -> Result<Value, AppError> {
        let state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("flash-sale state lock poisoned"))?;
        Ok(render_summary(uid, &state))
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
                Ok(json!(state.stock))
            }
            "flashsale.stock.decr" => {
                let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
                let amount = body.amount.unwrap_or(1);
                state.stock = (state.stock - amount).max(0);
                Ok(json!(state.stock))
            }
            "flashsale.wallet.read" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let key = body
                    .name
                    .as_ref()
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("wallet key is missing"))?;
                Ok(json!(state.wallets.get(key).copied().unwrap_or(0)))
            }
            "flashsale.orders.read-mine" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let uid = value_as_string(body.values.first(), "uid")?;
                let sku = value_as_string(body.values.get(1), "sku")?;
                let limit = value_as_i64(body.values.get(2), "limit").unwrap_or(10) as usize;

                let rows: Vec<Value> = state
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
                let next_index = state.orders.len() + 1;

                state.orders.insert(
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
        let session = request
            .session
            .ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
        if session.user.app != "flash-sale" {
            return Err(AppError::unauthorized(
                "flash-sale queue requires a flash-sale session",
            ));
        }

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
                self.enqueue_purchase(&session.user, sku, quantity)
            }
            ("GET", "/apps/flash-sale/summary") => self.summary(&session.user.uid),
            ("GET", "/apps/flash-sale/queue") => Ok(self.list_tickets(&session.user.uid)),
            ("GET", path) if path.starts_with("/apps/flash-sale/queue/") => {
                let ticket_id = path.trim_start_matches("/apps/flash-sale/queue/");
                self.ticket_details(&session.user.uid, ticket_id)
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
        "updatedAt": ticket.updated_at
    })
}

fn render_summary(uid: &str, state: &FlashSaleState) -> Value {
    let orders: Vec<Value> = state
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

    json!({
        "item": {
            "id": state.item.id,
            "sku": state.item.sku,
            "title": state.item.title,
            "price": state.item.price,
            "startsAt": state.item.starts_at
        },
        "stock": state.stock,
        "wallet": state.wallets.get(&format!("{uid}_wallet")).copied().unwrap_or(0),
        "orders": orders
    })
}

impl QueueTicket {
    fn current_position(&self, state: &FlashSaleState) -> Option<u64> {
        match self.status {
            QueueStatus::Completed | QueueStatus::Failed => None,
            QueueStatus::Processing => Some(1),
            QueueStatus::Queued => Some(
                state
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

fn default_queue_position_delay_ms() -> u64 {
    350
}

fn default_queue_processing_delay_ms() -> u64 {
    900
}

fn default_flash_sale_sku() -> String {
    "camera-pro".into()
}

fn process_ticket(
    state: Arc<Mutex<FlashSaleState>>,
    queue: FlashSaleQueueConfig,
    ticket_id: &str,
    uid: &str,
    initial_position: u64,
) {
    let queue_delay =
        Duration::from_millis(initial_position.saturating_sub(1) * queue.position_delay_ms);
    thread::sleep(queue_delay);

    {
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        if let Some(ticket) = state.tickets.get_mut(ticket_id) {
            ticket.status = QueueStatus::Processing;
            ticket.message = "Reserving stock and creating order".into();
            ticket.updated_at = now_label();
        } else {
            return;
        }
    }

    thread::sleep(Duration::from_millis(queue.processing_delay_ms));

    let mut state = match state.lock() {
        Ok(state) => state,
        Err(_) => return,
    };
    let Some((quantity, sku)) = state
        .tickets
        .get(ticket_id)
        .map(|ticket| (ticket.quantity, ticket.sku.clone()))
    else {
        return;
    };

    let next_index = state.orders.len() + 1;
    let result = if state.stock >= quantity {
        state.stock -= quantity;
        let order_id = format!("ord_{next_index:04}");
        state.orders.insert(
            0,
            FlashSaleOrderConfig {
                id: order_id.clone(),
                uid: uid.into(),
                sku,
                status: "queued-confirmed".into(),
                quantity,
                created_at: now_label(),
            },
        );
        Ok(order_id)
    } else {
        Err("Sold out before your turn reached the head of the queue".to_string())
    };

    if let Some(ticket) = state.tickets.get_mut(ticket_id) {
        ticket.updated_at = now_label();
        match result {
            Ok(order_id) => {
                ticket.status = QueueStatus::Completed;
                ticket.order_id = Some(order_id);
                ticket.message = "Reservation completed and order inserted".into();
            }
            Err(message) => {
                ticket.status = QueueStatus::Failed;
                ticket.message = message;
            }
        }
    }
}
