# dabba

Container, uses User Namespaces, CGroups, and [slirp4netns](https://github.com/rootless-containers/slirp4netns)

# Implementation Details

WIP, will be added after project is fully usable:

- [x] User Namespaces

- [x] Networking (with `slirp4netns`)

- [ ] CGroups

- [ ] Handling `setgroups()` inside the container

- [ ] Ability to fetch images from Docker Registry

# Usage

**NOTE:** The `slirp4netns` binary is a runtime dependency

```sh
cargo run -- <path_to_sandboxed_dir>
```

Example:

```sh
testuser@shed dabba.rs $ cargo run                                                                              git@main
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running `target/debug/dabba`
[PID 1648] INFO:dabba::sandbox -- Spawning sandbox!
[PID 1] INFO:dabba::sandbox -- Spawned sandbox!
[PID 1] INFO:dabba::sandbox -- Ensuring that child dies with parent
[PID 1] INFO:dabba::sandbox -- Setting hostname
[PID 1] INFO:dabba::sandbox -- Setting up UID GID mappings
[PID 1] INFO:dabba::sandbox -- UID GID mappings: 0 1000 1
[PID 1] INFO:dabba::sandbox -- Performing the mounting dance
[PID 1] INFO:dabba::sandbox -- Blocking mount propagataion
[PID 1] INFO:dabba::sandbox -- Mounting the container at "/tmp"
[PID 1] INFO:dabba::mount_helper -- Mounting Dev at "dev"
[PID 1] INFO:dabba::mount_helper -- Mounting Proc at "proc"
[PID 1] INFO:dabba::mount_helper -- Mounting Sys at "sys"
[PID 1] INFO:dabba::mount_helper -- Mounting Tmp at "tmp"
[PID 1] INFO:dabba::mount_helper -- Mounting Tmp at "run"
[PID 1] INFO:dabba::sandbox -- Performing pivot root
[PID 1] INFO:dabba::sandbox -- Setting up new session
[PID 1648] INFO:dabba::sandbox -- Successfully initialized sandbox and entered user_cb()
sh: 0: can't access tty; job control turned off
# whoami
root
# hostname
container
# id -u
0
#
```
