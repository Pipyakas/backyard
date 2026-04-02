use bollard::Docker;
use bollard::models::{HostConfig, DeviceRequest, DeviceMapping, ContainerCreateBody as Config};
use bollard::query_parameters::{CreateContainerOptions, StartContainerOptions};
use anyhow::{Result, Context};
use crate::state::State;

pub struct Orchestrator {
    docker: Docker,
}

impl Orchestrator {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_unix_defaults()
            .context("Failed to connect to Docker socket")?;
        Ok(Self { docker })
    }

    pub async fn start_engine(&self, model_id: i64, state: &State, gpu_type: &str) -> Result<String> {
        // 1. Get model and library path
        let (model_path, library_path): (String, String) = {
            let conn = state.conn.lock().unwrap();
            conn.query_row(
                "SELECT m.path, l.path FROM models m JOIN libraries l ON m.library_id = l.id WHERE m.id = ?",
                [model_id],
                |row| Ok((row.get(0)?, row.get(1)?))
            )?
        };

        // 2. Build HostConfig with GPU passthrough
        let mut host_config = HostConfig {
            binds: Some(vec![format!("{}:/models:ro", library_path)]),
            ..Default::default()
        };

        match gpu_type {
            "nvidia" => {
                host_config.device_requests = Some(vec![DeviceRequest {
                    driver: Some("nvidia".to_string()),
                    count: Some(-1),
                    capabilities: Some(vec![vec!["gpu".to_string()]]),
                    ..Default::default()
                }]);
            }
            "amd" => {
                host_config.devices = Some(vec![
                    DeviceMapping {
                        path_on_host: Some("/dev/kfd".to_string()),
                        path_in_container: Some("/dev/kfd".to_string()),
                        cgroup_permissions: Some("rwm".to_string()),
                    },
                    DeviceMapping {
                        path_on_host: Some("/dev/dri".to_string()),
                        path_in_container: Some("/dev/dri".to_string()),
                        cgroup_permissions: Some("rwm".to_string()),
                    },
                ]);
            }
            _ => {} // CPU only
        }

        // 3. Create and start container
        let container_name = format!("backyard-engine-{}", model_id);
        let image = match gpu_type {
            "nvidia" => "backyard:llama-cuda".to_string(),
            "amd" => "backyard:llama-rocm".to_string(),
            _ => "backyard:llama-cuda".to_string(), // Default
        };

        let config = Config {
            image: Some(image),
            host_config: Some(host_config),
            cmd: Some(vec![
                "-m".to_string(), 
                format!("/models/{}", model_path), 
                "--host".to_string(), 
                "0.0.0.0".to_string(), 
                "--port".to_string(), 
                "8080".to_string()
            ]),
            ..Default::default()
        };

        self.docker.create_container(
            Some(CreateContainerOptions { 
                name: Some(container_name.clone()), 
                platform: None,
            }), 
            config
        ).await?;
        
        self.docker.start_container(&container_name, None::<StartContainerOptions<String>>).await?;

        Ok(container_name)
    }

    pub async fn stop_engine(&self, container_name: &str) -> Result<()> {
        self.docker.stop_container(container_name, None).await?;
        self.docker.remove_container(container_name, None).await?;
        Ok(())
    }
}
