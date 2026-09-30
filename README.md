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

**`tailscale` is checked for every preset, including Frontend.** Every
`mc2-dev.com` hostname resolves to the server's tailnet address and the provider
firewall drops the public IP, so a machine that has not joined the tailnet cannot reach
the dev tier at all. Installed as a
cask (the macOS client is a GUI app that ships the CLI as a symlink); signing in
and being invited to the tailnet stay manual.

`mc2-rust` comes with the backend preset and is not optional there: the Rust
services declare it as a PATH dependency (`../mc2-rust`), so a checkout without
it fails at `cargo build` with a missing Cargo.toml rather than anything naming
the real problem. `mc2-python` is the same relationship for the FastAPI
services, resolved through uv rather than a path.

Frontend is deliberately the lighter path: those repos run against the deployed dev
tier, so that checkout needs no Docker, no kubectl and no kubeconfig — but it does
need tailscale, precisely *because* it runs against the deployed tier. `mc2-mailer-api`
is shown but unticked — it is an empty repo until the core platform is stable.

Doesn't touch the cluster. Afterwards (backend/everything):

```bash
kube --install-traefik   # one-time
kube --reboot
virtualize --setup
```

## Server mode

Installs k3s on a fresh Linux box. Needs no repos and no checkout.

**Normally nobody runs this.** `mc2-terraform/server` registers it as the VPS
provider's post-install hook, which fetches it at a pinned commit, verifies its
checksum and runs it while the OS is still installing. Provisioning a node is a
`terraform apply`. This repo is public so that hook can reach a raw URL — **nothing
secret may ever be added to it**; the unattended path takes its credentials from the
environment the hook sets.

```bash
MC2_UNATTENDED=1 MC2_SERVER_PASSPHRASE=... MC2_CONFIRM_HOSTNAME=$(hostname) \
  MC2_TS_AUTHKEY=tskey-... MC2_DISABLE_UFW=1 bash bootstrap.sh --server
```

Each variable replaces a prompt, and a missing one is a refusal naming the fix rather
than a default — a guard that defaults is not a guard. `MC2_TS_AUTHKEY` is the one that
matters: without a key `tailscale up` blocks on a browser URL nobody is watching, and
that single fact is what kept provisioning manual. Terraform mints the key
preauthorized, single-use and tagged, so the node joins as `tag:server`, its subnet
route is auto-approved by the tailnet policy, and it has no key expiry to lock anyone
out of a box with no inbound firewall rules.

With no terminal there is nothing to watch, so `/var/log/mc2-bootstrap.log` is the run
— `tail -f` it from the provider's web console.

The interactive path is unchanged and still the default, for repairing an existing box
or bringing up a provider Terraform does not cover:

```bash
scp bootstrap.sh root@<SERVER_IP>:/tmp/
ssh -t root@<SERVER_IP>            # -t, or the passphrase prompt refuses rather than echo it
bash /tmp/bootstrap.sh --server
```

Guards either way: refuses non-Linux, refuses non-root, requires the shared passphrase
(an accident guard, not a security control), and makes you name the hostname.

### What it does

**Tailscale first, before k3s.** The ordering is load-bearing in both directions: k3s
bakes its API certificate at install time and the tailnet address can only enter as a
`--tls-san`, and with the provider firewall closed the tunnel is the only route to the
host at all. `--accept-dns=false` is not optional on a Kubernetes node — accepting
tailnet DNS rewrites `/etc/resolv.conf`, which is what CoreDNS forwards to, so
in-cluster resolution breaks in a way that looks like a CoreDNS bug.

**Then k3s**, pinned to `K3S_VERSION`, with three install-time-only decisions:

| flag | why it cannot wait |
|---|---|
| `--tls-san` | Both the public and tailnet addresses. A SAN cannot be added to a running cluster without reinstalling. |
| `--secrets-encryption` | Otherwise every Secret sits base64-encoded in the datastore, where a snapshot or cloned disk hands over every password. Enabling it later needs a restart. |
| `--cluster-init` | Embedded etcd instead of SQLite. SQLite has **no snapshot mechanism at all** — `k3s etcd-snapshot` does not apply to it — so losing `state.db` loses the cluster, not just the data. |

Bundled Traefik and servicelb are kept. The script verifies encryption and the etcd
datastore actually came up rather than assuming the flags took: a cluster that came up
on SQLite looks completely healthy and simply has no backups, which is worth finding
out now rather than at a restore.

It also writes snapshot and kubelet settings to `/etc/rancher/k3s/config.yaml`
(6-hourly snapshots keeping 20; image GC at 70%/60%, because the kubelet's defaults do
not count the BuildKit cache PVC and the disk fills before it acts), raises inotify
limits to 8192/524288 in `/etc/sysctl.d`, and caps the journal at 500M. All four are
files rather than flags, so re-running fixes an existing node. Off-server snapshot
upload needs bucket credentials and is written by `mc2-terraform`, not here.

It offers to disable `ufw`, which filters flannel VXLAN and the pod/service CIDRs and
breaks networking in ways that look like application bugs.

### Afterwards

The kubeconfig lands at `/root/mc2-server.kubeconfig` with context `mc2-server` — named
for the role, not the vendor, because the previous `mc2-hetzner` pair turned a provider
change into a rename across five files in four repos.

```bash
scp root@<TAILNET_IP>:/root/mc2-server.kubeconfig ~/.kube/mc2-server.yaml
export KUBECONFIG=~/.kube/mc2-server.yaml && kubectl get nodes
```

Keep it as its own file. Then close the provider firewall — zero inbound rules, and
**attached**; an unattached firewall filters nothing — but only after `kubectl get
nodes` works over the tunnel. Closing it first locks you out of a box with no other way
in. Everything else is `mc2-k8s/docs/05-production-server.md`.
