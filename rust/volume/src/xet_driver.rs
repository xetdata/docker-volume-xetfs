use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::State;
use axum::Json;
use tokio::sync::RwLock;
use tokio::{fs, process};
use tracing::{info, warn};

use docker_volume::driver::{
    CapabilitiesResponse, Capability, CreateRequest, GetRequest, GetResponse, ListResponse,
    MountRequest, MountResponse, NullResponse, PathRequest, PathResponse, RemoveRequest, Scope,
    UnmountRequest, Volume, VolumeDriver,
};
use docker_volume::errors::{VolumeError, VolumeResponse};

#[derive(Debug, Default)]
pub struct XetVolume {
    repo: String,
    commit: String,
    username: String,
    pat: String,
    writeable: bool,
    mount_path: PathBuf,
    // Watch for changes to the commit reference over some interval (e.g. 30s, 1h)
    watch: Option<String>,
}

impl XetVolume {
    fn try_apply_option(&mut self, (key, value): (String, String)) -> Result<(), VolumeError> {
        match key.to_lowercase().as_str() {
            "repo" => self.repo = value,
            "commit" => self.commit = value,
            "username" => self.username = value,
            "pat" => self.pat = value,
            "write" => self.writeable = bool::from_str(&value.to_lowercase()).unwrap_or(false),
            "watch" => self.watch = Some(value.to_lowercase()),
            _ => {
                return Err(VolumeError::NoOption(key));
            }
        };
        Ok(())
    }

    fn validate(&self) -> Result<(), VolumeError> {
        if self.repo.is_empty() {
            return Err(VolumeError::InvalidOptions(
                "\"repo\" option not set".to_string(),
            ));
        }
        if self.commit.is_empty() {
            return Err(VolumeError::InvalidOptions(
                "\"commit\" option not set".to_string(),
            ));
        }
        if self.writeable && self.watch.is_some() {
            return Err(VolumeError::InvalidOptions(
                "Writable and watch are incompatible options".to_string(),
            ));
        }
        if let Some(ref interval) = self.watch {
            humantime::parse_duration(interval).map_err(|_| {
                VolumeError::InvalidOptions(format!(
                    "interval provided for watch: {interval} is invalid"
                ))
            })?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct XetDriver {
    mounts: Arc<RwLock<HashMap<String, XetVolume>>>,
    mount_root: PathBuf,
}

impl XetDriver {
    pub fn new(mount_root: PathBuf) -> Self {
        Self {
            mounts: Arc::new(RwLock::new(HashMap::new())),
            mount_root,
        }
    }
}

#[async_trait]
impl VolumeDriver for XetDriver {
    async fn create(
        State(driver): State<Arc<Self>>,
        Json(request): Json<CreateRequest>,
    ) -> VolumeResponse<Json<NullResponse>> {
        info!(
            "Creating new volume: {} with options: {:?}",
            request.name, request.options
        );
        let mut volume = XetVolume::default();
        for option in request.options {
            volume.try_apply_option(option)?;
        }

        volume.validate()?;

        let mount_path = driver.mount_root.join(Path::new(&request.name));
        volume.mount_path = mount_path.clone();
        if !mount_path.exists() {
            fs::create_dir_all(mount_path)
                .await
                .map_err(VolumeError::FailedIO)?;
        } else {
            warn!("Mountpath: {:?} already exists", mount_path)
        }

        let mut mount_map = driver.mounts.write().await;
        mount_map.insert(request.name, volume);
        Ok(Json(NullResponse {}))
    }

    async fn remove(
        State(driver): State<Arc<Self>>,
        Json(request): Json<RemoveRequest>,
    ) -> VolumeResponse<Json<NullResponse>> {
        info!("Removing volume: {}", request.name);
        let mut mount_map = driver.mounts.write().await;
        mount_map
            .remove(&request.name)
            .ok_or(VolumeError::NotFound)
            .map(|_| Json(NullResponse {}))
    }

    async fn mount(
        State(driver): State<Arc<Self>>,
        Json(request): Json<MountRequest>,
    ) -> VolumeResponse<Json<MountResponse>> {
        info!("Mounting volume: {} with ID: {}", request.name, request.id);
        let mount_map = driver.mounts.read().await;
        let volume = mount_map.get(&request.name).ok_or(VolumeError::NotFound)?;
        info!(
            "Mounting volume: {} (ID: {}) at: {:?}",
            request.name, request.id, volume.mount_path
        );
        let mut git_xet_cmd = process::Command::new("git-xet");
        if !volume.username.is_empty() && !volume.pat.is_empty() {
            git_xet_cmd.env("XET_USER_NAME", &volume.username);
            git_xet_cmd.env("XET_USER_TOKEN", &volume.pat);
        }
        let mut mount_cmd = git_xet_cmd
            .arg("mount")
            .arg("-r")
            .arg(&volume.commit)
            .arg(&volume.repo)
            .arg(&volume.mount_path);
        if volume.writeable {
            mount_cmd = mount_cmd.arg("-w");
        }
        if let Some(ref interval) = volume.watch {
            mount_cmd = mount_cmd.arg("--watch").arg(interval);
        }
        let output = mount_cmd
            .spawn()
            .map_err(VolumeError::FailedIO)?
            .wait_with_output()
            .await
            .map_err(VolumeError::FailedIO)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        info!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        info!("stderr: {}", stderr);
        if !output.status.success() {
            return Err(VolumeError::FailedMount(stderr.to_string()));
        }
        info!("Mounted volume: {} (ID: {})", request.name, request.id);

        return Ok(Json(MountResponse {
            mountpoint: volume.mount_path.as_os_str().to_str().unwrap().to_string(),
        }));
    }

    async fn unmount(
        State(driver): State<Arc<Self>>,
        Json(request): Json<UnmountRequest>,
    ) -> VolumeResponse<Json<NullResponse>> {
        info!("Unmounting volume: {} (id: {})", request.name, request.id);
        let mount_map = driver.mounts.read().await;
        let volume = mount_map.get(&request.name).ok_or(VolumeError::NotFound)?;
        info!(
            "Unmounting volume: {} (ID: {}) at: {:?}",
            request.name, request.id, volume.mount_path
        );
        let output = process::Command::new("umount")
            .arg(&volume.mount_path)
            .output()
            .await
            .map_err(VolumeError::FailedIO)?;
        info!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        info!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        info!("Unmounted volume: {} (ID: {})", request.name, request.id);
        return Ok(Json(NullResponse {}));
    }

    async fn path(
        State(driver): State<Arc<Self>>,
        Json(request): Json<PathRequest>,
    ) -> VolumeResponse<Json<PathResponse>> {
        info!("Getting path for volume: {}", request.name);
        let mount_path = driver.mount_root.join(Path::new(&request.name));
        Ok(Json(PathResponse {
            mountpoint: mount_path.as_os_str().to_str().unwrap().to_string(),
        }))
    }

    async fn get(
        State(driver): State<Arc<Self>>,
        Json(request): Json<GetRequest>,
    ) -> VolumeResponse<Json<GetResponse>> {
        info!("Getting volume: {}", request.name);
        let mount_map = driver.mounts.read().await;
        let volume = mount_map
            .get(&request.name)
            .map(|volume| GetResponse {
                volume: Some(Volume {
                    name: request.name.clone(),
                    mountpoint: volume.mount_path.as_os_str().to_str().unwrap().to_string(),
                    status: Default::default(),
                }),
            })
            .unwrap_or(GetResponse { volume: None });
        return Ok(Json(volume));
    }

    async fn list(State(driver): State<Arc<Self>>) -> VolumeResponse<Json<ListResponse>> {
        info!("Listing volumes");
        let mount_map = driver.mounts.read().await;
        let vols = mount_map
            .iter()
            .map(|(name, vol)| Volume {
                name: name.to_string(),
                mountpoint: vol.mount_path.as_os_str().to_str().unwrap().to_string(),
                status: Default::default(),
            })
            .collect();
        Ok(Json(ListResponse { volumes: vols }))
    }

    async fn capabilities(
        State(_): State<Arc<Self>>,
    ) -> VolumeResponse<Json<CapabilitiesResponse>> {
        Ok(Json(CapabilitiesResponse {
            capabilities: Capability {
                scope: Scope::Local,
            },
        }))
    }
}
