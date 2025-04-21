use clap::Parser;
use futures::future::join_all;
use rand::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::{MissedTickBehavior, interval, sleep};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "http://localhost:8081/event")]
    url: String,

    #[arg(short = 'r', long, default_value_t = 10000)]
    rps: u64,

    #[arg(short = 'd', long, default_value_t = 10)]
    duration: u64,

    #[arg(short = 'c', long, default_value_t = 100)]
    concurrency: usize,
}

#[derive(Serialize, Deserialize, Debug)]
struct EventPayload {
    user_id: u64,
    event_type: String,
    action: String,
    info: Value,
}

const EVENT_TYPES: &[&str] = &["click", "page_view", "add_to_cart", "purchase", "scroll"];
const ACTIONS_PER_TYPE: &[&[&str]] = &[
    &["submit", "link", "image", "button"],
    &["load", "unload"],
    &["product_card", "quick_add"],
    &["checkout_complete", "paypal"],
    &["page_bottom", "element_visible"],
];

fn generate_random_event() -> EventPayload {
    let mut rng = rand::rng();
    let type_index = rng.random_range(0..EVENT_TYPES.len());
    let event_type = EVENT_TYPES[type_index].to_string();
    let possible_actions = ACTIONS_PER_TYPE[type_index];
    let action = possible_actions[rng.random_range(0..possible_actions.len())].to_string();
    let user_id = rng.random_range(1..1_000_000_000);

    let info = match event_type.as_str() {
        "click" => json!({
            "element_id": format!("btn-{}", rng.random_range(1..100)),
            "target_url": format!("/path/{}", Uuid::new_v4())
        }),
        "page_view" => json!({
            "url": format!("/page/{}", Uuid::new_v4()),
            "referrer": format!("https://referrer{}.com", rng.random_range(1..5))
        }),
        "add_to_cart" => json!({
            "product_id": format!("prod-{}", rng.random_range(1000..2000)),
            "quantity": rng.random_range(1..5),
            "price": rng.random_range(1.0..100.0)
        }),
        "purchase" => json!({
            "order_id": Uuid::new_v4().to_string(),
            "total_value": rng.random_range(10.0..500.0),
            "currency": "USD"
        }),
        "scroll" => json!({ "scroll_depth_percent": rng.random_range(1..100) }),
        _ => json!({}),
    };

    EventPayload {
        user_id,
        event_type,
        action,
        info,
    }
}

async fn worker(
    id: usize,
    client: Arc<Client>,
    url: String,
    target_delay: Duration,
    counter: Arc<AtomicU64>,
) {
    let mut interval = interval(target_delay);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        interval.tick().await;
        let event = generate_random_event();
        let send_result = client.post(&url).json(&event).send().await;

        counter.fetch_add(1, Ordering::Relaxed);

        match send_result {
            Ok(response) => {
                if !response.status().is_success() {
                    eprintln!(
                        "Worker {} received non-success status: {}",
                        id,
                        response.status()
                    );
                }
            }
            Err(e) => {
                eprintln!("Worker {} failed to send request: {}", id, e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!(
        "Starting load test against {} with target {} RPS for {} seconds using {} concurrent workers.",
        args.url, args.rps, args.duration, args.concurrency
    );

    let client = Arc::new(Client::new());
    let start_time = Instant::now();
    let test_duration = Duration::from_secs(args.duration);

    let requests_sent = Arc::new(AtomicU64::new(0));

    let rps_per_worker = args.rps as f64 / args.concurrency as f64;

    let delay_per_worker_ns = if rps_per_worker > 0.0 {
        (1_000_000_000.0 / rps_per_worker).max(1.0) as u64
    } else {
        u64::MAX
    };
    let target_delay = Duration::from_nanos(delay_per_worker_ns);

    println!(
        "Target RPS per worker: {:.2}, Target delay per worker: {:?}",
        rps_per_worker, target_delay
    );
    if delay_per_worker_ns == u64::MAX {
        println!("Warning: Target RPS is 0, workers will not send requests frequently.");
    }

    let mut worker_handles = Vec::new();

    for i in 0..args.concurrency {
        let client_clone = Arc::clone(&client);
        let url_clone = args.url.clone();
        let counter_clone = Arc::clone(&requests_sent);
        let handle = tokio::spawn(worker(
            i,
            client_clone,
            url_clone,
            target_delay,
            counter_clone,
        ));
        worker_handles.push(handle);
    }

    sleep(test_duration).await;

    println!(
        "Test duration of {} seconds finished. Stopping workers...",
        args.duration
    );

    for handle in worker_handles {
        handle.abort();
    }

    let elapsed = start_time.elapsed();
    let final_count = requests_sent.load(Ordering::Relaxed);
    let measured_rps = final_count as f64 / elapsed.as_secs_f64();

    println!("Load test finished in {:?}", elapsed);
    println!(
        "Total requests sent (approximated by client attempts): {}",
        final_count
    );
    println!("Measured RPS: {:.2}", measured_rps);
}
