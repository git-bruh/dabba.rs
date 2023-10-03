# Container Internals

What even is _docker_?

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

- https://iximiuz.com/en/posts/implementing-container-runtime-shim
