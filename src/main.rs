//! A CI/CD for what brennanxyz needs right now.
mod modules;
mod routes;

use modules::{AppState, Config, ServiceBroadcast};
use routes::{
    add_new_service, all_status_request, app, deploy_service, edit_existing_service,
    edit_service_form, live_services, new_service_form, status,
};

use axum::{
    Router,
    routing::{get, post, put},
};
use sqlx::{Pool, sqlite::Sqlite};
use tracing::{Level, event};
use tracing_subscriber::fmt::writer::MakeWriterExt;

#[tokio::main]
async fn main() {
    // setup logging
    let logfile = tracing_appender::rolling::hourly("./logs", "route_traffic.log");
    let stdout = std::io::stdout.with_max_level(tracing::Level::INFO);
    tracing_subscriber::fmt()
        .pretty()
        .with_writer(stdout.and(logfile))
        .init();
    event!(Level::INFO, "Launching...");

    let config = match Config::new() {
        Ok(c) => {
            event!(Level::INFO, "Loaded configuration info.");
            c
        }
        Err(e) => {
            event!(Level::ERROR, "Failed to load configuration info.");
            panic!("Failed to load configuration info | {}", e);
        }
    };

    let db_string = &config.db_url;

    // TODO: get or create
    let pool = match Pool::<Sqlite>::connect(db_string).await {
        Ok(p) => {
            event!(Level::INFO, "Connected to DB.");
            p
        }
        Err(e) => {
            event!(Level::ERROR, "sqlite connection error | {}", e);
            panic!("sqlite connection error | {}", e);
        }
    };

    // run migrations
    match sqlx::migrate!("./migrations").run(&pool).await {
        Ok(_) => {
            event!(Level::INFO, "DB migration complete.");
        }
        Err(e) => {
            event!(Level::ERROR, "DB migrations failed | {}", e);
            panic!("DB migration failed | {}", e);
        }
    };

    let app_state = AppState {
        config: config.clone(),
        pool,
        service_broadcast: ServiceBroadcast::new(),
    };

    let app = Router::new()
        .route("/", get(app))
        .route("/status", get(status))
        .route("/html/service_form", get(new_service_form))
        .route("/html/service_form/{id}", get(edit_service_form))
        .route("/html/live_services", get(live_services))
        .route("/api/service", post(add_new_service))
        .route("/api/service/{id}", put(edit_existing_service))
        .route("/api/service/{id}/deploy", get(deploy_service))
        .route("/api/all_status", get(all_status_request))
        .with_state(app_state);

    let listener =
        match tokio::net::TcpListener::bind(format!("{}:{}", &config.app_host, &config.app_port))
            .await
        {
            Ok(lstnr) => {
                event!(Level::INFO, "Set up TCP listener.");
                event!(
                    Level::INFO,
                    "Running at {}:{}",
                    &config.app_host,
                    &config.app_port
                );
                lstnr
            }
            Err(e) => {
                event!(
                    Level::ERROR,
                    "Unexpected error in TCP listener setup | {}",
                    e
                );
                panic!("Unexpected error in TCP listener setup | {}", e);
            }
        };

    match axum::serve(listener, app.into_make_service()).await {
        Ok(_) => (),
        Err(e) => {
            event!(
                Level::ERROR,
                "Unexpected error in final app initialization step | {}",
                e
            );
            panic!("Unexpected error in final app initialization step | {}", e);
        }
    }
}
