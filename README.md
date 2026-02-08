# Wraut

Pronounced "route".

### A companion to Traefik and Docker

Wraut is a CI/CD server for automatically deploying
Dockerized apps through Traefik, applying configuration
for security and route handling.

It also serves as a dashboard to view the running status
of the containers.

### System requirements

Wraut assumes that the system where it's installed
satisfies the following constraints:
1. Is a Linux system
1. Has Docker with `compose` installed
1. Has Traefik 3.5.2 running as a Docker container
  with the following settings:
  - ports 80 and 443 exposed
  - volume: `"/var/run/docker.sock:/var/run/docker.sock:ro"`

### Service requirements

The services that run on it must comply with the following
requirements:
1. Have a `docker-compose.yml` file in the base of the repo.

There will be some config you'll need to provide Wraut as
a `.env` file (TODO: commit the `.env.sample` and a guide to
selecting the appropriate values).

## Cross-compiling
`cargo zigbuild --release --target aarch64-unknown-linux-musl`

## TODO
- [x] Make a refresh button on each service
- [x] Make a delete button on each service
  - [x] Make a deletion event
  - [x] Make a deletion flow
- [ ] Make a deactivate button
  - [ ] Make a deactivation event
  - [ ] Make a deactivation flow
- [ ] Register a service with a GitHub action
  - [ ] Test key
  - [ ] Test action
- [ ] Refactor error event logging on current `?;`s

