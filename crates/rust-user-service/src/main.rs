use bladb_server::{
    create_http_server_module_transport, create_server_module_launcher,
    CreateHttpServerModuleTransportOptions, CreateServerModuleLauncherOptions, ServerModuleHandler,
    ServerModuleInvocation, StaticServerModuleRegistry,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    env,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const DEFAULT_SESSION_LEASE_SECONDS: u64 = 60 * 60 * 24 * 30;

#[derive(Clone)]
struct AppState {
    users: Arc<Mutex<HashMap<String, StoredUser>>>,
    sessions: Arc<Mutex<HashMap<String, StoredSession>>>,
    next_user_id: Arc<Mutex<u64>>,
    next_session_id: Arc<Mutex<u64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredUser {
    app: String,
    uid: String,
    tenant_id: String,
    email: String,
    password: String,
    display_name: String,
    roles: Vec<String>,
    anonymous: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSession {
    token: String,
    user: StoredUser,
    session_kind: String,
    issued_at: u64,
    last_seen_at: u64,
    expires_at: u64,
}

fn main() {
    let host = env::var("BLADB_SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = env::var("BLADB_SERVER_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8791);

    let state = seeded_state();
    let health_state = state.clone();
    let login_state = state.clone();
    let register_state = state.clone();
    let me_state = state.clone();
    let logout_state = state.clone();
    let anonymous_state = state.clone();

    let registry = Arc::new(
        StaticServerModuleRegistry::new()
            .register_handler(
                "user",
                "health",
                Arc::new(move |_| health(&health_state)) as ServerModuleHandler,
            )
            .register_handler(
                "user",
                "login",
                Arc::new(move |invocation| login(&login_state, invocation)) as ServerModuleHandler,
            )
            .register_handler(
                "user",
                "register",
                Arc::new(move |invocation| register(&register_state, invocation))
                    as ServerModuleHandler,
            )
            .register_handler(
                "user",
                "me",
                Arc::new(move |invocation| me(&me_state, invocation)) as ServerModuleHandler,
            )
            .register_handler(
                "user",
                "logout",
                Arc::new(move |invocation| logout(&logout_state, invocation))
                    as ServerModuleHandler,
            )
            .register_handler(
                "user",
                "ensureAnonymousSession",
                Arc::new(move |invocation| ensure_anonymous_session(&anonymous_state, invocation))
                    as ServerModuleHandler,
            ),
    );

    let apps = supported_apps();
    let transport = create_http_server_module_transport(CreateHttpServerModuleTransportOptions {
        host: host.clone(),
        port,
    })
    .unwrap_or_else(|error| panic!("failed to start rust user service transport: {error}"));
    let mut subjects = Vec::new();
    for app in &apps {
        let launcher = create_server_module_launcher(CreateServerModuleLauncherOptions {
            app: app.clone(),
            transport: Arc::new(transport.clone()),
            registry: registry.clone(),
            modules: vec!["user".into()],
        });
        let started_subjects = launcher.start().unwrap_or_else(|error| {
            panic!("failed to register rust user service subjects: {error}")
        });
        subjects.extend(started_subjects);
    }

    println!(
        "Rust user service listening on {} for apps: {}",
        transport.base_url(),
        apps.join(", ")
    );
    subjects.sort();
    for subject in &subjects {
        println!("- {subject}");
    }

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn seeded_state() -> AppState {
    let mut users = HashMap::new();
    for user in seeded_users() {
        users.insert(user_key(&user.app, &user.email), user);
    }

    AppState {
        users: Arc::new(Mutex::new(users)),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        next_user_id: Arc::new(Mutex::new(6000)),
        next_session_id: Arc::new(Mutex::new(1)),
    }
}

fn health(_: &AppState) -> Result<Value, String> {
    Ok(json!({ "ok": true, "service": "rust-user-service" }))
}

fn login(state: &AppState, invocation: ServerModuleInvocation) -> Result<Value, String> {
    let input = invocation
        .input
        .ok_or_else(|| "missing login input".to_string())?;
    let app = normalize_app(
        input
            .get("app")
            .and_then(Value::as_str)
            .unwrap_or(invocation.app.as_str()),
    )?;
    let email = input
        .get("email")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing email".to_string())?
        .trim()
        .to_ascii_lowercase();
    let password = input
        .get("password")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing password".to_string())?;

    let user = state
        .users
        .lock()
        .map_err(|_| "users lock poisoned".to_string())?
        .get(&user_key(&app, &email))
        .cloned()
        .ok_or_else(|| "invalid email or password".to_string())?;
    if user.password != password {
        return Err("invalid email or password".into());
    }

    let session = issue_session(state, user, "authenticated")?;
    Ok(public_session(session))
}

fn register(state: &AppState, invocation: ServerModuleInvocation) -> Result<Value, String> {
    let input = invocation
        .input
        .ok_or_else(|| "missing register input".to_string())?;
    let app = normalize_app(
        input
            .get("app")
            .and_then(Value::as_str)
            .unwrap_or(invocation.app.as_str()),
    )?;
    let email = input
        .get("email")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing email".to_string())?
        .trim()
        .to_ascii_lowercase();
    let password = input
        .get("password")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing password".to_string())?;
    let display_name = input
        .get("displayName")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing displayName".to_string())?
        .trim()
        .to_string();

    if email.is_empty() || password.trim().len() < 6 || display_name.is_empty() {
        return Err(
            "register requires non-empty email, displayName, and a password with at least 6 characters"
                .to_string(),
        );
    }

    let mut users = state
        .users
        .lock()
        .map_err(|_| "users lock poisoned".to_string())?;
    let key = user_key(&app, &email);
    if users.contains_key(&key) {
        return Err("user already exists".into());
    }

    let (tenant_id, roles) = default_identity_for_app(&users, &app);
    let mut next_user_id = state
        .next_user_id
        .lock()
        .map_err(|_| "user counter lock poisoned".to_string())?;
    let user = StoredUser {
        app: app.clone(),
        uid: format!("u_{}", *next_user_id),
        tenant_id,
        email,
        password: password.to_string(),
        display_name,
        roles,
        anonymous: false,
    };
    *next_user_id += 1;
    users.insert(key, user.clone());
    drop(users);

    let session = issue_session(state, user, "authenticated")?;
    Ok(public_session(session))
}

fn me(state: &AppState, invocation: ServerModuleInvocation) -> Result<Value, String> {
    let token = read_session_token(&invocation, "me")?;
    let expected_app = normalized_invocation_app(&invocation)?;
    let session = resolve_session(state, &token, expected_app.as_deref())?;

    Ok(public_session(session))
}

fn logout(state: &AppState, invocation: ServerModuleInvocation) -> Result<Value, String> {
    let token = read_session_token(&invocation, "logout")?;
    let expected_app = normalized_invocation_app(&invocation)?;
    let _session = resolve_session(state, &token, expected_app.as_deref())?;

    let removed = state
        .sessions
        .lock()
        .map_err(|_| "sessions lock poisoned".to_string())?
        .remove(&token)
        .is_some();
    if !removed {
        return Err("session expired or token is invalid".into());
    }

    Ok(json!({ "revoked": true }))
}

fn ensure_anonymous_session(
    state: &AppState,
    invocation: ServerModuleInvocation,
) -> Result<Value, String> {
    let app = normalized_invocation_app(&invocation)?
        .ok_or_else(|| "missing app for anonymous session".to_string())?;
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| "sessions lock poisoned".to_string())?;
    if let Some(existing) = read_cookie_session(&sessions, &app, &invocation)? {
        return Ok(public_session(existing));
    }
    drop(sessions);

    let users = state
        .users
        .lock()
        .map_err(|_| "users lock poisoned".to_string())?;
    let (tenant_id, roles) = default_identity_for_app(&users, &app);
    drop(users);

    let mut next_user_id = state
        .next_user_id
        .lock()
        .map_err(|_| "user counter lock poisoned".to_string())?;
    let user_number = *next_user_id;
    *next_user_id += 1;
    drop(next_user_id);

    let uid = format!("anon_{}_{}", app.replace('-', "_"), user_number);
    let user = StoredUser {
        app: app.clone(),
        uid: uid.clone(),
        tenant_id,
        email: anonymous_email(&app, &uid),
        password: String::new(),
        display_name: anonymous_display_name(&app, user_number),
        roles,
        anonymous: true,
    };
    let session = issue_session(state, user, "anonymous")?;
    Ok(public_session(session))
}

fn public_session(session: StoredSession) -> Value {
    json!({
        "token": session.token,
        "sessionKind": session.session_kind,
        "anonymous": session.session_kind == "anonymous",
        "issuedAt": session.issued_at,
        "lastSeenAt": session.last_seen_at,
        "expiresAt": session.expires_at,
        "user": {
            "app": session.user.app,
            "uid": session.user.uid,
            "tenantId": session.user.tenant_id,
            "email": session.user.email,
            "displayName": session.user.display_name,
            "roles": session.user.roles,
            "anonymous": session.user.anonymous
        }
    })
}

fn supported_apps() -> Vec<String> {
    vec![
        "flash-sale".into(),
        "blog".into(),
        "iot-realtime".into(),
        "ros2-bridge".into(),
        "user-module-demo".into(),
        "rust-user-demo".into(),
    ]
}

fn seeded_users() -> Vec<StoredUser> {
    vec![
        StoredUser {
            app: "flash-sale".into(),
            uid: "u_2001".into(),
            tenant_id: "tenant_flashsale".into(),
            email: "buyer@flash-sale.demo".into(),
            password: "demo123".into(),
            display_name: "Flash Buyer".into(),
            roles: vec!["buyer".into()],
            anonymous: false,
        },
        StoredUser {
            app: "blog".into(),
            uid: "u_5001".into(),
            tenant_id: "tenant_blog".into(),
            email: "editor@blog.demo".into(),
            password: "demo123".into(),
            display_name: "Blog Editor".into(),
            roles: vec!["editor".into()],
            anonymous: false,
        },
        StoredUser {
            app: "blog".into(),
            uid: "u_5002".into(),
            tenant_id: "tenant_blog".into(),
            email: "guest@blog.demo".into(),
            password: "demo123".into(),
            display_name: "Guest Writer".into(),
            roles: vec!["editor".into()],
            anonymous: false,
        },
        StoredUser {
            app: "iot-realtime".into(),
            uid: "u_1001".into(),
            tenant_id: "tenant_a".into(),
            email: "operator@iot.demo".into(),
            password: "demo123".into(),
            display_name: "IoT Operator".into(),
            roles: vec!["operator".into()],
            anonymous: false,
        },
        StoredUser {
            app: "ros2-bridge".into(),
            uid: "u_3001".into(),
            tenant_id: "tenant_robotics".into(),
            email: "operator@ros2.demo".into(),
            password: "demo123".into(),
            display_name: "Robot Operator".into(),
            roles: vec!["operator".into()],
            anonymous: false,
        },
        StoredUser {
            app: "user-module-demo".into(),
            uid: "u_4001".into(),
            tenant_id: "tenant_local".into(),
            email: "member@user.demo".into(),
            password: "demo123".into(),
            display_name: "Demo Member".into(),
            roles: vec!["member".into()],
            anonymous: false,
        },
        StoredUser {
            app: "rust-user-demo".into(),
            uid: "u_rust_1001".into(),
            tenant_id: "tenant_rust".into(),
            email: "member@rust.demo".into(),
            password: "demo123".into(),
            display_name: "Rust Demo Member".into(),
            roles: vec!["member".into()],
            anonymous: false,
        },
    ]
}

fn issue_session(
    state: &AppState,
    user: StoredUser,
    session_kind: &str,
) -> Result<StoredSession, String> {
    let now = now_epoch_seconds();
    let mut next = state
        .next_session_id
        .lock()
        .map_err(|_| "session counter lock poisoned".to_string())?;
    let token_prefix = if session_kind == "anonymous" {
        "anon"
    } else {
        "session"
    };
    let token = format!("{token_prefix}-{}-{}", user.app, *next);
    *next += 1;
    drop(next);

    let session = StoredSession {
        token: token.clone(),
        user,
        session_kind: session_kind.to_string(),
        issued_at: now,
        last_seen_at: now,
        expires_at: now + DEFAULT_SESSION_LEASE_SECONDS,
    };
    state
        .sessions
        .lock()
        .map_err(|_| "sessions lock poisoned".to_string())?
        .insert(token, session.clone());
    Ok(session)
}

fn resolve_session(
    state: &AppState,
    token: &str,
    expected_app: Option<&str>,
) -> Result<StoredSession, String> {
    let mut sessions = state
        .sessions
        .lock()
        .map_err(|_| "sessions lock poisoned".to_string())?;
    let now = now_epoch_seconds();
    let Some(existing) = sessions.get(token).cloned() else {
        return Err("session expired or token is invalid".into());
    };

    if existing.expires_at <= now {
        sessions.remove(token);
        return Err("session expired or token is invalid".into());
    }

    if let Some(app) = expected_app {
        if existing.user.app != app {
            return Err("session does not belong to the requested app".into());
        }
    }

    let mut renewed = existing;
    renewed.last_seen_at = now;
    renewed.expires_at = now + DEFAULT_SESSION_LEASE_SECONDS;
    sessions.insert(token.to_string(), renewed.clone());
    Ok(renewed)
}

fn read_session_token(invocation: &ServerModuleInvocation, action: &str) -> Result<String, String> {
    if let Some(token) = invocation
        .db
        .as_ref()
        .and_then(|value| value.get("user"))
        .and_then(|value| value.get(action))
        .and_then(|value| value.get("token"))
        .and_then(Value::as_str)
    {
        return sanitize_session_token(token);
    }

    if action == "me" {
        if let Some(cookie_session) = read_cookie_session_token(invocation)? {
            return Ok(cookie_session);
        }
    }

    Err("missing bearer token".into())
}

fn read_cookie_session_token(
    invocation: &ServerModuleInvocation,
) -> Result<Option<String>, String> {
    let db = match invocation.db.as_ref() {
        Some(value) => value,
        None => return Ok(None),
    };
    let cookie_token = db
        .get("user")
        .and_then(|value| value.get("cookie"))
        .and_then(|value| value.get("token"))
        .and_then(Value::as_str);
    match cookie_token {
        Some(token) => sanitize_cookie_token(token).map(Some),
        None => Ok(None),
    }
}

fn read_cookie_session(
    sessions: &HashMap<String, StoredSession>,
    app: &str,
    invocation: &ServerModuleInvocation,
) -> Result<Option<StoredSession>, String> {
    let Some(token) = read_cookie_session_token(invocation)? else {
        return Ok(None);
    };
    let Some(session) = sessions.get(&token).cloned() else {
        return Ok(None);
    };
    if session.user.app != app {
        return Err("session does not belong to the requested app".into());
    }
    Ok(Some(session))
}

fn normalized_invocation_app(
    invocation: &ServerModuleInvocation,
) -> Result<Option<String>, String> {
    let input_app = invocation
        .input
        .as_ref()
        .and_then(|value| value.get("app"))
        .and_then(Value::as_str);
    let candidate = input_app
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(invocation.app.as_str());
    Ok(Some(normalize_app(candidate)?))
}

fn default_identity_for_app(
    users: &HashMap<String, StoredUser>,
    app: &str,
) -> (String, Vec<String>) {
    users
        .values()
        .find(|user| user.app == app)
        .map(|user| (user.tenant_id.clone(), user.roles.clone()))
        .unwrap_or_else(|| ("tenant_local".into(), vec!["member".into()]))
}

fn user_key(app: &str, email: &str) -> String {
    format!("{app}:{}", email.trim().to_ascii_lowercase())
}

fn normalize_app(app: &str) -> Result<String, String> {
    let normalized = app.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err("app is required".into());
    }
    Ok(normalized)
}

fn sanitize_session_token(value: &str) -> Result<String, String> {
    let token = value
        .trim()
        .strip_prefix("Bearer ")
        .or_else(|| value.trim().strip_prefix("bearer "))
        .unwrap_or_else(|| value.trim())
        .trim();
    if token.is_empty() {
        return Err("missing bearer token".into());
    }
    Ok(token.to_string())
}

fn sanitize_cookie_token(value: &str) -> Result<String, String> {
    let token = value.trim();
    if token.is_empty() {
        return Err("missing session cookie".into());
    }
    Ok(token.to_string())
}

fn anonymous_display_name(app: &str, user_id: u64) -> String {
    format!("Anonymous {} {}", app.replace('-', " "), user_id)
}

fn anonymous_email(app: &str, uid: &str) -> String {
    format!("anon+{uid}@{app}.local")
}

fn now_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
