import { invoke } from "@tauri-apps/api/core";

export type EngineState = "stopped" | "starting" | "running" | "stopping" | "error";

export type ContainerActionKind = "start" | "stop" | "restart" | "remove";

export interface WslConfigSpec {
  memory_mb: number;
  swap_mb: number;
  sparse_vhd: boolean;
  mirrored_networking: boolean;
}

export interface EngineConfig {
  distro_name: string;
  idle_timeout_secs: number | null;
  auto_start_on_login: boolean;
  auto_trim_on_events: boolean;
  wsl: WslConfigSpec;
}

export interface ContainerSummary {
  id: string;
  names: string[];
  image: string;
  state: string;
  status: string;
  created: number;
  ports: string[];
  compose_project: string | null;
}

export interface ImageSummary {
  id: string;
  repo_tags: string[];
  size: number;
  created: number;
  in_use: boolean;
  used_by: string[];
}

export interface VolumeSummary {
  name: string;
  driver: string;
  mountpoint: string;
  created_at: string | null;
  in_use: boolean;
  used_by: string[];
}

export interface NetworkSummary {
  id: string;
  name: string;
  driver: string;
  scope: string;
  internal: boolean;
  in_use: boolean;
  used_by: string[];
}

export interface ContainerMount {
  mount_type: string;
  source: string;
  destination: string;
  mode: string;
}

export interface ContainerInspect {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  created: string;
  restart_policy: string;
  cmd: string[];
  entrypoint: string[];
  env: string[];
  mounts: ContainerMount[];
  networks: string[];
  ip_address: string;
  ports: string[];
}

export interface ContainerStats {
  id: string;
  cpu_percent: number;
  mem_usage_bytes: number;
  mem_limit_bytes: number;
}

export interface RunContainerSpec {
  image: string;
  name: string | null;
  ports: string[];
  env: string[];
}

export interface ImageLayer {
  id: string;
  created: number;
  created_by: string;
  size: number;
  comment: string;
}

export interface ContainerFileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  mode: string;
  modified: number;
}

export interface WslDistroInfo {
  name: string;
  state: string;
  version: number;
  is_default: boolean;
  integrated: boolean;
}

export interface EngineMetrics {
  hibernate_count: number;
  total_reclaimed_bytes: number;
  uptime_secs: number;
}

export interface DoctorCheckItem {
  name: string;
  level: "ok" | "warn" | "fail";
  detail: string;
}

export interface EnvironmentStatus {
  wsl_installed: boolean;
  wsl_status_detail: string;
  install_dir: string;
  nanodock_cli_available: boolean;
  nanodock_in_path: boolean;
  docker_shim_active: boolean;
  external_docker_path: string | null;
  creds_store_warning: string | null;
  wincred_available: boolean;
}

export type ControlResponse =
  | {
      status: "status";
      data: {
        state: EngineState;
        running_containers: number;
        vhdx_size_bytes: number | null;
      };
    }
  | { status: "config"; data: EngineConfig }
  | { status: "engine_logs"; data: { daemon_log: string; dockerd_log: string } }
  | { status: "engine_metrics"; data: EngineMetrics }
  | { status: "wsl_distros"; data: WslDistroInfo[] }
  | { status: "containers"; data: ContainerSummary[] }
  | { status: "container_logs"; data: { logs: string } }
  | { status: "container_inspect"; data: ContainerInspect }
  | { status: "container_stats"; data: ContainerStats }
  | { status: "exec_output"; data: { exit_code: number; output: string } }
  | { status: "container_files"; data: ContainerFileEntry[] }
  | {
      status: "container_file_content";
      data: { path: string; content: string; truncated: boolean };
    }
  | { status: "container_created"; data: { id: string } }
  | { status: "images"; data: ImageSummary[] }
  | { status: "image_layers"; data: ImageLayer[] }
  | { status: "volumes"; data: VolumeSummary[] }
  | { status: "networks"; data: NetworkSummary[] }
  | { status: "prune_result"; data: { reclaimed_bytes: number } }
  | { status: "ack" }
  | { status: "error"; data: { message: string } };

export const daemonApi = {
  getStatus: () => invoke<ControlResponse>("get_engine_status"),
  startEngine: () => invoke<ControlResponse>("start_engine"),
  stopEngine: () => invoke<ControlResponse>("stop_engine"),
  trimDisk: () => invoke<ControlResponse>("trim_disk"),
  getConfig: () => invoke<ControlResponse>("get_config"),
  updateConfig: (config: EngineConfig) => invoke<ControlResponse>("update_config", { config }),
  listContainers: () => invoke<ControlResponse>("list_containers"),
  containerAction: (id: string, action: ContainerActionKind) =>
    invoke<ControlResponse>("container_action", { id, action }),
  getContainerLogs: (id: string, tail = 200) =>
    invoke<ControlResponse>("get_container_logs", { id, tail }),
  listImages: () => invoke<ControlResponse>("list_images"),
  pullImage: (reference: string) => invoke<ControlResponse>("pull_image", { reference }),
  removeImage: (id: string) => invoke<ControlResponse>("remove_image", { id }),
  pruneImages: () => invoke<ControlResponse>("prune_images"),
  listVolumes: () => invoke<ControlResponse>("list_volumes"),
  createVolume: (name: string, driver?: string) =>
    invoke<ControlResponse>("create_volume", { name, driver: driver ?? null }),
  removeVolume: (name: string) => invoke<ControlResponse>("remove_volume", { name }),
  pruneVolumes: () => invoke<ControlResponse>("prune_volumes"),
  listNetworks: () => invoke<ControlResponse>("list_networks"),
  createNetwork: (name: string, driver?: string, internal = false) =>
    invoke<ControlResponse>("create_network", { name, driver: driver ?? null, internal }),
  removeNetwork: (id: string) => invoke<ControlResponse>("remove_network", { id }),
  pruneNetworks: () => invoke<ControlResponse>("prune_networks"),
  inspectContainer: (id: string) => invoke<ControlResponse>("inspect_container", { id }),
  getContainerStats: (id: string) => invoke<ControlResponse>("get_container_stats", { id }),
  runContainer: (spec: RunContainerSpec) => invoke<ControlResponse>("run_container", { spec }),
  openContainerShell: (id: string) => invoke<void>("open_container_shell", { id }),
  openLocalhostPort: (port: number) => invoke<void>("open_localhost_port", { port }),
  execContainer: (id: string, command: string[]) =>
    invoke<ControlResponse>("exec_container", { id, command }),
  listContainerFiles: (id: string, path: string) =>
    invoke<ControlResponse>("list_container_files", { id, path }),
  readContainerFile: (id: string, path: string) =>
    invoke<ControlResponse>("read_container_file", { id, path }),
  inspectImageLayers: (id: string) => invoke<ControlResponse>("inspect_image_layers", { id }),
  exportVolume: (name: string, destPath: string) =>
    invoke<ControlResponse>("export_volume", { name, destPath }),
  importVolume: (name: string, srcPath: string) =>
    invoke<ControlResponse>("import_volume", { name, srcPath }),
  getEngineLogs: (lines = 150) => invoke<ControlResponse>("get_engine_logs", { lines }),
  getEngineMetrics: () => invoke<ControlResponse>("get_engine_metrics"),
  listWslDistros: () => invoke<ControlResponse>("list_wsl_distros"),
  setWslDistroIntegration: (distro: string, enabled: boolean) =>
    invoke<ControlResponse>("set_wsl_distro_integration", { distro, enabled }),
};

export const systemApi = {
  getEnvironmentStatus: () => invoke<EnvironmentStatus>("get_environment_status"),
  installWsl2: () => invoke<string>("install_wsl2"),
  setUserPathIntegration: (enabled: boolean) =>
    invoke<EnvironmentStatus>("set_user_path_integration", { enabled }),
  setDockerCliShim: (enabled: boolean) =>
    invoke<EnvironmentStatus>("set_docker_cli_shim", { enabled }),
  fixDockerCredsStore: () => invoke<EnvironmentStatus>("fix_docker_creds_store"),
  setWincredCredsStore: () => invoke<EnvironmentStatus>("set_wincred_creds_store"),
  runDoctorChecks: () => invoke<DoctorCheckItem[]>("run_doctor_checks"),
  pickArchiveFile: (mode: "save" | "open", defaultName: string) =>
    invoke<string | null>("pick_archive_file", { mode, defaultName }),
};
