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

# Prevent WSL2 clock drift after host sleep/hibernate.
hwclock -s >/dev/null 2>&1 || true

# Kernel tuning for heavy dev stacks (Elasticsearch, Vite/Webpack, high-concurrency DBs).
if command -v sysctl >/dev/null 2>&1; then
    sysctl -w vm.max_map_count=262144 >/dev/null 2>&1 || true
    sysctl -w fs.inotify.max_user_watches=524288 >/dev/null 2>&1 || true
    sysctl -w fs.inotify.max_user_instances=8192 >/dev/null 2>&1 || true
    sysctl -w net.core.somaxconn=4096 >/dev/null 2>&1 || true
fi

# Enable binfmt_misc for multi-arch container builds/execution when QEMU static is present.
if [ -d /proc/sys/fs/binfmt_misc ]; then
    mountpoint -q /proc/sys/fs/binfmt_misc || mount -t binfmt_misc binfmt_misc /proc/sys/fs/binfmt_misc >/dev/null 2>&1 || true
    if [ -x /usr/bin/qemu-aarch64-static ] && [ ! -e /proc/sys/fs/binfmt_misc/qemu-aarch64 ] && [ -w /proc/sys/fs/binfmt_misc/register ]; then
        echo ':qemu-aarch64:M::\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\xb7\x00:\xff\xff\xff\xff\xff\xff\xff\x00\xff\xff\xff\xff\xff\xff\xff\xff\xfe\xff\xff\xff:/usr/bin/qemu-aarch64-static:F' > /proc/sys/fs/binfmt_misc/register 2>/dev/null || true
    fi
fi

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
