# dabba

Container, uses User Namespaces, CGroups, and [slirp4netns](https://github.com/rootless-containers/slirp4netns)

# Implementation

See the `docs/` directory

- [x] User Namespaces

- [x] Networking (with `slirp4netns`)

  - [x] Outgoing Connections

  - [x] Port Forwarding to expose container

- [ ] CGroups (needs integration for delegation mechanisms)

- [x] Clear Environment Variables

- [x] Handling `setgroups()` inside the container

- [ ] Proper PTY handling (use `tmux` inside the container for now)

- [x] Ability to fetch images from Docker Registry

- [x] Image storage

- [ ] Handling whiteout files in layers (https://github.com/containers/storage/blob/main/pkg/archive/archive_linux.go#L115)

# Dependencies

- [`slirp4netns`](https://github.com/rootless-containers/slirp4netns)

- `new{uid,gid}map`: Likely provided by the `shadow` or `uidmap` package, depending upon distribution

# Usage

```sh
cargo run -- <image_name> <tag>
```

```sh
testuser@shed dabba.rs $ cargo run -- alpine latest
    Finished dev [unoptimized + debuginfo] target(s) in 0.10s
     Running `target/debug/dabba alpine latest`
[PID 23690] INFO:dabba -- Creating base directory
[PID 23690] INFO:dabba -- Using image: 'alpine', tag: 'latest'
[PID 23690] INFO:dabba -- Image Manifest: Manifest {
    schema_version: 2,
    media_type: "application/vnd.docker.distribution.manifest.v2+json",
    config: ManifestConfig {
        media_type: "application/vnd.docker.container.image.v1+json",
        size: 1472,
        digest: "sha256:8ca4688f4f356596b5ae539337c9941abc78eda10021d35cbc52659c74d9b443",
    },
    layers: [
        ManifestConfig {
            media_type: "application/vnd.docker.image.rootfs.diff.tar.gzip",
            size: 3401967,
            digest: "sha256:96526aa774ef0126ad0fe9e9a95764c5fc37f409ab9e97021e7b4775d82bf6fa",
        },
    ],
}
[PID 23690] INFO:dabba -- Image Config: ImageConfig {
    architecture: "amd64",
    os: "linux",
    config: ImageConfigRuntime {
        exposed_ports: None,
        env: [
            "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        ],
        working_dir: Some(
            "",
        ),
        cmd: [
            "/bin/sh",
        ],
        entrypoint: None,
        stop_signal: None,
    },
}
[PID 23690] INFO:dabba::storage -- Downloading layer sha256:96526aa774ef0126ad0fe9e9a95764c5fc37f409ab9e97021e7b4775d82bf6fa
[PID 23690] INFO:dabba::storage -- Successfully extracted layer sha256:96526aa774ef0126ad0fe9e9a95764c5fc37f409ab9e97021e7b4775d82bf6fa
[PID 23690] INFO:dabba::sandbox -- Spawning sandbox!
[PID 23690] INFO:dabba::sandbox -- Launched sandbox with pid 23758
[PID 23690] INFO:dabba::idmap_helper -- Executing newuidmap with cmdline ["23758", "0", "1000", "1", "1", "100000", "65536"]
[PID 23690] INFO:dabba::idmap_helper -- Executing newgidmap with cmdline ["23758", "0", "1000", "1", "1", "100000", "65536"]
[PID 1] INFO:dabba::sandbox -- Received success event from parent, continuing setup
[PID 1] INFO:dabba::sandbox -- Spawned sandbox!
[PID 1] INFO:dabba::sandbox -- Ensuring that child dies with parent
[PID 1] INFO:dabba::sandbox -- Setting hostname
[PID 1] INFO:dabba::sandbox -- Performing the mounting dance
[PID 1] INFO:dabba::sandbox -- Using "/tmp/dabba/dabba-overlay" inside sandbox as overlay path
[PID 1] INFO:dabba::mount_helper -- Mounting Tmp at "/tmp/dabba/dabba-overlay"
[PID 1] INFO:dabba::sandbox -- Blocking mount propagataion
[PID 1] INFO:dabba::sandbox -- Mounting the layers at "/tmp/dabba/dabba-overlay"
[PID 1] INFO:dabba::mount_helper -- OverlayFS args: lowerdir=/tmp/dabba/storage/sha256\:96526aa774ef0126ad0fe9e9a95764c5fc37f409ab9e97021e7b4775d82bf6fa,upperdir=/tmp/dabba/dabba-overlay/upper,workdir=/tmp/dabba/dabba-overlay/work
[PID 1] INFO:dabba::mount_helper -- Mounting Dev at "dev"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/full" to "dev/full"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/null" to "dev/null"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/random" to "dev/random"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/tty" to "dev/tty"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/urandom" to "dev/urandom"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/zero" to "dev/zero"
[PID 1] INFO:dabba::mount_helper -- Creating /dev stdio symlinks
[PID 1] INFO:dabba::mount_helper -- Creating /dev/fd
[PID 1] INFO:dabba::mount_helper -- Creating shm
[PID 1] INFO:dabba::mount_helper -- Creating ptmx symlink
[PID 1] INFO:dabba::mount_helper -- Mounting devpts
[PID 1] INFO:dabba::mount_helper -- Mounting Proc at "proc"
[PID 1] INFO:dabba::mount_helper -- Mounting Sys at "sys"
[PID 1] INFO:dabba::mount_helper -- Mounting Tmp at "tmp"
[PID 1] INFO:dabba::mount_helper -- Mounting Tmp at "run"
[PID 1] INFO:dabba::sandbox -- Performing pivot root
[PID 1] INFO:dabba::sandbox -- Setting up new session
[PID 1] INFO:dabba::sandbox -- Dropping privileges
[PID 1] INFO:dabba::sandbox -- Closing FDs
[PID 1] INFO:dabba::util -- Not closing stdin
[PID 1] INFO:dabba::util -- Not closing stdout
[PID 1] INFO:dabba::util -- Not closing stderr
[PID 1] INFO:dabba::util -- Closing FD 3
[PID 1] INFO:dabba::util -- Closing FD 4
[PID 1] INFO:dabba::util -- Closing FD 5
[PID 1] INFO:dabba::util -- Closing FD 6
[PID 1] INFO:dabba::util -- Not closing dir fd 7
[PID 1] INFO:dabba::util -- Clearing variable: CARGO
[PID 1] INFO:dabba::util -- Clearing variable: CARGO_HOME
[PID 1] INFO:dabba::util -- Clearing variable: CARGO_MANIFEST_DIR
...
[PID 1] INFO:dabba::util -- Clearing variable: WAYLAND_DISPLAY
[PID 1] INFO:dabba::util -- Setting variable 'HOME' to '/root'
[PID 1] INFO:dabba::util -- Setting variable 'TERM' to 'xterm'
[PID 1] INFO:dabba::util -- Setting variable 'PATH' to '/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin'
[PID 1] INFO:dabba::util -- Setting variable 'PATH' to '/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin'
[PID 1] INFO:dabba::sandbox -- Launching program '/bin/sh' with args: []
/bin/sh: can't access tty; job control turned off
/ # whoami
root
/ # env
SHLVL=1
HOME=/root
TERM=xterm
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
PWD=/
/ # ip a
1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN qlen 1000
    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00
    inet 127.0.0.1/8 scope host lo
       valid_lft forever preferred_lft forever
    inet6 ::1/128 scope host 
       valid_lft forever preferred_lft forever
2: ip_vti0@NONE: <NOARP> mtu 1480 qdisc noop state DOWN qlen 1000
    link/ipip 0.0.0.0 brd 0.0.0.0
3: sit0@NONE: <NOARP> mtu 1480 qdisc noop state DOWN qlen 1000
    link/sit 0.0.0.0 brd 0.0.0.0
4: tap0: <BROADCAST,UP,LOWER_UP> mtu 1500 qdisc pfifo_fast state UNKNOWN qlen 1000
    link/ether fe:7c:24:98:65:d3 brd ff:ff:ff:ff:ff:ff
    inet 10.0.2.100/24 brd 10.0.2.255 scope global tap0
       valid_lft forever preferred_lft forever
    inet6 fe80::fc7c:24ff:fe98:65d3/64 scope link 
       valid_lft forever preferred_lft forever
/ # echo nameserver 1.1.1.1 | tee /etc/resolv.conf
nameserver 1.1.1.1
/ # apk add curl
fetch https://dl-cdn.alpinelinux.org/alpine/v3.18/main/x86_64/APKINDEX.tar.gz
fetch https://dl-cdn.alpinelinux.org/alpine/v3.18/community/x86_64/APKINDEX.tar.gz
(1/7) Installing ca-certificates (20230506-r0)
(2/7) Installing brotli-libs (1.0.9-r14)
(3/7) Installing libunistring (1.1-r1)
(4/7) Installing libidn2 (2.3.4-r1)
(5/7) Installing nghttp2-libs (1.55.1-r0)
(6/7) Installing libcurl (8.3.0-r0)
(7/7) Installing curl (8.3.0-r0)
Executing busybox-1.36.1-r2.trigger
Executing ca-certificates-20230506-r0.trigger
OK: 12 MiB in 22 packages
/ # curl https://httpbin.org/get
{
  "args": {}, 
  "headers": {
    "Accept": "*/*", 
    "Host": "httpbin.org", 
    "User-Agent": "curl/8.3.0", 
    "X-Amzn-Trace-Id": "Root=1-651ebeb4-3b24654205ff53323044df91"
  }, 
  "origin": "XX.XXX.XXX.XX",
  "url": "https://httpbin.org/get"
}
```
