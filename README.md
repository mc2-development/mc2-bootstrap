# mc2-bootstrap

One-shot local dev setup for MC2/Quotance. Clones the repos, symlinks the `kube`/`virtualize`
CLI tools, and checks prerequisites — everything up to (but not including) actually booting
the cluster.

## Usage

```bash
git clone https://github.com/mc2-development/mc2-bootstrap.git
cd mc2-bootstrap
./bootstrap.sh
```

If you don't already have an SSH key, it'll generate one and walk you through adding it to
your GitHub account before cloning anything.

## What it does

1. Checks for `git`, Docker Desktop, `kubectl`, `helm`
2. Generates an SSH key (`~/.ssh/id_ed25519`) if you don't have one, has you add it to
   GitHub, and confirms it works before continuing
3. Creates `~/Work/mc2`
4. Clones `mc2-wrappers`, `mc2-k8s`, `mc2-configs`, `mc2-cors`, `mc2-operation-api`,
   `mc2-accounting-api`, `mc2-agent-api`, `mc2-quotance-frontend` into it
5. Symlinks `kube` and `virtualize` into `/usr/local/bin` (asks for `sudo`)

## What it doesn't do

Doesn't touch the cluster. After it finishes, run these yourself:

```bash
cd ~/Work/mc2
kube --install-traefik   # one-time
kube --reboot
virtualize --setup
```

See `mc2-k8s/docs/02-local-setup.md` for details on what those do.
