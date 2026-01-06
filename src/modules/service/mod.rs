pub mod html;

use std::path::PathBuf;

use serde::Deserialize;
use std::process::{Command, Output};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{Level, event};

use super::{Config, db::DBError};

#[derive(Clone, Debug)]
pub enum ServiceStatus {
    Inactive,
    Running,
    DiscoveryFailed,
    CommandFailed(String),
    CloneOrPullFailed,
    DeploymentRequested,
    Cloning,
    Pulling,
    Stopping,
    Starting,
    Unknown,
}

impl ServiceStatus {
    pub fn from_error(se: ServiceError) -> Self {
        match se {
            ServiceError::Command(e) => Self::CommandFailed(e.to_string()),
            ServiceError::Status => {
                Self::CommandFailed("Command resulted in failure status".to_string())
            }
            ServiceError::Unexpected => {
                Self::CommandFailed("Command resulted in unexpected string".to_string())
            }
            ServiceError::Parse(_) => {
                Self::CommandFailed("Failed to parse command output".to_string())
            }
            ServiceError::Start => {
                Self::CommandFailed("Failed to start Docker service".to_string())
            }
            ServiceError::Unknown => Self::Unknown,
            ServiceError::Discovery => Self::DiscoveryFailed,
            ServiceError::CloneOrPull => Self::CloneOrPullFailed,
        }
    }

    pub fn to_string(self) -> String {
        match self {
            Self::Inactive => "Inactive".into(),
            Self::Running => "Running".into(),
            Self::DiscoveryFailed => "Failed to discover service".into(),
            Self::CommandFailed(s) => format!("Failed command | {}", s),
            Self::CloneOrPullFailed => "Failed to clone or pull".into(),
            Self::DeploymentRequested => "Deployment requested...".into(),
            Self::Cloning => "Cloning repo...".into(),
            Self::Pulling => "Pulling repo...".into(),
            Self::Stopping => "Stopping service...".into(),
            Self::Starting => "Starting service...".into(),
            Self::Unknown => "Unknown status".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ServiceEvent {
    AllStatus,
    ServiceUpdate { id: i64, status: ServiceStatus },
    UnknownEvent { msg: String },
}

#[derive(Clone)]
pub struct Service {
    pub id: i64,
    pub name: String,
    pub repo_url: String,
    pub access_url: String,
    pub active: bool,
    pub cred_file: Option<String>,
}

#[allow(non_snake_case, dead_code)]
#[derive(Deserialize, Debug)]
pub struct DockerServiceEntry {
    ID: String,
    Image: String,
    Names: String,
    Labels: String,
    State: String,
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("No response from system command")]
    Command(#[from] std::io::Error),
    #[error("System command resulted in failure")]
    Status,
    #[error("System command returned unexpected output")]
    Unexpected,
    #[error("Failed to parse output string")]
    Parse(#[from] std::str::Utf8Error),
    #[error("Error unknown to Service domain")]
    Unknown,
    #[error("Error in Service discovery in Docker")]
    Discovery,
    #[error("Error cloning or pulling a repo")]
    CloneOrPull,
    #[error("Error starting the Docker service")]
    Start,
}

impl Service {
    pub fn label_name(&self) -> String {
        format!("|||{}|||", self.name)
    }

    pub async fn get_list() -> Result<Vec<DockerServiceEntry>, ServiceError> {
        let output = Command::new("docker")
            .args(vec!["ps", "--format", "json"])
            .output()?;

        match output.status.success() {
            true => (),
            false => {
                return Err(ServiceError::Status);
            }
        }

        let output_string = std::str::from_utf8(&output.stdout)?;

        let containers: Vec<DockerServiceEntry> = output_string
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        Ok(containers)
    }

    pub fn is_running(&self, services: &Vec<DockerServiceEntry>) -> bool {
        match services.len() {
            0 => {
                // no services running
                false
            }
            _ => {
                match services
                    .iter()
                    .find(|service| service.Labels.contains(&self.label_name()))
                {
                    Some(service) => service.State == "running".to_string(),
                    None => false,
                }
            }
        }
    }

    // on Result::Ok, returns path, and a boolean: true = created; false = got existing
    fn get_or_create_directory(path: PathBuf) -> Result<(PathBuf, bool), ServiceError> {
        let chkdir_output = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "[ -d {} ] && echo \"Y\" || echo \"N\"",
                path.to_string_lossy()
            ))
            .output()?;

        match chkdir_output.status.success() {
            true => (),
            false => {
                return Err(ServiceError::Status);
            }
        }

        let chkdir_output_string = std::str::from_utf8(&chkdir_output.stdout)?;

        event!(Level::WARN, "||{}||", chkdir_output_string);

        match chkdir_output_string {
            "Y\n" => Ok((path, false)),
            "N\n" => {
                event!(Level::WARN, "mkdir {}", path.to_string_lossy());
                let mkdir_output = Command::new("mkdir")
                    .arg(format!("{}", path.to_string_lossy()))
                    .output()?;

                match mkdir_output.status.success() {
                    true => Ok((path, true)),
                    false => {
                        event!(
                            Level::WARN,
                            "{}",
                            std::str::from_utf8(&mkdir_output.stderr)?
                        );
                        Err(ServiceError::Status)
                    }
                }
            }
            _ => Err(ServiceError::Unexpected),
        }
    }

    pub fn clone_or_pull(
        &self,
        config: Config,
        br: broadcast::Sender<ServiceEvent>,
    ) -> Result<(), ServiceError> {
        let cf_string_opt = match self.cred_file.clone() {
            Some(cf) => Some(format!(" -c \"core.sshCommand=ssh -i {}\" ", cf)),
            None => None,
        };

        let mut path = config.services_root_dir;
        path.push(&self.name);

        let (path, created) = Service::get_or_create_directory(path)?;

        let output: Output = match created {
            true => {
                let _ = br.send(ServiceEvent::ServiceUpdate {
                    id: self.id,
                    status: ServiceStatus::Cloning,
                });

                match cf_string_opt {
                    Some(cf_string) => Command::new("git")
                        .arg("clone")
                        .arg(cf_string)
                        .arg(self.repo_url.clone())
                        .arg(path.to_string_lossy().to_string())
                        .output()?,
                    None => Command::new("git")
                        .arg("clone")
                        .arg(self.repo_url.clone())
                        .arg(path.to_string_lossy().to_string())
                        .output()?,
                }
            }
            false => {
                let _ = br.send(ServiceEvent::ServiceUpdate {
                    id: self.id,
                    status: ServiceStatus::Pulling,
                });

                match cf_string_opt {
                    Some(cf_string) => Command::new("git")
                        .arg("pull")
                        .arg(cf_string)
                        .current_dir(path)
                        .output()?,
                    None => Command::new("git").arg("pull").current_dir(path).output()?,
                }
            }
        };

        match output.status.success() {
            true => Ok(()),
            false => {
                event!(
                    Level::ERROR,
                    "CLONE FAIL | {}",
                    std::str::from_utf8(&output.stderr)?
                );
                Err(ServiceError::CloneOrPull)
            }
        }
    }

    pub fn stop(
        &self,
        config: Config,
        br: broadcast::Sender<ServiceEvent>,
    ) -> Result<(), ServiceError> {
        let _ = br.send(ServiceEvent::ServiceUpdate {
            id: self.id,
            status: ServiceStatus::Stopping,
        });

        let mut path = config.services_root_dir;
        path.push(&self.name);

        let (path, _) = Service::get_or_create_directory(path)?;

        Command::new(format!(
            "mv {} && docker compose stop",
            path.to_string_lossy(),
        ))
        .output()?;

        Ok(())
    }

    pub fn start(
        &self,
        config: Config,
        br: broadcast::Sender<ServiceEvent>,
    ) -> Result<(), ServiceError> {
        let _ = br.send(ServiceEvent::ServiceUpdate {
            id: self.id,
            status: ServiceStatus::Starting,
        });

        let mut path = config.services_root_dir;
        path.push(&self.name);

        let (path, _) = Service::get_or_create_directory(path)?;

        let output = Command::new("docker")
            .arg("compose")
            .arg("up")
            .arg("-d")
            .current_dir(path.to_string_lossy().to_string())
            .output()?;

        match output.status.success() {
            true => Ok(()),
            false => {
                event!(
                    Level::ERROR,
                    "START FAIL | {}",
                    std::str::from_utf8(&output.stderr)?
                );
                Err(ServiceError::Start)
            }
        }
    }

    pub async fn deploy(
        config: Config,
        service: Result<Service, DBError>,
        br: broadcast::Sender<ServiceEvent>,
    ) -> Result<(), ServiceError> {
        // emit `ServiceEvent`s instead of returning a value
        event!(Level::INFO, "Initiating deployment...");

        match service {
            Ok(serv) => {
                let _ = br.send(ServiceEvent::ServiceUpdate {
                    id: serv.id,
                    status: ServiceStatus::DeploymentRequested,
                });

                let services = match Self::get_list().await {
                    Ok(lst) => lst,
                    Err(_e) => {
                        let _ = br.send(ServiceEvent::ServiceUpdate {
                            id: serv.id,
                            status: ServiceStatus::DiscoveryFailed,
                        });
                        return Err(ServiceError::Discovery);
                    }
                };

                serv.clone_or_pull(config.clone(), br.clone())?;

                if serv.is_running(&services) {
                    serv.stop(config.clone(), br.clone())?;
                }

                serv.start(config, br)?;

                Ok(())
            }
            Err(e) => {
                event!(
                    Level::ERROR,
                    "Service not successfully pulled from database | {}",
                    e
                );
                let _ = br.send(ServiceEvent::UnknownEvent { msg: e.to_string() });
                Err(ServiceError::Unknown)
            }
        }
    }
}
