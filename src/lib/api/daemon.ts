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
}

export interface VolumeSummary {
  name: string;
  driver: string;
  mountpoint: string;
  created_at: string | null;
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
  | { status: "containers"; data: ContainerSummary[] }
  | { status: "container_logs"; data: { logs: string } }
  | { status: "images"; data: ImageSummary[] }
  | { status: "volumes"; data: VolumeSummary[] }
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
  removeVolume: (name: string) => invoke<ControlResponse>("remove_volume", { name }),
  pruneVolumes: () => invoke<ControlResponse>("prune_volumes"),
};
