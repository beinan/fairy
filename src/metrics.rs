use prometheus::{
    Counter, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
};
use prometheus::{labels, register_counter, register_histogram};

use lazy_static::lazy_static;

use std::error::Error;

use log::{error, trace};

use crate::settings::SETTINGS;



lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    pub static ref INCOMING_REQUESTS : IntCounter =
        IntCounter::new("incoming_requests", "Incoming Requests").expect("metric can be created");

    pub static ref CONNECTED_CLIENTS: IntGauge =
        IntGauge::new("connected_clients", "Connected Clients").expect("metric can be created");

    pub static ref RESPONSE_CODE_COLLECTOR: IntCounterVec = IntCounterVec::new(
        Opts::new("response_code", "Response Codes"),
        &["env", "statuscode", "type"]
    )
    .expect("metric can be created");

    pub static ref RESPONSE_TIME_COLLECTOR: HistogramVec = HistogramVec::new(
        HistogramOpts::new("response_time", "Response Times"),
        &["env"]
    )
    .expect("metric can be created");
    static ref PUSH_COUNTER: Counter = register_counter!(
        "push_counter",
        "Total number of prometheus client pushed."
    )
    .unwrap();
    static ref PUSH_REQ_HISTOGRAM: Histogram = register_histogram!(
        "push_request_latency_seconds",
        "The push request latencies in seconds."
    )
    .unwrap();
}

pub fn register_custom_metrics() {
    REGISTRY
        .register(Box::new(INCOMING_REQUESTS.clone()))
        .expect("collector can be registered");

    REGISTRY
        .register(Box::new(CONNECTED_CLIENTS.clone()))
        .expect("collector can be registered");

    REGISTRY
        .register(Box::new(RESPONSE_CODE_COLLECTOR.clone()))
        .expect("collector can be registered");

    REGISTRY
        .register(Box::new(RESPONSE_TIME_COLLECTOR.clone()))
        .expect("collector can be registered");
}

pub fn push_metrics() -> Result<(), Box<dyn Error>>{
    let push_uri = SETTINGS.metrics_push_uri.as_deref().unwrap();
    trace!("Pushing metrics to gateway {}", push_uri);

    PUSH_COUNTER.inc();
    let metric_families = prometheus::gather();
    let _timer = PUSH_REQ_HISTOGRAM.start_timer();
    let push_result = prometheus::push_metrics(
        "fairy_worker_push",
        labels! {"fairy_worker".to_owned() => "worker".to_owned(),},
        push_uri,
        metric_families,
        None,
    );
    match push_result {
        Ok(_) => {
            trace!("Pushing metrics to gateway {} succeed", push_uri);
            Ok(())
        },
        Err(e) => {
            error!("Push metrics failed: {}", e);
            Ok(())
        }
    }
}

pub fn metrics_result() -> String{
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

    return res;
}