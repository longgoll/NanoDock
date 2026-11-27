use shared_types::{ControlRequest, ControlResponse};
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;
use crate::lifecycle::{
    container_exec, container_files, docker_api, image_layers, networks, volume_archive,
};

pub(super) async fn dispatch_docker_request<E: WslExecutor>(
    req: ControlRequest,
    ctx: &AppContext<E>,
) -> ControlResponse {
    match req {
        ControlRequest::ListContainers => match docker_api::list_containers(&ctx.transport).await {
            Ok(items) => ControlResponse::Containers(items),
            Err(err) => err_resp(err),
        },
        ControlRequest::ContainerAction { id, action } => {
            match docker_api::perform_container_action(&ctx.transport, &id, action).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::GetContainerLogs { id, tail } => {
            match docker_api::get_container_logs(&ctx.transport, &id, tail.unwrap_or(200)).await {
                Ok(logs) => ControlResponse::ContainerLogs { logs },
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::InspectContainer { id } => {
            match docker_api::inspect_container(&ctx.transport, &id).await {
                Ok(info) => ControlResponse::ContainerInspect(info),
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::GetContainerStats { id } => {
            match docker_api::get_container_stats(&ctx.transport, &id).await {
                Ok(stats) => ControlResponse::ContainerStats(stats),
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::ExecContainer { id, command } => {
            match container_exec::exec_container(&ctx.transport, &id, &command).await {
                Ok((exit_code, output)) => ControlResponse::ExecOutput { exit_code, output },
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::ListContainerFiles { id, path } => {
            match container_files::list_container_files(&ctx.transport, &id, &path).await {
                Ok(entries) => ControlResponse::ContainerFiles(entries),
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::ReadContainerFile { id, path } => {
            match container_files::read_container_file(&ctx.transport, &id, &path).await {
                Ok((content, truncated)) => ControlResponse::ContainerFileContent {
                    path,
                    content,
                    truncated,
                },
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::RunContainer(spec) => {
            match docker_api::run_container(&ctx.transport, &spec).await {
                Ok(id) => ControlResponse::ContainerCreated { id },
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::ListImages => match docker_api::list_images(&ctx.transport).await {
            Ok(items) => ControlResponse::Images(items),
            Err(err) => err_resp(err),
        },
        ControlRequest::PullImage { reference } => {
            match docker_api::pull_image(&ctx.transport, &reference).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::RemoveImage { id } => {
            match docker_api::remove_image(&ctx.transport, &id).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::PruneImages => match docker_api::prune_images(&ctx.transport).await {
            Ok(reclaimed_bytes) => ControlResponse::PruneResult { reclaimed_bytes },
            Err(err) => err_resp(err),
        },
        ControlRequest::GetBuildCache => {
            match docker_api::get_build_cache_summary(&ctx.transport).await {
                Ok(summary) => ControlResponse::BuildCache(summary),
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::PruneBuildCache => {
            match docker_api::prune_build_cache(&ctx.transport).await {
                Ok(reclaimed_bytes) => ControlResponse::PruneResult { reclaimed_bytes },
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::InspectImageLayers { id } => {
            match image_layers::inspect_image_layers(&ctx.transport, &id).await {
                Ok(layers) => ControlResponse::ImageLayers(layers),
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::ListVolumes => match docker_api::list_volumes(&ctx.transport).await {
            Ok(items) => ControlResponse::Volumes(items),
            Err(err) => err_resp(err),
        },
        ControlRequest::CreateVolume { name, driver } => {
            match docker_api::create_volume(&ctx.transport, &name, driver.as_deref()).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::RemoveVolume { name } => {
            match docker_api::remove_volume(&ctx.transport, &name).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::PruneVolumes => match docker_api::prune_volumes(&ctx.transport).await {
            Ok(reclaimed_bytes) => ControlResponse::PruneResult { reclaimed_bytes },
            Err(err) => err_resp(err),
        },
        ControlRequest::ExportVolume { name, dest_path } => {
            let distro = ctx.config.read().await.distro_name.clone();
            match volume_archive::export_volume(ctx.wsl.as_ref(), &distro, &name, &dest_path).await
            {
                Ok(()) => ControlResponse::Ack,
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::ImportVolume { name, src_path } => {
            let distro = ctx.config.read().await.distro_name.clone();
            match volume_archive::import_volume(ctx.wsl.as_ref(), &distro, &name, &src_path).await
            {
                Ok(()) => ControlResponse::Ack,
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::ListNetworks => match networks::list_networks(&ctx.transport).await {
            Ok(items) => ControlResponse::Networks(items),
            Err(err) => err_resp(err),
        },
        ControlRequest::CreateNetwork {
            name,
            driver,
            internal,
        } => {
            match networks::create_network(&ctx.transport, &name, driver.as_deref(), internal)
                .await
            {
                Ok(()) => ControlResponse::Ack,
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::RemoveNetwork { id } => {
            match networks::remove_network(&ctx.transport, &id).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => err_resp(err),
            }
        }
        ControlRequest::PruneNetworks => match networks::prune_networks(&ctx.transport).await {
            Ok(reclaimed_bytes) => ControlResponse::PruneResult { reclaimed_bytes },
            Err(err) => err_resp(err),
        },
        _ => ControlResponse::Error {
            message: "unsupported docker control request".to_string(),
        },
    }
}

fn err_resp(err: anyhow::Error) -> ControlResponse {
    ControlResponse::Error {
        message: err.to_string(),
    }
}
