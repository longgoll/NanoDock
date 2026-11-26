pub mod bootstrap;
pub mod distro;
pub mod distro_integration;
pub mod docker_cli;
pub mod error;
pub mod runner;
pub mod traits;
pub mod wslconfig;

pub use bootstrap::{ensure_distro_imported, BootstrapOutcome};
pub use distro::{graceful_hibernate, trim_distro, wake_distro};
pub use distro_integration::{
    list_user_wsl_distros, parse_wsl_list_verbose, set_distro_docker_integration,
};
pub use docker_cli::{
    check_creds_store_content, check_user_docker_config, detect_wincred_available,
    fix_user_docker_creds_store, remove_desktop_creds_store_content, set_user_docker_wincred,
    set_wincred_creds_store_content, CredsStoreWarning,
};
pub use error::WslError;
pub use runner::{decode_wsl_output, SystemWslExecutor};
pub use traits::{CommandOutput, WslExecutor};
pub use wslconfig::{ensure_wslconfig, merge_wslconfig_content, recommended_memory_mb};
