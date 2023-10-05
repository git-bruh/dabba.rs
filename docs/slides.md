# Container Internals

- What even is _docker_?

- cgroups

- namespaces

- image layers? union filesystems?

---

# An Example

What components are involved in making `docker run` work?

```bash
docker run --rm alpine:latest echo Hello!
```

- Docker CLI

- Docker Daemon

- `containerd`: Managing `runc`, Pushing & Pulling images to/from the Container registry, ...

- `runc`: Actually runs the "container" by setting up cgroups, namespaces, [PTYs](https://github.com/opencontainers/runc/blob/main/docs/terminals.md), etc.

![Diagram](docker-components.png)

---

# Digging Deeper

So, what exactly is isolated from the "host" in the container?

`docker run --rm -it alpine:latest`

- Rootfs (duh)

- PIDs

```bash
/ # echo $$
1
/ # ps -A
PID   USER     TIME  COMMAND
    1 root      0:00 /bin/sh
    7 root      0:00 ps -A
```

- Pseudo-Filesystems (/dev, /proc, etc.)

```bash
/ # ls /dev
console  full     null     pts      shm      stdin    tty      zero
fd       mqueue   ptmx     random   stderr   stdout   urandom
```

```bash
/ # ls /proc
1             cmdline       dma           iomem         kpagecgroup   modules       slabinfo      timer_list
15            config.gz     driver        ioports       kpagecount    mounts        softirqs      tty
acpi          consoles      execdomains   irq           kpageflags    mtrr          stat          uptime
asound        cpuinfo       fb            kallsyms      loadavg       net           swaps         version
buddyinfo     crypto        filesystems   key-users     locks         pagetypeinfo  sys           vmallocinfo
bus           devices       fs            keys          meminfo       partitions    sysvipc       vmstat
cgroups       diskstats     interrupts    kmsg          misc          self          thread-self   zoneinfo
```

---

- Network

```bash
/ # ip a
1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN qlen 1000
    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00
    inet 127.0.0.1/8 scope host lo
       valid_lft forever preferred_lft forever
2: ip_vti0@NONE: <NOARP> mtu 1480 qdisc noop state DOWN qlen 1000
    link/ipip 0.0.0.0 brd 0.0.0.0
3: sit0@NONE: <NOARP> mtu 1480 qdisc noop state DOWN qlen 1000
    link/sit 0.0.0.0 brd 0.0.0.0
33: eth0@if34: <BROADCAST,MULTICAST,UP,LOWER_UP,M-DOWN> mtu 1500 qdisc noqueue state UP 
    link/ether 02:42:ac:11:00:02 brd ff:ff:ff:ff:ff:ff
    inet 172.17.0.2/16 brd 172.17.255.255 scope global eth0
       valid_lft forever preferred_lft forever
```

---

# Reinventing It!

We could use the `unshare` command to do, well, basically everything that's going to be described in the slides, but we'll be writing some code for better understanding: [code](toy-demo/src)

Now, with this code, we have a semi-isolated process running, with it's own PID and Network namespaces

```bash
$ ps -A
PID   USER     TIME  COMMAND
    1 65534     0:00 sh
    2 65534     0:00 ps -A
$ ls /proc
1             devices       kallsyms      mounts        thread-self
3             diskstats     key-users     mtrr          timer_list
acpi          dma           keys          net           tty
asound        driver        kmsg          pagetypeinfo  uptime
buddyinfo     execdomains   kpagecgroup   partitions    version
bus           fb            kpagecount    self          vmallocinfo
cgroups       filesystems   kpageflags    slabinfo      vmstat
cmdline       fs            loadavg       softirqs      zoneinfo
config.gz     interrupts    locks         stat
consoles      iomem         meminfo       swaps
cpuinfo       ioports       misc          sys
crypto        irq           modules       sysvipc
```

```bash
$ ip a
1: lo: <LOOPBACK> mtu 65536 qdisc noop state DOWN group default qlen 1000
    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00
2: ip_vti0@NONE: <NOARP> mtu 1480 qdisc noop state DOWN group default qlen 1000
    link/ipip 0.0.0.0 brd 0.0.0.0
3: sit0@NONE: <NOARP> mtu 1480 qdisc noop state DOWN group default qlen 1000
    link/sit 0.0.0.0 brd 0.0.0.0
$ curl https://duckduckgo.com
curl: (6) Could not resolve host: duckduckgo.com
```

---

# Something more usable

That was a fine toy example, but we would like something more... usable. For starters, let's make ourselves root inside the namespace, which would be needed to say, install packages in a container, bind to a privileged port, etc.

For this, user namespaces allow us to create UID and GID mappings, wherein a proxy UID/GID inside the container can have privileges equal to a UID/GID on the host:

- `/proc/<pid>/uid_map`

- `/proc/<pid>/gid_map`

Root to user mapping: `0 1000 1`

User to user mapping: `1 100000 65536`

| Host UID | Namespace UID |
|----------|---------------|
|   1000   |       0       |
|  100000  |       1       |
|  100001  |       2       |

[code](toy-demo/src)

---

# Networking

We'd like one more thing, how do we access the internet from the container? `slirp4netns`!

Usage: `slirp4netns -c <PID> tap0`

- Creates a subprocess to enter into the existing namespace

- Creates a TAP device and sets up the namespace's routing table to route all traffic through it

- Subprocess passes the FD handle of the TAP device back to the main process over a `socketpair` and exits

```
  ┌─────────────────┐
  │                 │
  │ ┌─────────────┐ │ fork()     ┌───────────────┐
  │ │   (Child)   ◄─┼────────────┤  slirp4netns  │
  │ └──────┬──────┘ │ setns(X)   └───────────────┘
  │        │        │
  │        │        │
  │ ┌──────▼──────┐ │
  │ │TAP /dev/tap0│ │
  │ └─────────────┘ │
  │                 │
  └─────────────────┘
  Container Namespace
(X = /proc/<PID>/ns/net)
```

---

# Docker Images

- Collection of "layers"

- Layers speeds up incremental builds

```dockerfile
# Base image (which itself might contain multiple layers)
FROM debian:12.1
# Layer N + 1
RUN apt update -y
# Layer N + 2
RUN apt install -y htop
```

Now, what _is_ a layer?

```sh
# Build the image
$ docker build . -t test:latest
# Crack open the image
$ docker save test:latest > image.tar
```

```json
[
  {
    "Config": "19e77748741e658e71345136c33e63f0460426c6cdfbe8d520244c3301b75e5a.json",
    "RepoTags": [
      "test:latest"
    ],
    "Layers": [
      # FROM ...
      "3ed2e39e6cfe97a8053c0927d6cb63b0ad1f0a4e15d1833861bcf89e71ef26a5/layer.tar",
      # RUN ...
      "38260da32abbe4c22c349d72bbdb9b4981846457d440e50148168a20e308ff27/layer.tar",
      # RUN ...
      "1f197e68078a6ebb1602708ddd3de2412935d6e44d826132717be0dda0c432cf/layer.tar"
    ]
  }
]
```

---

# OverlayFS

So, each layer is a tar'd up _`diff`_ of the changes to be applied to the base layer, and this concept maps 1:1 to a union filesystem.

![OverlayFS](overlay_constructs.jpg)

- Each intermediate layer is overlayed on top of the base layer to form the "lower" directory

- An external read-write directory forms the "upper" directory, where file modifications can take place

```sh
# mount -t overlay overlay -o lowerdir=layer3:layer2:layer1,upperdir=<PREFIX>/upper,workdir=<PREFIX>/work <PREFIX>/merged
```

---

# Putting it all together

---

# A note on cgroups

cgroups are mainly useful for controlling how much of a resource a process can use, and are hence not essential for containerization, but are a nice to have.

Example

```sh
```

---

# References

- https://iximiuz.com/en/posts/implementing-container-runtime-shim

- https://rootlesscontaine.rs

- https://github.com/containers/bubblewrap
