use actix_cors::Cors;
use actix_web::{App, HttpResponse, HttpServer, Responder, error, web, http};
use chrono::{DateTime, Utc};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
struct EventPayload {
    user_id: u64,
    event_type: String,
    action: String,
    info: Value,
}

#[derive(Serialize, Debug)]
struct EventInternal {
    user_id: u64,
    #[serde(serialize_with = "serialize_datetime_as_millis")]
    event_ts: DateTime<Utc>,
    event_type: String,
    action: String,
    info: Value,
}

fn serialize_datetime_as_millis<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_i64(dt.timestamp_millis())
}

async fn handle_event(
    event: web::Json<EventPayload>,
    producer: web::Data<FutureProducer>,
) -> Result<HttpResponse, error::Error> {
    println!("Received event: {:?}", event);
    let internal_event = EventInternal {
        user_id: event.user_id,
        event_ts: Utc::now(),
        event_type: event.event_type.clone(),
        action: event.action.clone(),
        info: event.info.clone(),
    };

    let payload_str = match serde_json::to_string(&internal_event) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to serialize event: {}", e);

            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    let user_id_str = internal_event.user_id.to_string();
    let record = FutureRecord::to("user-events")
        .payload(&payload_str)
        .key(&user_id_str);

    let status = producer
        .send(record, Timeout::After(Duration::from_secs(2)))
        .await;

    match &status {
        Ok(_) => {
            println!("Message sent successfully. Payload: {}, status: {:?}", payload_str, &status);
            Ok(HttpResponse::Ok().finish())
        }
        Err((e, _owned_message)) => {
            eprintln!("Failed to produce message: {}, status: {:?}", e, &status);

            Ok(HttpResponse::InternalServerError().finish())
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {

    let producer: FutureProducer = ClientConfig::new()
        .set(
            "bootstrap.servers",
            "localhost:29092,localhost:39092,localhost:49092",
        )
        .set("message.timeout.ms", "5000")
        .set("acks", "all") // or 1 for performance
        .set("compression.type", "snappy") // or none
        .set("linger.ms", "10")
        .set("batch.size", "65536")
        .set("message.send.max.retries", "5")
        .set("enable.idempotence", "true")
        .create()
        .expect("Producer creation error");

    println!("Starting Actix server...");

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::default()
                .allowed_origin("http://localhost:5173") // Your Vite dev server URL
                .allowed_methods(vec!["POST"])
                .allowed_headers(vec![http::header::CONTENT_TYPE])
                .max_age(3600))
            .app_data(web::Data::new(producer.clone()))
            .route("/event", web::post().to(handle_event))
    })
    .bind(("0.0.0.0", 8081))
    .expect("Failed to bind to port 8081")
    .run()
    .await
}
