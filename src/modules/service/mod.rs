pub mod html;

use std::path::PathBuf;

use serde::Deserialize;
use serde_yaml::Error as SerdeError;
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
    Copying,
    RewritingConfig,
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
            ServiceError::Stop => Self::CommandFailed("Failed to stop Docker service".to_string()),
            ServiceError::Remove => {
                Self::CommandFailed("Failed to remove live directory contents".to_string())
            }
            ServiceError::Copy => Self::CommandFailed("Failed to copy repo contents".to_string()),
            ServiceError::Yaml(_) => Self::CommandFailed("Failed to parse YAML file".to_string()),
            ServiceError::Key(k) => Self::CommandFailed(format!("Failed to find key '{}'", k)),
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
            Self::Copying => "Copying repo...".into(),
            Self::RewritingConfig => "Rewriting docker-compose.yml...".into(),
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
    pub compose_name: String,
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
    #[error("Error stopping the Docker service")]
    Stop,
    #[error("Error removing the contents of a directory")]
    Remove,
    #[error("Error copying the contents of a directory")]
    Copy,
    #[error("Error parsing YAML file")]
    Yaml(#[from] SerdeError),
    #[error("Error parsing expected key")]
    Key(String),
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

    fn make_labels(&self) -> Vec<String> {
        vec![self.label_name()]
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
        br: &broadcast::Sender<ServiceEvent>,
    ) -> Result<(), ServiceError> {
        let cf_string_opt = match self.cred_file.clone() {
            Some(cf) => Some(format!(" -c \"core.sshCommand=ssh -i {}\" ", cf)),
            None => None,
        };

        let mut path = config.services_repo_dir;
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

    pub fn copy_to_live(
        &self,
        config: Config,
        br: &broadcast::Sender<ServiceEvent>,
    ) -> Result<(), ServiceError> {
        let _ = br.send(ServiceEvent::ServiceUpdate {
            id: self.id,
            status: ServiceStatus::Copying,
        });

        let mut live_path = config.services_live_dir;
        live_path.push(self.name.clone());

        let (live_path, created) = Service::get_or_create_directory(live_path)?;

        let mut live_path_contents = live_path.clone();
        live_path_contents.push("*");

        if !created {
            let rm_outp = Command::new("rm")
                .arg("-rf")
                .arg(live_path_contents.to_string_lossy().to_string())
                .output()?;

            match rm_outp.status.success() {
                true => (),
                false => {
                    event!(
                        Level::ERROR,
                        "{}",
                        std::str::from_utf8(&rm_outp.stderr).unwrap_or("NA")
                    );
                    return Err(ServiceError::Remove);
                }
            }
        }

        let mut repo_path_contents = config.services_repo_dir;
        repo_path_contents.push(self.name.clone());
        repo_path_contents.push(".");

        let cp_outp = Command::new("cp")
            .arg("-af")
            .arg(repo_path_contents.to_string_lossy().to_string())
            .arg(".")
            .current_dir(live_path.to_string_lossy().to_string())
            .output()?;

        match cp_outp.status.success() {
            true => Ok(()),
            false => {
                event!(
                    Level::ERROR,
                    "{}",
                    std::str::from_utf8(&cp_outp.stderr).unwrap_or("NA")
                );
                Err(ServiceError::Copy)
            }
        }
    }

    pub fn apply_tags(
        &self,
        config: Config,
        br: &broadcast::Sender<ServiceEvent>,
    ) -> Result<(), ServiceError> {
        let _ = br.send(ServiceEvent::ServiceUpdate {
            id: self.id,
            status: ServiceStatus::RewritingConfig,
        });

        // Read docker-compose file
        let mut compose_path = config.services_live_dir;
        compose_path.push(self.name.clone());
        compose_path.push("docker-compose.yaml");
        let compose_content = std::fs::read_to_string(compose_path.clone())?;
        let mut compose: serde_yaml::Value = serde_yaml::from_str(&compose_content)?;

        // Get or create labels
        let services = match compose.get_mut("services") {
            Some(svcs) => svcs,
            None => {
                return Err(ServiceError::Key("services".into()));
            }
        };

        let service = match services.get_mut(self.compose_name.clone()) {
            Some(svc) => svc,
            None => {
                return Err(ServiceError::Key(self.compose_name.clone()));
            }
        };

        let service_map = match service.as_mapping_mut() {
            Some(sm) => sm,
            None => {
                return Err(ServiceError::Key(format!(
                    "{} (as map)",
                    self.compose_name.clone()
                )));
            }
        };

        let labels = service_map
            .entry(serde_yaml::Value::String("labels".into()))
            .or_insert_with(|| serde_yaml::Value::Sequence(vec![]));

        let label_array = match labels.as_sequence_mut() {
            Some(la) => la,
            None => {
                return Err(ServiceError::Key(format!(
                    "{} labels (as sequence)",
                    self.compose_name.clone()
                )));
            }
        };

        for label in self.make_labels() {
            label_array.push(serde_yaml::Value::String(label))
        }

        let yaml_string: String = serde_yaml::to_string(&compose)?;

        std::fs::write(compose_path, yaml_string)?;

        // Write back to file
        Ok(())
    }

    pub fn stop(
        &self,
        config: Config,
        br: &broadcast::Sender<ServiceEvent>,
    ) -> Result<(), ServiceError> {
        let _ = br.send(ServiceEvent::ServiceUpdate {
            id: self.id,
            status: ServiceStatus::Stopping,
        });

        let mut path = config.services_live_dir;
        path.push(&self.name);

        let (path, _) = Service::get_or_create_directory(path)?;

        let outp = Command::new("docker")
            .arg("compose")
            .arg("stop")
            .current_dir(path.to_string_lossy().to_string())
            .output()?;

        match outp.status.success() {
            true => Ok(()),
            false => Err(ServiceError::Stop),
        }
    }

    pub fn start(
        &self,
        config: Config,
        br: &broadcast::Sender<ServiceEvent>,
    ) -> Result<(), ServiceError> {
        let _ = br.send(ServiceEvent::ServiceUpdate {
            id: self.id,
            status: ServiceStatus::Starting,
        });

        let mut path = config.services_live_dir;
        path.push(&self.name);

        let (path, created) = match Service::get_or_create_directory(path) {
            Ok(outp) => outp,
            Err(e) => {
                event!(Level::ERROR, "GOC | {}", e);
                return Err(e);
            }
        };

        if created {
            event!(
                Level::ERROR,
                "Service {} created its live dir when it should already exist",
                self.name
            );
            return Err(ServiceError::Unexpected);
        }

        event!(Level::INFO, "{}", path.to_string_lossy().to_string());

        let output = match Command::new("docker")
            .arg("compose")
            .arg("up")
            .arg("-d")
            .current_dir(path.to_string_lossy().to_string())
            .output()
        {
            Ok(outp) => outp,
            Err(e) => {
                event!(Level::ERROR, "DCE | {}", e);
                return Err(ServiceError::Command(e));
            }
        };

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

                serv.clone_or_pull(config.clone(), &br)?;

                serv.copy_to_live(config.clone(), &br)?;

                serv.apply_tags(config.clone(), &br)?;

                if serv.is_running(&services) {
                    serv.stop(config.clone(), &br)?;
                }

                serv.start(config, &br)?;

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
