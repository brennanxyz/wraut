use crate::modules::{
    AppState, db,
    service::{Service, ServiceEvent, ServiceStatus},
};

use axum::{
    Form,
    extract::{Path, State},
    response::{
        Html, IntoResponse, Sse,
        sse::{Event, KeepAlive},
    },
};
use futures::Stream;
use serde::Deserialize;
use tracing::{Level, event};

pub async fn status() -> impl IntoResponse {
    event!(Level::INFO, "GET /status");
    "OK"
}

pub async fn app() -> impl IntoResponse {
    event!(Level::INFO, "GET /");
    Html(
        "
        <!DOCTYPE html>
        <html lang=\"en\">
            <head>
                <script src=\"https://cdn.jsdelivr.net/npm/htmx.org@2.0.8/dist/htmx.min.js\"></script>
                <script src=\"https://cdn.jsdelivr.net/npm/htmx-ext-sse@2.2.4\" integrity=\"sha384-A986SAtodyH8eg8x8irJnYUk7i9inVQqYigD6qZ9evobksGNIXfeFvDwLSHcp31N\" crossorigin=\"anonymous\"></script>
            </head>
            <style>
                @import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:ital,wght@0,100;0,200;0,300;0,400;0,500;0,600;0,700;1,100;1,200;1,300;1,400;1,500;1,600;1,700&display=swap');
                @property --block-color {
                    syntax: \"<color>\";
                    inherits: false;
                    initial-value: #084b78;
                }
                @property --dark-color {
                    syntax: \"<color>\";
                    inherits: false;
                    initial-value: #1b2222;
                }
                @property --light-color {
                    syntax: \"<color>\";
                    inherits: false;
                    initial-value: #f9f5fc;
                }
                @property --success-color {
                    syntax: \"<color>\";
                    inherits: false;
                    initial-value: #33ca7f;
                }
                @property --warning-color {
                    syntax: \"<color>\";
                    inherits: false;
                    initial-value: #ffe45e;
                }
                @property --error-color {
                    syntax: \"<color>\";
                    inherits: false;
                    initial-value: #e02c29;
                }
                @property --unknown-color {
                    syntax: \"<color>\";
                    inherits: false;
                    initial-value: #AAAAAA;
                }
                html {
                    height: 100%;
                    margin: 0;
                }
                body {
                    background-color: var(--light-color);
                    height: 100%;
                    margin: 0;
                    padding: 0;
                    font-family: \"IBM Plex Mono\", monospace;
                    font-weight: 400;
                    font-style: normal;
                }
                input, textarea, select, button {
                    font-family:inherit;
                }
                th {
                    text-align: left;
                }
                .block {
                    padding: 12px;
                    border-radius: 4px;
                }
                .body {
                    display: flex;
                    flex-flow: column;
                    height: 100%;
                    background-color: var(--light-color);
                }
                .body .row.header {
                    flex: 0 1 auto;
                }
                .body .row.content {
                    flex: 1 1 auto;
                }
                .banner {
                    padding: 12px;
                    background-color: var(--block-color);
                    color: var(--light-color);
                }
                .error {
                    color: var(--error-color);
                }
                .warning {
                    background-color: var(--warning-color);
                }
                .button {
                    background-color: var(--block-color);
                    color: var(--light-color);
                    margin-top: 6px;
                    cursor: pointer;
                }
                .form {
                    background-color: var(--warning-color);
                    margin-top: 6px;
                }
                .success-chip, .warning-chip, .error-chip, .unknown-chip {
                    padding: 2px 6px 2px 6px;
                    border-radius: 4px;
                }
                .success-chip {
                    color: var(--light-color);
                    background-color: var(--success-color);
                }
                .warning-chip {
                    color: var(--dark-color);
                    background-color: var(--warning-color);
                }
                .error-chip {
                    color: var(--light-color);
                    background-color: var(--error-color);
                }
                .unknown-chip {
                    color: var(--dark-color);
                    background-color: var(--unknown-color);
                }
                #services-list {
                    padding: 12px;
                }
            </style>
            <body hx-ext=\"sse\">
                <div class=\"body\">
                    <div class=\"banner row header\" style=\"display:flex;flex-direction:row;justify-content:space-between\">
                        <div style=\"padding: 2px 0px 2px 0px;\">WRAUT</div>
                        <div
                            id=\"live-service-connection\"
                            sse-connect=\"/html/live_services\"
                            sse-swap=\"service_event\"
                        >
                            <!-- This is the direct target of the SSE endpoint -->
                            Connecting...
                        </div>
                    </div>
                    <table id=\"services-list\">
                        <tr><td>Waiting connection...</td></tr>
                    </table>
                    <div
                        id=\"add-service-btn\"
                        style=\"margin:12px;border-radius:4px;cursor:pointer;\"
                        class=\"success-chip\"
                        hx-get=\"/html/service_form\"
                        hx-swap=\"outerHTML\"
                    >
                        + Add service
                    </div>
                </div>
            </body>
        </html>
    ",
    )
}

pub async fn new_service_form() -> impl IntoResponse {
    event!(Level::INFO, "GET /html/service_form");

    return Html(
        "
        <form
            id=\"add-service-btn\"
            hx-post=\"/api/service\"
            hx-target=\"#services-list\"
            style=\"margin:12px;border-radius:4px;margin-left:auto;margin-right:auto;\"
            class=\"success-chip\"
        >
            <table>
                <tr><td align=\"right\">Name:</td><td><input name=\"name\" /></td></tr>
                <tr><td align=\"right\">Compose Name:</td><td><input name=\"compose_name\" /></td></tr>
                <tr><td align=\"right\">Repo URL:</td><td><input name=\"repo_url\" /></td></tr>
                <tr><td align=\"right\">Access URL:</td><td><input name=\"access_url\" /></td></tr>
                <tr><td align=\"right\">Active:</td><td><input name=\"active\" type=\"checkbox\" value=\"true\" /></td></tr>
                <tr><td align=\"right\">Use key:</td><td><input name=\"use_key\" type=\"checkbox\" value=\"false\" /></td></tr>
                <tr><td align=\"center\" colspan=\"2\"><button type=\"submit\">Submit</button></td></tr>
            </table>
        </form>
        ",
    );
}

pub async fn edit_service_form(
    State(app_state): State<AppState>,
    Path(service_id): Path<i64>,
) -> impl IntoResponse {
    event!(Level::INFO, "GET /html/service_form/:id");

    let service = match db::get_service(&app_state.pool, service_id).await {
        Ok(s) => s,
        Err(e) => {
            event!(Level::ERROR, "Unable to get service from DB | {}", e);
            return Html(
                "<td colspan=\"2\" class=\"error\">Unable to get service information.</td>"
                    .to_string(),
            );
        }
    };

    return Html(format!(
        "
        <td colspan=\"2\">
            <form hx-put=\"/api/service/{}\" hx-target=\"#services-list\">
                Name: <input name=\"name\" value=\"{}\"/>
                Repo URL: <input name=\"repo_url\" value=\"{}\"/><br />
                Access URL: <input name=\"access_url\" value=\"{}\"/><br />
                Active: <input name=\"active\" type=\"checkbox\" value=\"{}\" /><br />
                <button type=\"submit\">Submit</button>
            </form>
        </td>
        ",
        service.id, service.name, service.repo_url, service.access_url, service.active,
    ));
}

#[derive(Deserialize)]
pub struct ServiceForm {
    name: String,
    compose_name: String,
    repo_url: String,
    access_url: String,
    active: Option<bool>,
    use_key: Option<bool>,
}

pub async fn add_new_service(
    State(app_state): State<AppState>,
    Form(service_form): Form<ServiceForm>,
) -> impl IntoResponse {
    event!(Level::INFO, "POST /api/service");

    let service = Service {
        id: 0, // NOT USED
        name: service_form.name,
        compose_name: service_form.compose_name,
        repo_url: service_form.repo_url,
        access_url: service_form.access_url,
        active: service_form.active.unwrap_or(false),
        use_key: service_form.use_key.unwrap_or(false),
    };

    match db::new_service(&app_state.pool, service).await {
        Ok(_) => (),
        Err(e) => {
            event!(Level::ERROR, "Error processing new service | {}", e);
            return Html(
                "<div class=\"error\">Posting new service failed. See logs and reset the page.</div>",
            ).into_response();
        }
    }

    let _ = app_state
        .service_broadcast
        .broadcaster
        .send(ServiceEvent::AllStatus);

    "OK".into_response()
}

pub async fn edit_existing_service(
    State(app_state): State<AppState>,
    Path(service_id): Path<i64>,
    Form(service_form): Form<ServiceForm>,
) -> impl IntoResponse {
    event!(Level::INFO, "PUT /api/service/:id");

    let service = Service {
        id: 0, // NOT USED
        name: service_form.name,
        compose_name: service_form.compose_name,
        repo_url: service_form.repo_url,
        access_url: service_form.access_url,
        active: service_form.active.unwrap_or(false),
        use_key: service_form.use_key.unwrap_or(false),
    };

    match db::update_service(&app_state.pool, service_id, service).await {
        Ok(_) => (),
        Err(e) => {
            event!(Level::ERROR, "Error editing existing service | {}", e);
            return Html(
                "<div class=\"error\">Updating existing service failed. See logs and reset the page.</div>",
            ).into_response();
        }
    }

    let _ = app_state
        .service_broadcast
        .broadcaster
        .send(ServiceEvent::AllStatus);

    "OK".into_response()
}

pub async fn deploy_service(
    State(app_state): State<AppState>,
    Path(service_id): Path<i64>,
) -> impl IntoResponse {
    event!(Level::INFO, "GET /api/service/:id/deploy");
    let service = db::get_service(&app_state.pool, service_id.clone()).await;

    tokio::spawn(async move {
        let status = match Service::deploy(
            app_state.config,
            service,
            app_state.service_broadcast.broadcaster.clone(),
        )
        .await
        {
            Ok(_) => ServiceStatus::Running,
            Err(e) => ServiceStatus::from_error(e),
        };

        let _ = app_state
            .service_broadcast
            .broadcaster
            .send(ServiceEvent::ServiceUpdate {
                id: service_id,
                status,
            });
    });

    "OK"
}

pub async fn live_services(
    State(app_state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    event!(Level::INFO, "SSE /html/live_services");

    let stream = app_state
        .service_broadcast
        .event_stream(app_state.pool.clone())
        .await;

    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn all_status_request(State(app_state): State<AppState>) -> impl IntoResponse {
    event!(Level::INFO, "GET /api/all_status");

    match app_state
        .service_broadcast
        .broadcaster
        .send(ServiceEvent::AllStatus)
    {
        Ok(_) => "OK",
        Err(e) => {
            event!(Level::ERROR, "All status request failed | {}", e);
            "SIGNAL FAILED"
        }
    }
}
