use std::{sync::Arc, time::Duration};

fn native_tls_connector() -> Result<Arc<ureq::native_tls::TlsConnector>, String> {
    let connector = ureq::native_tls::TlsConnector::new()
        .map_err(|error| format!("failed to initialize native TLS connector: {error}"))?;
    Ok(Arc::new(connector))
}

pub fn default_http_agent() -> Result<ureq::Agent, String> {
    let connector = native_tls_connector()?;
    Ok(ureq::AgentBuilder::new().tls_connector(connector).build())
}

pub fn http_agent_with_timeouts(
    read_timeout: Duration,
    write_timeout: Duration,
) -> Result<ureq::Agent, String> {
    let connector = native_tls_connector()?;
    Ok(ureq::AgentBuilder::new()
        .timeout_read(read_timeout)
        .timeout_write(write_timeout)
        .tls_connector(connector)
        .build())
}
