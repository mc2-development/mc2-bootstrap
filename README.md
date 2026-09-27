# mc2-bootstrap

One script, two modes.

| | |
|---|---|
| `--member` (default) | Developer Mac setup — clones the repos, symlinks `kube`/`virtualize` |
| `--server` | Installs k3s on a fresh Linux box. Needs no repos and no checkout |

## Member mode

```bash
git clone https://github.com/mc2-development/mc2-bootstrap.git
cd mc2-bootstrap && ./bootstrap.sh
```

Asks what you'll be working on, checks and offers to install the prerequisites that
answer actually needs, generates an SSH key if you have none and walks you through
adding it to GitHub, clones the repos into `~/Work/mc2`, symlinks the CLIs.

| Preset | Clones | Toolchains checked |
|---|---|---|
| Frontend | the three apps + `mc2-ui` | node, pnpm |
| Backend | the APIs, gateway, migrator, both shared libs | uv, cargo, Docker, kubectl, helm |
| Everything | all of the above | all of the above |
| Choose myself | whatever you tick | derived from what you ticked |

`mc2-rust` comes with the backend preset and is not optional there: the Rust
services declare it as a PATH dependency (`../mc2-rust`), so a checkout without
it fails at `cargo build` with a missing Cargo.toml rather than anything naming
the real problem. `mc2-python` is the same relationship for the FastAPI
services, resolved through uv rather than a path.

Frontend is deliberately the lighter path: those repos run against the deployed dev
tier, so that checkout needs no Docker, no kubectl and no kubeconfig. `mc2-mailer-api`
is shown but unticked — it is an empty repo until the core platform is stable.

Doesn't touch the cluster. Afterwards (backend/everything):

```bash
kube --install-traefik   # one-time
kube --reboot
virtualize --setup
```

## Server mode

Copy the one file over and run it **from a shell on the server** — it prompts three
times, and if k3s misbehaves you want to already be there for `journalctl -u k3s`.

```bash
scp bootstrap.sh root@<SERVER_IP>:/tmp/
ssh root@<SERVER_IP>
bash /tmp/bootstrap.sh --server
```

Guards: refuses non-Linux, refuses non-root, requires the shared passphrase (an
accident guard, not a security control), and makes you type the hostname back.

Installs k3s pinned to `K3S_VERSION`, with:

- `--tls-san <public-ip>` so a kubeconfig pointed at the public address verifies
- `--secrets-encryption` — **install-time only**; enabling it later needs a restart,
  and without it every database password sits base64-encoded in the datastore where
  a disk image or Hetzner snapshot exposes it
- bundled Traefik and servicelb kept

It also offers to disable `ufw`, which filters flannel VXLAN and the pod/service
CIDRs and breaks networking in ways that look like application bugs.

And it raises the node's inotify limits to **8192 instances / 524288 watches**,
persisted in `/etc/sysctl.d/99-mc2-inotify.conf`. The kernel default of 128
instances is a desktop number: k3s's kubelet alone holds 25-40 watching
ConfigMap and Secret volumes, and every Go component on top — Traefik,
cert-manager, ArgoCD, Argo Workflows, Grafana, Loki, Prometheus, Alloy — opens
more. Past the limit a watcher fails with "too many open files" and the loser is
whichever process asked last, so the symptom surfaces far from the cause. Here it
cost the database's logs: Alloy's tailer failed, retried every three seconds, and
wrote its own failure into the stream it was meant to be reading. Nothing
alerted, because nothing had crashed.

Then, from your Mac:

```bash
scp root@<SERVER_IP>:/root/mc2-hetzner.kubeconfig ~/.kube/hetzner.yaml
export KUBECONFIG=~/.kube/hetzner.yaml && kubectl get nodes
```

Keep it as its own file — the gap between `docker-desktop` and production is not
something to leave to whichever context is current.

Continue with `mc2-k8s/docs/05-production-hetzner.md`.

> Re-running skips the k3s install if it's already active, but does **not** verify an
> existing server carries the same `--tls-san`.
>
> Re-running DOES apply the inotify settings, so an existing node gets them
> without a rebuild — it is the supported way to bring an older server up to
> this configuration.
