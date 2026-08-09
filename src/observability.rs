//! ORES OpenTelemetry-compatible service lifecycle logging.
//!
//! Transfer identifiers, pairing material, capabilities, filenames, request
//! URLs, remote addresses, and file bytes are prohibited from this module.

use std::sync::Arc;

use next_loggers::{json, JsonObject, Logger, LoggerError, OpenTelemetryTransport, Options, Value};

pub fn logger() -> Logger {
    let transport = Arc::new(OpenTelemetryTransport::new(|record| {
        let encoded = serde_json::to_string(&record)
            .map_err(|error| LoggerError(format!("cannot encode OTEL log record: {error}")))?;
        eprintln!("{encoded}");
        Ok(())
    }));
    let mut options = Options::default().with_transport(transport);
    options.app_name = "ftnl-web-server".into();
    options.name = Some("web".into());
    options.console = false;
    Logger::new(options)
}

pub fn event(logger: &Logger, name: &'static str) {
    let _ = logger
        .info(vec![Value::String(name.into())])
        .add_fields(JsonObject::from_iter([
            ("event.name".into(), json!(name)),
            ("data.classification".into(), json!("metadata-free")),
        ]))
        .add_tags(["file-tunnel", "web"])
        .send();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logger_accepts_a_constant_metadata_free_event() {
        let logger = logger();
        event(&logger, "web.test");
        logger.close().unwrap();
    }
}
