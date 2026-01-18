use crate::modules::{HTMLTarget, ServiceHTML, db::DBError};

use super::{DockerServiceEntry, Service, ServiceError, ServiceStatus};

pub fn list(
    db_list: Result<Vec<Service>, DBError>,
    docker_list: Result<Vec<DockerServiceEntry>, ServiceError>,
) -> ServiceHTML {
    match db_list {
        Ok(dbl) => match docker_list {
            Ok(dkl) => ServiceHTML {
                status_class: "success".to_string(),
                status_string: "Services found".to_string(),
                html_targets: vec![HTMLTarget {
                    id: "services-list".to_string(),
                    element: "table".to_string(),
                    class: None,
                    html_content: format!(
                        "
                        <tr>
                            <th>ID</th>
                            <th>Name</th>
                            <th>Repo</th>
                            <th>URL</th>
                            <th>Active</th>
                            <th>Status</th>
                        </tr>
                        {}",
                        dbl.iter()
                            .map(|dbe| {
                                format!(
                                    "
                            <tr>
                                <td>{}</td>
                                <td>{}</td>
                                <td>{}</td>
                                <td>{}</td>
                                <td>{}</td>
                                <td id=\"service-{}-status\" class=\"{}-chip\">{}</td>
                            </tr>
                        ",
                                    dbe.id,
                                    dbe.name,
                                    dbe.repo_url,
                                    dbe.access_url,
                                    dbe.active,
                                    dbe.id,
                                    match dbe.is_running(&dkl) {
                                        true => "success",
                                        false => "unknown",
                                    },
                                    match dbe.is_running(&dkl) {
                                        true => ServiceStatus::Running.to_string(),
                                        false => ServiceStatus::Inactive.to_string(),
                                    }
                                )
                            })
                            .collect::<String>()
                    ),
                }],
            },
            Err(e) => ServiceHTML {
                status_class: "warning".to_string(),
                status_string: "Services status unknown".to_string(),
                html_targets: vec![HTMLTarget {
                    id: "services-list".to_string(),
                    element: "table".to_string(),
                    class: None,
                    html_content: format!(
                        "
                        <tr>
                            <th>ID</th>
                            <th>Name</th>
                            <th>Repo</th>
                            <th>URL</th>
                            <th>Active</th>
                            <th>Status</th>
                        </tr>
                        {}
                        <tr><td colspan=\"6\" class=\"error-chip\">{}</td></tr>",
                        dbl.iter()
                            .map(|dbe| {
                                format!(
                                    "
                            <tr>
                                <td>{}</td>
                                <td>{}</td>
                                <td>{}</td>
                                <td>{}</td>
                                <td>{}</td>
                                <td id=\"service-{}-status\" class=\"unknown-chip\">{}</td>
                            </tr>
                        ",
                                    dbe.id,
                                    dbe.name,
                                    dbe.repo_url,
                                    dbe.access_url,
                                    dbe.active,
                                    dbe.id,
                                    ServiceStatus::Unknown.to_string(),
                                )
                            })
                            .collect::<String>(),
                        e,
                    ),
                }],
            },
        },
        Err(e) => ServiceHTML {
            status_class: "error".to_string(),
            status_string: "Database error".to_string(),
            html_targets: vec![HTMLTarget {
                id: "services-list".to_string(),
                element: "table".to_string(),
                class: None,
                html_content: format!(
                    "
                    <tr><td class=\"error-chip\">Unable to retrieve services from database. | {} </td></tr>
                ",
                    e
                ),
            }],
        },
    }
}

fn app_status_class(status: &ServiceStatus) -> String {
    match status {
        ServiceStatus::Unknown => "unknown".to_string(),
        ServiceStatus::Running | ServiceStatus::Inactive => "success".to_string(),
        ServiceStatus::DiscoveryFailed
        | ServiceStatus::CommandFailed(_)
        | ServiceStatus::CloneOrPullFailed => "error".to_string(),
        ServiceStatus::Cloning
        | ServiceStatus::Pulling
        | ServiceStatus::Stopping
        | ServiceStatus::Starting
        | ServiceStatus::Copying
        | ServiceStatus::DeploymentRequested => "warning".to_string(),
    }
}

fn app_status_name(status: &ServiceStatus) -> String {
    match status {
        ServiceStatus::Unknown => "Service unknown".to_string(),
        ServiceStatus::DiscoveryFailed
        | ServiceStatus::CommandFailed(_)
        | ServiceStatus::CloneOrPullFailed => "Service failure".to_string(),
        ServiceStatus::Cloning
        | ServiceStatus::Pulling
        | ServiceStatus::Stopping
        | ServiceStatus::Starting
        | ServiceStatus::Copying
        | ServiceStatus::DeploymentRequested => "Service pending...".to_string(),
        _ => "Connected".to_string(),
    }
}

fn service_class_name(status: &ServiceStatus) -> String {
    match status {
        ServiceStatus::Unknown | ServiceStatus::Inactive => "unknown".to_string(),
        ServiceStatus::Running => "success".to_string(),
        ServiceStatus::DiscoveryFailed
        | ServiceStatus::CommandFailed(_)
        | ServiceStatus::CloneOrPullFailed => "error".to_string(),
        ServiceStatus::Cloning
        | ServiceStatus::Pulling
        | ServiceStatus::Stopping
        | ServiceStatus::Starting
        | ServiceStatus::Copying
        | ServiceStatus::DeploymentRequested => "warning".to_string(),
    }
}

fn service_status_name(status: &ServiceStatus) -> String {
    status.clone().to_string()
}

pub fn service(service: Result<Service, DBError>, status: ServiceStatus) -> ServiceHTML {
    match service {
        Ok(serv) => ServiceHTML {
            status_class: app_status_class(&status),
            status_string: app_status_name(&status),
            html_targets: vec![HTMLTarget {
                id: format!("service-{}-status", serv.id),
                element: "div".to_string(),
                class: Some(service_class_name(&status)),
                html_content: service_status_name(&status),
            }],
        },
        Err(e) => ServiceHTML {
            status_class: "error".to_string(),
            status_string: "Unknown service error".to_string(),
            html_targets: vec![HTMLTarget {
                id: "app-message".to_string(),
                element: "div".to_string(),
                class: Some("error".to_string()),
                html_content: format!("Unable to access service from the database | {}", e),
            }],
        },
    }
}

pub fn unknown(msg: String) -> ServiceHTML {
    ServiceHTML {
        status_class: "error".to_string(),
        status_string: "Unknown error".to_string(),
        html_targets: vec![HTMLTarget {
            id: "app-message".to_string(),
            element: "div".to_string(),
            class: Some("error".to_string()),
            html_content: msg,
        }],
    }
}
