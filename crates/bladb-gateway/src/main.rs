use bladb_core::protocol::{GatewayFailure, GatewayRequest, GatewaySuccess, ResponseMeta};
use bladb_gateway::{LocalGatewayApp, LocalGatewayConfig};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    env, fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    process,
    sync::Arc,
    thread,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() || args.first().is_some_and(|arg| arg == "serve") {
        let (addr, config_path) = parse_serve_args(&args[1..])?;
        return serve(addr, config_path.as_deref());
    }

    if args.first().is_some_and(|arg| arg == "prepare") {
        return run_prepare(&args[1..]);
    }

    if args.first().is_some_and(|arg| arg == "route") {
        return run_route(&args[1..]);
    }

    if args.len() >= 2 {
        return run_prepare(&args);
    }

    Err(usage())
}

fn serve(addr: SocketAddr, config_path: Option<&str>) -> Result<(), String> {
    let state = Arc::new(load_gateway_app(config_path)?);
    let listener =
        TcpListener::bind(addr).map_err(|error| format!("failed to bind server: {error}"))?;

    println!("Bladb gateway listening on http://{addr}");

    for stream in listener.incoming() {
        let state = Arc::clone(&state);
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let _ = handle_client(stream, &state);
                });
            }
            Err(error) => eprintln!("accept error: {error}"),
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream, state: &Arc<LocalGatewayApp>) -> Result<(), String> {
    let request = read_http_request(&mut stream)?;
    let bearer_token = request.header("authorization").map(str::to_string);
    if request.method != "OPTIONS" && request.path.starts_with("/apps/") {
        let response = match parse_optional_json_body(request.body.clone()) {
            Ok(body) => match state.handle_app_api(
                &request.method,
                &request.path,
                bearer_token.as_deref(),
                body,
            ) {
                Ok(Some(data)) => Some(http_json(
                    StatusCode::Ok,
                    json!({ "ok": true, "data": data }),
                )),
                Ok(None) => None,
                Err(error) => Some(error_response(error)?),
            },
            Err(error) => Some(http_json(
                StatusCode::BadRequest,
                json!({
                    "ok": false,
                    "code": "INVALID_REQUEST",
                    "message": error,
                    "meta": { "traceId": "dev-trace" }
                }),
            )),
        };
        if let Some(response) = response {
            stream
                .write_all(response.as_bytes())
                .map_err(|error| format!("failed to write response: {error}"))?;
            return Ok(());
        }
    }

    let response = match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health") => http_json(StatusCode::Ok, json!({ "ok": true })),
        ("GET", "/topology") => http_json(
            StatusCode::Ok,
            json!({ "ok": true, "data": state.topology_snapshot() }),
        ),
        ("POST", "/execute") => handle_gateway_request(request.body, |gateway_request| {
            state.handle_execute_for_token(gateway_request, bearer_token.as_deref())
        })?,
        ("POST", "/route") => handle_gateway_request(request.body, |gateway_request| {
            state.inspect_request_for_token(gateway_request, bearer_token.as_deref())
        })?,
        ("POST", "/auth/login") => handle_json_body(request.body, |payload| {
            let app = required_string(&payload, "app")?;
            let email = required_string(&payload, "email")?;
            let password = required_string(&payload, "password")?;
            match state.login(&app, &email, &password) {
                Ok(data) => Ok(http_json(
                    StatusCode::Ok,
                    json!({ "ok": true, "data": data }),
                )),
                Err(error) => error_response(error),
            }
        })?,
        ("POST", "/auth/register") => handle_json_body(request.body, |payload| {
            let app = required_string(&payload, "app")?;
            let email = required_string(&payload, "email")?;
            let password = required_string(&payload, "password")?;
            let display_name = required_string(&payload, "displayName")?;
            match state.register(&app, &email, &password, &display_name) {
                Ok(data) => Ok(http_json(
                    StatusCode::Ok,
                    json!({ "ok": true, "data": data }),
                )),
                Err(error) => error_response(error),
            }
        })?,
        ("GET", "/auth/me") => match bearer_token.as_deref() {
            Some(token) => match state.me(token) {
                Ok(data) => http_json(StatusCode::Ok, json!({ "ok": true, "data": data })),
                Err(error) => error_response(error)?,
            },
            None => http_json(
                StatusCode::Unauthorized,
                json!({
                    "ok": false,
                    "code": "AUTH_EXPIRED",
                    "message": "missing bearer token",
                    "meta": { "traceId": "dev-trace" }
                }),
            ),
        },
        ("OPTIONS", _) => http_empty(StatusCode::NoContent),
        _ => http_json(
            StatusCode::NotFound,
            json!({
                "ok": false,
                "code": "INVALID_REQUEST",
                "message": "route not found",
                "meta": { "traceId": "dev-trace" }
            }),
        ),
    };

    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("failed to write response: {error}"))?;

    Ok(())
}

fn load_gateway_app(config_path: Option<&str>) -> Result<LocalGatewayApp, String> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve workspace root".to_string())?;
    let default_config_path = workspace_root.join("apps/examples/gateway/local-gateway.yaml");
    let resolved_config_path = config_path
        .map(Path::new)
        .map(Path::to_path_buf)
        .or_else(|| env::var("BLADB_GATEWAY_CONFIG").ok().map(Into::into))
        .unwrap_or(default_config_path);
    let app_config = LocalGatewayConfig::from_path(&resolved_config_path)?;

    println!(
        "Loaded gateway config from {}",
        resolved_config_path.display()
    );
    LocalGatewayApp::from_local_config(app_config)
}

fn handle_json_body(
    body: Vec<u8>,
    handler: impl FnOnce(Value) -> Result<String, String>,
) -> Result<String, String> {
    let payload = match serde_json::from_slice::<Value>(&body) {
        Ok(payload) => payload,
        Err(error) => {
            return Ok(http_json(
                StatusCode::BadRequest,
                json!({
                    "ok": false,
                    "code": "INVALID_REQUEST",
                    "message": format!("failed to parse json body: {error}"),
                    "meta": { "traceId": "dev-trace" }
                }),
            ))
        }
    };

    match handler(payload) {
        Ok(response) => Ok(response),
        Err(message) => Ok(http_json(
            StatusCode::BadRequest,
            json!({
                "ok": false,
                "code": "INVALID_REQUEST",
                "message": message,
                "meta": { "traceId": "dev-trace" }
            }),
        )),
    }
}

fn parse_optional_json_body(body: Vec<u8>) -> Result<Option<Value>, String> {
    if body.is_empty() {
        return Ok(None);
    }

    serde_json::from_slice::<Value>(&body)
        .map(Some)
        .map_err(|error| format!("failed to parse json body: {error}"))
}

fn handle_gateway_request(
    body: Vec<u8>,
    handler: impl FnOnce(GatewayRequest) -> Result<Value, bladb_gateway::AppError>,
) -> Result<String, String> {
    match serde_json::from_slice::<GatewayRequest>(&body) {
        Ok(gateway_request) => match handler(gateway_request) {
            Ok(data) => Ok(http_json(
                StatusCode::Ok,
                serde_json::to_value(GatewaySuccess {
                    ok: true,
                    data,
                    meta: ResponseMeta {
                        trace_id: Some("dev-trace".into()),
                        cursor: None,
                    },
                })
                .map_err(|error| error.to_string())?,
            )),
            Err(error) => Ok(http_json(
                StatusCode::from_u16(error.status),
                serde_json::to_value(GatewayFailure {
                    ok: false,
                    code: error.code,
                    message: error.message,
                    meta: ResponseMeta {
                        trace_id: Some("dev-trace".into()),
                        cursor: None,
                    },
                })
                .map_err(|error| error.to_string())?,
            )),
        },
        Err(error) => Ok(http_json(
            StatusCode::BadRequest,
            json!({
                "ok": false,
                "code": "INVALID_REQUEST",
                "message": format!("failed to parse request json: {error}"),
                "meta": { "traceId": "dev-trace" }
            }),
        )),
    }
}

fn required_string(payload: &Value, field: &str) -> Result<String, String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing or invalid field `{field}`"))
}

fn error_response(error: bladb_gateway::AppError) -> Result<String, String> {
    Ok(http_json(
        StatusCode::from_u16(error.status),
        serde_json::to_value(GatewayFailure {
            ok: false,
            code: error.code,
            message: error.message,
            meta: ResponseMeta {
                trace_id: Some("dev-trace".into()),
                cursor: None,
            },
        })
        .map_err(|render_error| render_error.to_string())?,
    ))
}

fn run_prepare(args: &[String]) -> Result<(), String> {
    let policy_path = args.first().ok_or_else(usage)?;
    let request_path = args.get(1).ok_or_else(usage)?;
    let auth_path = args.get(2);

    let policy_yaml = fs::read_to_string(policy_path)
        .map_err(|error| format!("failed to read policy file: {error}"))?;
    let request_json = fs::read_to_string(request_path)
        .map_err(|error| format!("failed to read request file: {error}"))?;

    let gateway = bladb_gateway::Gateway::from_yaml(&policy_yaml)
        .map_err(|error| format!("gateway init failed: {error}"))?;
    let request: GatewayRequest = serde_json::from_str(&request_json)
        .map_err(|error| format!("failed to parse request json: {error}"))?;

    let auth = if let Some(auth_path) = auth_path {
        let auth_json = fs::read_to_string(auth_path)
            .map_err(|error| format!("failed to read auth file: {error}"))?;
        serde_json::from_str::<bladb_gateway::AuthContext>(&auth_json)
            .map_err(|error| format!("failed to parse auth json: {error}"))?
    } else {
        bladb_gateway::AuthContext::default()
    };

    let prepared = gateway
        .prepare(&request, &auth)
        .map_err(|error| format!("gateway prepare failed: {error}"))?;

    let output = serde_json::json!({
        "policy": prepared.authorization.policy_name,
        "body": prepared.body
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|error| format!("failed to render output: {error}"))?
    );

    Ok(())
}

fn run_route(args: &[String]) -> Result<(), String> {
    let policy_path = args.first().ok_or_else(usage)?;
    let topology_path = args.get(1).ok_or_else(usage)?;
    let request_path = args.get(2).ok_or_else(usage)?;
    let auth_path = args.get(3);

    let policy_yaml = fs::read_to_string(policy_path)
        .map_err(|error| format!("failed to read policy file: {error}"))?;
    let topology_yaml = fs::read_to_string(topology_path)
        .map_err(|error| format!("failed to read topology file: {error}"))?;
    let request_json = fs::read_to_string(request_path)
        .map_err(|error| format!("failed to read request file: {error}"))?;

    let gateway = bladb_gateway::Gateway::from_yaml(&policy_yaml)
        .map_err(|error| format!("gateway init failed: {error}"))?;
    let registry = bladb_gateway::ModuleRegistry::from_yaml(&topology_yaml)
        .map_err(|error| format!("topology init failed: {error}"))?;
    let request: GatewayRequest = serde_json::from_str(&request_json)
        .map_err(|error| format!("failed to parse request json: {error}"))?;

    let auth = if let Some(auth_path) = auth_path {
        let auth_json = fs::read_to_string(auth_path)
            .map_err(|error| format!("failed to read auth file: {error}"))?;
        serde_json::from_str::<bladb_gateway::AuthContext>(&auth_json)
            .map_err(|error| format!("failed to parse auth json: {error}"))?
    } else {
        bladb_gateway::AuthContext::default()
    };

    let prepared = gateway
        .prepare(&request, &auth)
        .map_err(|error| format!("gateway prepare failed: {error}"))?;
    let routed = bladb_gateway::route_prepared_request(&registry, &request, prepared, &auth)
        .map_err(|error| format!("gateway route failed: {error}"))?;

    let output = serde_json::json!({
        "policy": routed.authorization.policy_name,
        "route": {
            "cluster": routed.route.cluster,
            "category": routed.route.category,
            "runtime": routed.route.runtime,
            "service": routed.route.service,
            "namespace": routed.route.namespace,
            "routeKey": routed.route.route_key,
            "shard": routed.route.shard,
            "sticky": routed.route.sticky
        },
        "body": routed.body
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|error| format!("failed to render output: {error}"))?
    );

    Ok(())
}
fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut temp = [0_u8; 4096];
    let mut header_end = None;
    let mut content_length = 0_usize;

    loop {
        let bytes_read = stream
            .read(&mut temp)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..bytes_read]);

        if header_end.is_none() {
            if let Some(index) = find_header_end(&buffer) {
                header_end = Some(index);
                let headers = String::from_utf8_lossy(&buffer[..index]);
                content_length = parse_content_length(&headers);
            }
        }

        if let Some(index) = header_end {
            if buffer.len() >= index + 4 + content_length {
                break;
            }
        }
    }

    let header_end = header_end.ok_or_else(|| "malformed http request".to_string())?;
    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "missing path".to_string())?
        .to_string();
    let parsed_headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect::<HashMap<_, _>>();
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    let body = if buffer.len() >= body_end {
        buffer[body_start..body_end].to_vec()
    } else {
        vec![]
    };

    Ok(HttpRequest {
        method,
        path,
        headers: parsed_headers,
        body,
    })
}

fn parse_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn http_json(status: StatusCode, body: Value) -> String {
    let rendered = serde_json::to_string(&body).unwrap_or_else(|_| "{\"ok\":false}".into());
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type, authorization\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nConnection: close\r\n\r\n{}",
        status.code(),
        status.reason(),
        rendered.len(),
        rendered
    )
}

fn http_empty(status: StatusCode) -> String {
    format!(
        "HTTP/1.1 {} {}\r\nContent-Length: 0\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type, authorization\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nConnection: close\r\n\r\n",
        status.code(),
        status.reason()
    )
}

fn usage() -> String {
    "usage: bladb-gateway [serve [addr] [config.yaml|config.json]] | prepare <policy.yaml> <request.json> [auth.json] | route <policy.yaml> <topology.yaml> <request.json> [auth.json]".into()
}

fn parse_serve_args(args: &[String]) -> Result<(SocketAddr, Option<String>), String> {
    let default_addr: SocketAddr = "127.0.0.1:8787"
        .parse()
        .map_err(|error| format!("invalid default socket address: {error}"))?;

    match args {
        [] => Ok((default_addr, None)),
        [one] => match one.parse::<SocketAddr>() {
            Ok(addr) => Ok((addr, None)),
            Err(_) => Ok((default_addr, Some(one.clone()))),
        },
        [one, two, ..] => {
            let addr = one
                .parse::<SocketAddr>()
                .map_err(|error| format!("invalid socket address: {error}"))?;
            Ok((addr, Some(two.clone())))
        }
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl HttpRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

#[derive(Clone, Copy)]
enum StatusCode {
    Ok,
    NoContent,
    Unauthorized,
    BadRequest,
    Forbidden,
    NotFound,
    InternalServerError,
}

impl StatusCode {
    fn from_u16(code: u16) -> Self {
        match code {
            200 => Self::Ok,
            204 => Self::NoContent,
            401 => Self::Unauthorized,
            400 => Self::BadRequest,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            _ => Self::InternalServerError,
        }
    }

    fn code(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::NoContent => 204,
            Self::Unauthorized => 401,
            Self::BadRequest => 400,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::InternalServerError => 500,
        }
    }

    fn reason(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::NoContent => "No Content",
            Self::Unauthorized => "Unauthorized",
            Self::BadRequest => "Bad Request",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::InternalServerError => "Internal Server Error",
        }
    }
}
