# dabba

Container, uses User Namespaces, CGroups, and [slirp4netns](https://github.com/rootless-containers/slirp4netns)

# Implementation Details

WIP

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

- [ ] Handling whiteout files in layers

# Usage

**NOTE:** The `slirp4netns` binary is a runtime dependency

```sh
cargo run -- <path_to_sandboxed_dir>
```

Example (extracts the rootfs from an OCI image pulled via Docker, Rust logic is WIP):

```sh
$ mkdir root
$ cd root
$ docker pull debian:latest
$ docker save debian:latest > debian.tar
$ tar xf debian.tar
$ mkdir extracted; cd extracted
$ tar xf ../*/layer.tar
```

Note that Ctrl + C would likely kill the container as a new session using `setsid` is created, but no proxy PTY is set up. As a work around for this, `tmux` can be used inside the container.

```sh
testuser@shed dabba.rs $ cargo run -- root/extracted
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running `target/debug/dabba root/extracted`
[PID 23882] INFO:dabba::sandbox -- Spawning sandbox!
[PID 23882] INFO:dabba::sandbox -- Launched sandbox with pid 23884
[PID 23882] INFO:dabba::idmap_helper -- Executing newuidmap with cmdline ["23884", "0", "1000", "1", "1", "100000", "65536"]
[PID 23882] INFO:dabba::idmap_helper -- Executing newgidmap with cmdline ["23884", "0", "1000", "1", "1", "100000", "65536"]
[PID 1] INFO:dabba::sandbox -- Received success event from parent, continuing setup
[PID 1] INFO:dabba::sandbox -- Spawned sandbox!
[PID 1] INFO:dabba::sandbox -- Ensuring that child dies with parent
[PID 1] INFO:dabba::sandbox -- Setting hostname
[PID 1] INFO:dabba::sandbox -- Performing the mounting dance
[PID 1] INFO:dabba::sandbox -- Blocking mount propagataion
[PID 1] INFO:dabba::sandbox -- Mounting the container at "/tmp"
[PID 1] INFO:dabba::mount_helper -- Mounting Dev at "dev"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/full" to "dev/full"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/null" to "dev/null"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/random" to "dev/random"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/tty" to "dev/tty"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/urandom" to "dev/urandom"
[PID 1] INFO:dabba::mount_helper -- Binding "/dev/zero" to "dev/zero"
[PID 1] INFO:dabba::mount_helper -- Creating /dev stdio symlinks
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
sh: 0: can't access tty; job control turned off
# id -u
0
# whoami
root
# hostname
container
# apt update && apt upgrade -y && apt install -y psmisc
Hit:1 http://deb.debian.org/debian bookworm InRelease
Hit:2 http://deb.debian.org/debian bookworm-updates InRelease
Hit:3 http://deb.debian.org/debian-security bookworm-security InRelease
Reading package lists... Done
Building dependency tree... Done
Reading state information... Done
All packages are up to date.
Reading package lists... Done
Building dependency tree... Done
Reading state information... Done
Calculating upgrade... Done
0 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.
Reading package lists... Done
Building dependency tree... Done
Reading state information... Done
The following NEW packages will be installed:
  psmisc
0 upgraded, 1 newly installed, 0 to remove and 0 not upgraded.
Need to get 259 kB of archives.
After this operation, 931 kB of additional disk space will be used.
Get:1 http://deb.debian.org/debian bookworm/main amd64 psmisc amd64 23.6-1 [259 kB]
Fetched 259 kB in 0s (598 kB/s)
perl: warning: Setting locale failed.
perl: warning: Please check that your locale settings:
	LANGUAGE = (unset),
	LC_ALL = (unset),
	LANG = "en_US.UTF-8"
    are supported and installed on your system.
perl: warning: Falling back to the standard locale ("C").
debconf: delaying package configuration, since apt-utils is not installed
Selecting previously unselected package psmisc.
(Reading database ... 6933 files and directories currently installed.)
Preparing to unpack .../psmisc_23.6-1_amd64.deb ...
Unpacking psmisc (23.6-1) ...
Setting up psmisc (23.6-1) ...
# TERM=xterm pstree
dabba---sh---pstree
# echo $$
2
```
