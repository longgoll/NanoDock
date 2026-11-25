#!/bin/sh
set -eu

DOCKER_SOCK="/var/run/docker.sock"
SHARED_DIR="/mnt/wsl/nanodock"
SHARED_SOCK="${SHARED_DIR}/docker.sock"
CONTAINERD_SOCK="/run/containerd/containerd.sock"
DOCKER_PID_FILE="/var/run/docker.pid"
LOG_DIR="/var/log/nanodock"

# Fast-path idempotency: if dockerd is already alive and socket exists, exit 0.
if [ -f "${DOCKER_PID_FILE}" ] && kill -0 "$(cat "${DOCKER_PID_FILE}")" 2>/dev/null && [ -S "${DOCKER_SOCK}" ]; then
    exit 0
fi

mkdir -p /proc /sys /dev/pts /sys/fs/cgroup /var/run /run/containerd "${SHARED_DIR}" "${LOG_DIR}"

mountpoint -q /proc || mount -t proc proc /proc
mountpoint -q /sys || mount -t sysfs sysfs /sys
mountpoint -q /dev/pts || mount -t devpts devpts /dev/pts
mountpoint -q /sys/fs/cgroup || mount -t cgroup2 none /sys/fs/cgroup

# Remove stale sockets from any previous ungraceful termination.
rm -f "${DOCKER_SOCK}" "${SHARED_SOCK}" "${CONTAINERD_SOCK}"

# Start containerd in a detached session so it survives when /init.sh exits.
if ! pgrep -x containerd >/dev/null 2>&1; then
    setsid containerd </dev/null >/var/log/nanodock/containerd.log 2>&1 &
fi

# Wait up to 10s (100 * 100ms) for containerd socket.
i=0
while [ ! -S "${CONTAINERD_SOCK}" ]; do
    i=$((i + 1))
    if [ "${i}" -ge 100 ]; then
        echo "Timed out waiting for containerd socket" >&2
        exit 1
    fi
    sleep 0.1
done

# Start dockerd in a detached session so it survives when /init.sh exits.
setsid dockerd --config-file=/etc/docker/daemon.json </dev/null >/var/log/nanodock/dockerd.log 2>&1 &

# Wait up to 10s (100 * 100ms) for Docker socket.
i=0
while [ ! -S "${DOCKER_SOCK}" ]; do
    i=$((i + 1))
    if [ "${i}" -ge 100 ]; then
        echo "Timed out waiting for dockerd socket" >&2
        exit 1
    fi
    sleep 0.1
done

# Allow cross-distro access from other WSL2 distros (e.g., Ubuntu, Dev Containers).
if [ -S "${SHARED_SOCK}" ]; then
    chmod 0666 "${SHARED_SOCK}" || true
fi
