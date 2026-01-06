pub mod db;
pub mod service;

use std::{
    env,
    path::{Path, PathBuf},
};

use async_stream::stream;
use axum::response::sse::Event;
use dotenv::dotenv;
use futures::stream::Stream;
use service::{Service, ServiceEvent};
use sqlx::{Pool, Sqlite, SqlitePool};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{Level, event};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Environment variable missing")]
    EnvVarError(#[from] env::VarError),
    #[error("Environment variable parse error")]
    ParseError(#[from] std::num::ParseIntError),
}

#[derive(Clone, Debug)]
pub struct Config {
    pub db_url: String,
    pub app_host: String,
    pub app_port: u16,
    pub services_root_dir: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self, ConfigError> {
        dotenv().ok();
        let db_url = env::var("DB_URL")?;
        let app_host = env::var("APP_HOST")?;
        let app_port = env::var("APP_PORT")?.parse::<u16>()?;
        let services_root_dir_string: String = env::var("SERVICE_ROOT_PATH")?;
        let services_root_dir = Path::new(services_root_dir_string.as_str());
        Ok(Config {
            db_url,
            app_host,
            app_port,
            services_root_dir: services_root_dir.to_path_buf(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Config,
    pub pool: Pool<Sqlite>,
    pub service_broadcast: ServiceBroadcast,
}

#[derive(Clone, Debug)]
pub struct ServiceBroadcast {
    pub broadcaster: broadcast::Sender<ServiceEvent>,
}

pub struct HTMLTarget {
    id: String,
    element: String,
    class: Option<String>,
    html_content: String,
}

pub struct ServiceHTML {
    status_class: String,
    status_string: String,
    html_targets: Vec<HTMLTarget>,
}

impl ServiceHTML {
    fn render(self) -> Event {
        let target_snippets: String = self
            .html_targets
            .iter()
            .map(|ht| {
                format!(
                    "
                <{} id=\"{}\" hx-swap-oob=\"true\" {}>{}</{}>        
            ",
                    ht.element,
                    ht.id,
                    match &ht.class {
                        Some(c) => format!("class=\"{}\"", c),
                        None => "".to_string(),
                    },
                    ht.html_content,
                    ht.element
                )
            })
            .collect();

        Event::default().event("service_event").data(&format!(
            "
            <div id=\"link-status\" class=\"{}-chip\">{}</div>
            {}
            ",
            self.status_class, self.status_string, target_snippets
        ))
    }
}

impl ServiceBroadcast {
    pub fn new() -> Self {
        let (broadcaster, _) = broadcast::channel(100);
        Self { broadcaster }
    }

    fn subscribe(&self) -> broadcast::Receiver<ServiceEvent> {
        self.broadcaster.subscribe()
    }

    pub async fn event_stream(
        self,
        pool: SqlitePool,
    ) -> impl Stream<Item = Result<Event, axum::Error>> {
        let mut receiver = self.subscribe();

        stream! {
            // yield the list that triggers the AllStatus event
            yield Ok(Event::default().event("service_event").data(
                "
                <div id=\"link-status\" class=\"success-chip\">Connected</div>
                <table id=\"services-list\" hx-swap-oob=\"true\"><tr><td hx-get=\"/api/all_status\" hx-trigger=\"load\">Waiting query results...</td></tr></table>
                <div id=\"app-message\"></div>
                "
            ));

            while let Ok(event) = receiver.recv().await {
                let docker_list = Service::get_list().await;
                match event {
                    ServiceEvent::AllStatus => {
                        let db_list = db::get_services(&pool).await;
                        yield(Ok(service::html::list(db_list, docker_list).render()));
                    },
                    ServiceEvent::ServiceUpdate {id, status} => {
                        event!(Level::WARN, "{:?}", status);
                        let service = db::get_service(&pool, id).await;
                        yield(Ok(service::html::service(service, status).render()));
                    },
                    ServiceEvent::UnknownEvent { msg } => {
                        yield(Ok(service::html::unknown(msg).render()));
                    }
                }
            }
        }
    }
}
