use crate::{
    launcher::{start_server_modules, CreateServerModuleLauncherOptions, ServerModuleRegistry},
    transport::{ServerModuleTransport, TransportHandler},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

pub struct CreateHttpServerModuleTransportOptions {
    pub host: String,
    pub port: u16,
}

pub struct StartHttpServerModulesOptions {
    pub app: String,
    pub host: String,
    pub port: u16,
    pub registry: Arc<dyn ServerModuleRegistry>,
    pub modules: Vec<String>,
}

#[derive(Clone)]
pub struct HttpServerModuleTransport {
    host: String,
    port: u16,
    handlers: Arc<Mutex<HashMap<String, TransportHandler>>>,
}

impl HttpServerModuleTransport {
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

pub fn create_http_server_module_transport(
    options: CreateHttpServerModuleTransportOptions,
) -> Result<HttpServerModuleTransport, String> {
    let listener = TcpListener::bind(format!("{}:{}", options.host, options.port))
        .map_err(|error| format!("failed to bind rust server modules http transport: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| {
            format!("failed to read rust server modules http transport addr: {error}")
        })?
        .port();
    let transport = HttpServerModuleTransport {
        host: options.host.clone(),
        port,
        handlers: Arc::new(Mutex::new(HashMap::new())),
    };
    let transport_for_thread = transport.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                continue;
            };
            let transport = transport_for_thread.clone();
            thread::spawn(move || {
                let _ = handle_connection(stream, &transport);
            });
        }
    });

    Ok(transport)
}

impl ServerModuleTransport for HttpServerModuleTransport {
    fn subscribe(&self, subject: String, handler: TransportHandler) -> Result<(), String> {
        self.handlers
            .lock()
            .map_err(|_| "http server module transport lock poisoned".to_string())?
            .insert(subject, handler);
        Ok(())
    }
}

pub fn start_http_server_modules(
    options: StartHttpServerModulesOptions,
) -> Result<(HttpServerModuleTransport, Vec<String>), String> {
    let transport = create_http_server_module_transport(CreateHttpServerModuleTransportOptions {
        host: options.host.clone(),
        port: options.port,
    })?;

    let started = start_server_modules(CreateServerModuleLauncherOptions {
        app: options.app,
        transport: Arc::new(transport.clone()),
        registry: options.registry,
        modules: options.modules,
    })?;

    Ok((transport, started.subjects))
}

fn handle_connection(
    mut stream: TcpStream,
    transport: &HttpServerModuleTransport,
) -> Result<(), String> {
    let (request_head, body_bytes) = read_http_request(&mut stream)?;
    let request = String::from_utf8_lossy(&request_head);
    let mut lines = request.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default();
    let path = request_parts.next().unwrap_or_default();

    if method != "POST" || !path.starts_with("/invoke/") {
        write_json_response(
            &mut stream,
            404,
            json!({ "ok": false, "code": "NOT_FOUND", "message": "route not found" }),
        )?;
        return Ok(());
    }

    let payload: Value = if body_bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body_bytes).unwrap_or_else(|_| json!({}))
    };
    let subject = path.trim_start_matches("/invoke/").to_string();
    let handler = transport
        .handlers
        .lock()
        .map_err(|_| "http server module transport lock poisoned".to_string())?
        .get(&subject)
        .cloned();

    let Some(handler) = handler else {
        write_json_response(
            &mut stream,
            404,
            json!({
                "ok": false,
                "code": "SUBJECT_NOT_FOUND",
                "message": format!("no handler registered for {subject}"),
            }),
        )?;
        return Ok(());
    };

    let result = handler(payload).unwrap_or_else(|message| {
        json!({
            "ok": false,
            "code": "HTTP_TRANSPORT_ERROR",
            "message": message,
        })
    });
    write_json_response(&mut stream, 200, result)?;
    Ok(())
}

fn write_json_response(stream: &mut TcpStream, status: u16, body: Value) -> Result<(), String> {
    let body = serde_json::to_string(&body).map_err(|error| error.to_string())?;
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Internal Server Error",
    };
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        status_text,
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("failed to write response: {error}"))
}

fn read_http_request(stream: &mut TcpStream) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end;

    loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if read == 0 {
            return Err("request ended before headers were complete".into());
        }
        buffer.extend_from_slice(&chunk[..read]);

        if let Some(index) = find_header_end(&buffer) {
            header_end = index;
            break;
        }
    }

    let headers = buffer[..header_end].to_vec();
    let mut body = buffer[(header_end + 4).min(buffer.len())..].to_vec();
    let content_length = parse_content_length(&headers)?;

    while body.len() < content_length {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("failed to read request body: {error}"))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }

    body.truncate(content_length);
    Ok((headers, body))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &[u8]) -> Result<usize, String> {
    let text = String::from_utf8_lossy(headers);
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("Content-Length:") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|error| format!("invalid content-length: {error}"));
        }
        if let Some(value) = line.strip_prefix("content-length:") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|error| format!("invalid content-length: {error}"));
        }
    }

    Ok(0)
}
