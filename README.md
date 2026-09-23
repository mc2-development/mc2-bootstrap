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
| Backend | the APIs, gateway, migrator, shared Python lib | uv, cargo, Docker, kubectl, helm |
| Everything | all of the above | all of the above |
| Choose myself | whatever you tick | derived from what you ticked |

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
