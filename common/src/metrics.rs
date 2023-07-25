use prometheus::{
    labels, register_counter, register_histogram, register_int_counter_vec, register_int_gauge,
};
use prometheus::{Counter, Histogram, IntCounterVec, IntGauge, Opts, Registry};

use lazy_static::lazy_static;

use std::error::Error;

use log::{error, info, trace};

use crate::settings::SETTINGS;

use std::time::Duration;
use tokio::time::sleep;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    pub static ref INCOMING_REQUESTS: Counter =
        register_counter!("incoming_requests", "Incoming Requests").unwrap();
    pub static ref CONNECTED_CLIENTS: IntGauge =
        register_int_gauge!("connected_clients", "Connected Clients").unwrap();
    pub static ref RESPONSE_CODE_COLLECTOR: IntCounterVec = register_int_counter_vec!(
        Opts::new("response_code", "Response Codes"),
        &["env", "statuscode", "type"]
    )
    .unwrap();
    pub static ref RESPONSE_TIME_COLLECTOR: Histogram =
        register_histogram!("response_time", "Response Times").unwrap();
    static ref PUSH_COUNTER: Counter =
        register_counter!("push_counter", "Total number of prometheus client pushed.").unwrap();
    static ref PUSH_REQ_HISTOGRAM: Histogram = register_histogram!(
        "push_request_latency_seconds",
        "The push request latencies in seconds."
    )
    .unwrap();
}

pub async fn start_push() -> Result<(), Box<dyn Error>> {
    if SETTINGS.metrics_push_uri.is_none() {
        info!("No prometheus push uri specified");
        return Ok(())
    }
    tokio::spawn(async move {
        loop {
            tokio::task::spawn_blocking(move || {
                let _ = push_metrics();
            });
            sleep(Duration::from_secs(30)).await;
        }
    });

    tokio::task::yield_now().await;
    Ok(())
}

pub fn push_metrics() -> Result<(), Box<dyn Error>> {
    let push_uri = SETTINGS.metrics_push_uri.as_deref().unwrap();
    trace!("Pushing metrics to gateway {}", push_uri);

    PUSH_COUNTER.inc();
    let metric_families = prometheus::gather();
    let _timer = PUSH_REQ_HISTOGRAM.start_timer();
    let push_result = prometheus::push_metrics(
        "fairy_worker",
        labels! {"instance".to_owned() => format!("{}:{}", SETTINGS.local_ip, SETTINGS.http_port),},
        push_uri,
        metric_families,
        None,
    );
    match push_result {
        Ok(_) => {
            trace!("Pushing metrics to gateway {} succeed", push_uri);
            Ok(())
        }
        Err(e) => {
            error!("Push metrics failed: {}", e);
            Ok(())
        }
    }
}

pub fn metrics_result() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
        eprintln!("could not encode custom metrics: {}", e);
    };
    let mut res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("custom metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        eprintln!("could not encode prometheus metrics: {}", e);
    };
    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("prometheus metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    res.push_str(&res_custom);

    res
}
