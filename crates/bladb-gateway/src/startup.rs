use crate::local::{LocalGatewayConfig, LocalGatewayFileConfig, OfficialModulesConfig};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const DEFAULT_CONFIG_FILE: &str = "bladb.yml";
const GATEWAY_CONFIG_ENV: &str = "BLADB_GATEWAY_CONFIG";
const RUNTIME_ROLE_ENV: &str = "BLADB_RUNTIME_ROLE";

#[derive(Debug, Clone)]
pub enum GatewayStartup {
    Standalone {
        config_path: PathBuf,
        app: LocalGatewayConfig,
    },
    Cluster {
        config_path: PathBuf,
        role: String,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GatewayStartupError {
    #[error("failed to read config file `{path}`: {reason}")]
    ConfigRead { path: String, reason: String },
    #[error("failed to parse config file `{path}`: {reason}")]
    ConfigParse { path: String, reason: String },
    #[error("unable to find `{0}` from the current directory upward")]
    ConfigNotFound(&'static str),
    #[error("standalone mode requires a `gateway` section in `{0}`")]
    MissingGatewaySection(String),
    #[error("cluster mode requires `runtime.role` in config or `{0}` in env")]
    MissingRuntimeRole(&'static str),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnifiedConfigFile {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    runtime: UnifiedRuntimeSection,
    #[serde(default)]
    modules: UnifiedModulesSection,
    #[serde(default)]
    gateway: Option<LocalGatewayFileConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnifiedModulesSection {
    #[serde(default)]
    official: OfficialModulesConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnifiedRuntimeSection {
    #[serde(default)]
    role: Option<String>,
}

pub fn load_gateway_startup(
    explicit_path: Option<&Path>,
    cwd: &Path,
) -> Result<GatewayStartup, GatewayStartupError> {
    let config_path = resolve_config_path(explicit_path, cwd)?;
    let contents =
        fs::read_to_string(&config_path).map_err(|error| GatewayStartupError::ConfigRead {
            path: config_path.display().to_string(),
            reason: error.to_string(),
        })?;
    let parsed = parse_unified_config(&config_path, &contents)?;

    match parsed.mode.as_deref() {
        Some(mode) if mode.eq_ignore_ascii_case("standalone") => {
            let mut gateway = parsed.gateway.ok_or_else(|| {
                GatewayStartupError::MissingGatewaySection(config_path.display().to_string())
            })?;
            gateway.official = parsed.modules.official;
            let base_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
            let app =
                LocalGatewayConfig::from_file_config(gateway, base_dir).map_err(|reason| {
                    GatewayStartupError::ConfigParse {
                        path: config_path.display().to_string(),
                        reason,
                    }
                })?;
            Ok(GatewayStartup::Standalone { config_path, app })
        }
        _ => {
            let role = parsed
                .runtime
                .role
                .or_else(|| env::var(RUNTIME_ROLE_ENV).ok())
                .ok_or(GatewayStartupError::MissingRuntimeRole(RUNTIME_ROLE_ENV))?;
            Ok(GatewayStartup::Cluster { config_path, role })
        }
    }
}

pub fn discover_bladb_config(start_dir: &Path) -> Option<PathBuf> {
    start_dir
        .ancestors()
        .map(|dir| dir.join(DEFAULT_CONFIG_FILE))
        .find(|path| path.is_file())
}

fn resolve_config_path(
    explicit_path: Option<&Path>,
    cwd: &Path,
) -> Result<PathBuf, GatewayStartupError> {
    if let Some(path) = explicit_path {
        return Ok(path.to_path_buf());
    }

    if let Ok(path) = env::var(GATEWAY_CONFIG_ENV) {
        return Ok(PathBuf::from(path));
    }

    discover_bladb_config(cwd).ok_or(GatewayStartupError::ConfigNotFound(DEFAULT_CONFIG_FILE))
}

fn parse_unified_config(
    path: &Path,
    contents: &str,
) -> Result<UnifiedConfigFile, GatewayStartupError> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => serde_json::from_str::<UnifiedConfigFile>(contents).map_err(|error| {
            GatewayStartupError::ConfigParse {
                path: path.display().to_string(),
                reason: error.to_string(),
            }
        }),
        _ => serde_yaml::from_str::<UnifiedConfigFile>(contents).map_err(|error| {
            GatewayStartupError::ConfigParse {
                path: path.display().to_string(),
                reason: error.to_string(),
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        discover_bladb_config, load_gateway_startup, GatewayStartup, GatewayStartupError,
        RUNTIME_ROLE_ENV,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        temp_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf()
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("bladb-{name}-{nanos}"))
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, contents).expect("write file");
    }

    struct EnvGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvGuard {
        fn clear(key: &'static str) -> Self {
            let original = std::env::var(key).ok();
            // SAFETY: tests serialize access with a global mutex so process env mutation
            // stays deterministic for this module.
            unsafe { std::env::remove_var(key) };
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => {
                    // SAFETY: guarded by the same module-level mutex as setup.
                    unsafe { std::env::set_var(self.key, value) };
                }
                None => {
                    // SAFETY: guarded by the same module-level mutex as setup.
                    unsafe { std::env::remove_var(self.key) };
                }
            }
        }
    }

    #[test]
    fn discovers_bladb_config_from_parent_directory() {
        let root = unique_temp_dir("discover-config");
        let nested = root.join("apps/examples/flash-sale");
        write_file(&root.join("bladb.yml"), "mode: standalone\n");
        fs::create_dir_all(&nested).expect("create nested directory");

        let discovered = discover_bladb_config(&nested).expect("discover config");
        assert_eq!(discovered, root.join("bladb.yml"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn loads_standalone_gateway_from_unified_bladb_config() {
        let _guard = lock_env();
        let _role_guard = EnvGuard::clear(RUNTIME_ROLE_ENV);
        let config_path = workspace_root().join("bladb.yml");

        let startup =
            load_gateway_startup(Some(&config_path), Path::new(".")).expect("load startup");
        match startup {
            GatewayStartup::Standalone { config_path, app } => {
                assert!(config_path.ends_with("bladb.yml"));
                assert_eq!(app.runtimes.len(), 4);
                assert_eq!(app.runtimes[0].name, "flash-sale");
                assert_eq!(app.auth_users.len(), 5);
                assert!(app.official_users.is_some());
                assert_eq!(
                    app.official_users
                        .as_ref()
                        .and_then(|config| config.storage.engine.as_deref()),
                    Some("mysql")
                );
            }
            other => panic!("expected standalone startup, got {other:?}"),
        }
    }

    #[test]
    fn falls_back_to_env_role_when_mode_is_not_standalone() {
        let _guard = lock_env();
        let env_guard = EnvGuard::clear(RUNTIME_ROLE_ENV);
        let root = unique_temp_dir("cluster-role-fallback");
        let config_path = root.join("bladb.yml");
        write_file(&config_path, "runtime: {}\n");
        // SAFETY: guarded by the module-level mutex above.
        unsafe { std::env::set_var(RUNTIME_ROLE_ENV, "gateway") };

        let startup = load_gateway_startup(None, &root).expect("load cluster startup");
        match startup {
            GatewayStartup::Cluster { config_path, role } => {
                assert_eq!(config_path, root.join("bladb.yml"));
                assert_eq!(role, "gateway");
            }
            other => panic!("expected cluster startup, got {other:?}"),
        }

        drop(env_guard);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prefers_runtime_role_from_config_before_env() {
        let _guard = lock_env();
        let _env_guard = EnvGuard::clear(RUNTIME_ROLE_ENV);
        let root = unique_temp_dir("cluster-role-precedence");
        let config_path = root.join("bladb.yml");
        write_file(&config_path, "runtime:\n  role: module-runtime\n");
        // SAFETY: guarded by the module-level mutex above.
        unsafe { std::env::set_var(RUNTIME_ROLE_ENV, "gateway") };

        let startup = load_gateway_startup(None, &root).expect("load cluster startup");
        match startup {
            GatewayStartup::Cluster { role, .. } => {
                assert_eq!(role, "module-runtime");
            }
            other => panic!("expected cluster startup, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn requires_role_outside_standalone_mode() {
        let _guard = lock_env();
        let _role_guard = EnvGuard::clear(RUNTIME_ROLE_ENV);
        let root = unique_temp_dir("missing-role");
        write_file(&root.join("bladb.yml"), "runtime: {}\n");

        let error = load_gateway_startup(None, &root).expect_err("missing runtime role");
        assert_eq!(
            error,
            GatewayStartupError::MissingRuntimeRole(RUNTIME_ROLE_ENV)
        );

        let _ = fs::remove_dir_all(root);
    }
}
