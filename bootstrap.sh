#!/bin/bash
# Prepares a machine: --member sets up a developer Mac, --server installs k3s on a
# Linux host. Terraform runs the server mode as the provider's post-install hook, so
# MC2_UNATTENDED exists; see usage(). This repo is PUBLIC — nothing secret goes in it.
# No pipefail: ~27 pipelines rely on the first command being allowed to fail.
set -eu

GITHUB_OWNER="${GITHUB_OWNER:-mc2-development}"

# A post-install hook has no login environment, so $HOME is unset and `set -u` aborts.
: "${HOME:=/root}"

WORK_DIR="$HOME/Work"
MC2_DIR="$WORK_DIR/mc2"
REPOS=(mc2-wrappers mc2-k8s mc2-core mc2-python mc2-rust mc2-gateway mc2-account-api mc2-crons mc2-operation-api mc2-accounting-api mc2-agent-api mc2-mailer-api mc2-operation-frontend mc2-accounting-frontend mc2-platform-frontend mc2-ui)

# The default name, so ssh finds it with no ~/.ssh/config. Overridable, because a Mac
# with a work and a personal GitHub account has one key each and the default name is
# usually the personal one: MC2_SSH_KEY=~/.ssh/id_work ./bootstrap.sh --member
SSH_KEY="${MC2_SSH_KEY:-$HOME/.ssh/id_ed25519}"

# --- server mode settings -----------------------------------------------------

# An accident guard, not a security control. Change it: printf '%s' 'x' | shasum -a 256
SERVER_PASSPHRASE_SHA256="f8c87098a3fdd32415f48701a4d497433f0beaad98654a873b8755d424158ded"

# Named for the role, not the vendor: the last rename touched five files in four repos.
KUBECONFIG_OUT="/root/mc2-server.kubeconfig"
KUBE_CONTEXT_NAME="mc2-server"

# Unattended there is no terminal, so this is the only record of the run.
SERVER_LOG="/var/log/mc2-bootstrap.log"

# Pinned so a rebuild reproduces THIS cluster, not whatever "stable" means today.
K3S_VERSION="v1.36.4+k3s1"

UNATTENDED="${MC2_UNATTENDED:-}"

# Every mc2-dev.com hostname resolves to this tailnet device.
MC2_TS_HOSTNAME="${MC2_TS_HOSTNAME:-mc2}"

# Advertised to the tailnet; must match mc2-k8s/overlays/<env>/data/clusterips.yaml.
K3S_SERVICE_CIDR="10.43.0.0/16"

MODE="member"
IP_OVERRIDE=""
usage() {
  cat <<'EOF'
Usage: bootstrap.sh [--member | --server] [options]

Modes:
  --member          Set up a developer Mac (default). Repos, GitHub SSH, CLI tools.
  --server          Set up a Linux k3s host. Installs k3s only — no repos, no SSH keys.

Options:
  --ip <address>    Server mode: override the auto-detected public IP.
  -h, --help        Show this message.

Server mode, unattended. This is how Terraform runs it as the provider's post-install
hook; interactively none of it is needed. A value that is missing is a refusal, not a
default — every one of these replaces a guard, and a guard that defaults is not one.

  MC2_UNATTENDED=1            No prompts. No spinner (it animates into the log with
                              \r forever). Everything tee'd to /var/log/mc2-bootstrap.log.
  MC2_SERVER_PASSPHRASE=...   The accident guard. Also honoured interactively.
  MC2_CONFIRM_HOSTNAME=...    Must equal `hostname`, replacing typing it back.
  MC2_TS_AUTHKEY=tskey-...    Pre-authorized, tagged, single-use. Replaces the browser
                              auth URL — the one reason this script used to need a human.
  MC2_DISABLE_UFW=1           Answers the ufw prompt. Unset means leave ufw alone, which
                              on a k3s node is a decision, so it is stated rather than assumed.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --server)
      MODE="server"
      shift
      ;;
    --member)
      MODE="member"
      shift
      ;;
    --ip)
      IP_OVERRIDE="$2"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo ""
      usage
      exit 1
      ;;
  esac
done

# Runs a command with a spinner, or plain lines when there is no terminal.
run_with_spinner() {
  local msg="$1"
  shift

  # \r frames would turn the log into megabytes of carriage returns.
  if [[ -n "$UNATTENDED" || ! -t 1 ]]; then
    echo "      · $msg"
    if "$@" >>"${SERVER_LOG:-/dev/null}" 2>&1; then
      echo "      ✓ $msg"
      return 0
    fi
    echo "      ✗ $msg (failed)"
    tail -40 "${SERVER_LOG:-/dev/null}" 2>/dev/null || true
    exit 1
  fi

  local log
  log="$(mktemp)"
  "$@" >"$log" 2>&1 &
  local pid=$!
  local frames='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
  local i=0
  while kill -0 "$pid" 2>/dev/null; do
    printf "\r      %s %s" "${frames:$i:1}" "$msg"
    i=$(((i + 1) % ${#frames}))
    sleep 0.1
  done
  # `wait` under `set -e` would abort before the failure branch could print the log.
  local status=0
  wait "$pid" || status=$?
  if [[ $status -eq 0 ]]; then
    printf "\r      ✓ %s\n" "$msg"
  else
    printf "\r      ✗ %s (failed)\n" "$msg"
    cat "$log"
    rm -f "$log"
    exit 1
  fi
  rm -f "$log"
}

# Empty repos: selectable, never ticked by default, since cloning one gets a bare dir.
repo_note() {
  case "$1" in
    mc2-mailer-api) echo " — placeholder, not started" ;;
    *) echo "" ;;
  esac
}

# Arrow-key menu; already-cloned repos are locked on. Result in $CHECKED_ITEMS.
checkbox_menu() {
  set +e
  local items=("$@")
  local n=${#items[@]}
  local checked=() locked=()
  local i cur key key2

  for ((i = 0; i < n; i++)); do
    if [[ -n "$(repo_note "${items[i]}")" ]]; then checked[i]=0; else checked[i]=1; fi
    if [[ -d "$MC2_DIR/${items[i]}" ]]; then locked[i]=1; else locked[i]=0; fi
  done

  cur=-1
  for ((i = 0; i < n; i++)); do
    if [[ ${locked[i]} -eq 0 ]]; then
      cur=$i
      break
    fi
  done

  draw() {
    for ((i = 0; i < n; i++)); do
      if [[ ${locked[i]} -eq 1 ]]; then
        printf "\r\033[K    \033[2m[x] %s (already installed)\033[0m\n" "${items[i]}"
        continue
      fi
      local mark=" "
      [[ ${checked[i]} -eq 1 ]] && mark="x"
      local note
      note="$(repo_note "${items[i]}")"
      if [[ $i -eq $cur ]]; then
        printf "\r\033[K  > [%s] %s\033[2m%s\033[0m\n" "$mark" "${items[i]}" "$note"
      else
        printf "\r\033[K    [%s] %s\033[2m%s\033[0m\n" "$mark" "${items[i]}" "$note"
      fi
    done
  }

  if [[ $cur -eq -1 ]]; then
    draw
    CHECKED_ITEMS=("${items[@]}")
    set -e
    return
  fi

  tput civis 2>/dev/null
  draw
  while true; do
    IFS= read -rsn1 key
    if [[ $key == $'\x1b' ]]; then
      read -rsn2 key2
      if [[ $key2 == "[A" ]]; then
        for ((i = cur - 1; i >= 0; i--)); do
          [[ ${locked[i]} -eq 0 ]] && {
            cur=$i
            break
          }
        done
      elif [[ $key2 == "[B" ]]; then
        for ((i = cur + 1; i < n; i++)); do
          [[ ${locked[i]} -eq 0 ]] && {
            cur=$i
            break
          }
        done
      fi
    elif [[ $key == " " ]]; then
      if [[ ${checked[cur]} -eq 1 ]]; then checked[cur]=0; else checked[cur]=1; fi
    elif [[ -z $key ]]; then
      break
    fi
    printf "\033[%dA" "$n"
    draw
  done
  tput cnorm 2>/dev/null

  CHECKED_ITEMS=()
  for ((i = 0; i < n; i++)); do
    [[ ${checked[i]} -eq 1 ]] && CHECKED_ITEMS+=("${items[i]}")
  done
  set -e
}

banner() {
  cat <<'EOF'
                            ░██████
                           ░██   ░██
░█████████████   ░███████        ░██
░██   ░██   ░██ ░██    ░██   ░█████
░██   ░██   ░██ ░██         ░██
░██   ░██   ░██ ░██    ░██ ░██
░██   ░██   ░██  ░███████  ░████████
EOF
  echo ""
  echo "$1"
  echo "--------------------"
  echo ""
}

sha256_of() {
  if command -v shasum >/dev/null 2>&1; then
    printf '%s' "$1" | shasum -a 256 | awk '{print $1}'
  else
    printf '%s' "$1" | sha256sum | awk '{print $1}'
  fi
}

# ==============================================================================
# SERVER MODE
# ==============================================================================
run_server_bootstrap() {
  banner "Server bootstrap (k3s)"

  # Before the guards: a refusal is what someone reads the log for.
  if [[ -n "$UNATTENDED" ]]; then
    mkdir -p "$(dirname "$SERVER_LOG")"
    exec > >(tee -a "$SERVER_LOG") 2>&1
    echo "=== bootstrap.sh --server (unattended) $(date -u +%FT%TZ) ==="
  fi

  # --- guard 1: platform -------------------------------------------------------
  # Before the passphrase: --server on a Mac deserves better than "wrong passphrase".
  if [[ "$(uname -s)" != "Linux" ]]; then
    echo "Refusing: --server provisions a Linux k3s host, but this is $(uname -s)."
    echo "Did you mean --member (the developer Mac setup)?"
    exit 1
  fi

  if [[ "$EUID" -ne 0 ]]; then
    echo "Refusing: --server needs root (it installs k3s as a systemd service)."
    exit 1
  fi

  # --- guard 2: passphrase -----------------------------------------------------
  local supplied
  if [[ -n "${MC2_SERVER_PASSPHRASE:-}" ]]; then
    supplied="$MC2_SERVER_PASSPHRASE"
  elif [[ -n "$UNATTENDED" ]]; then
    echo "Refusing: MC2_UNATTENDED is set but MC2_SERVER_PASSPHRASE is not."
    echo "  Unattended, every prompt must be answered by a value. Pass it, or drop"
    echo "  MC2_UNATTENDED and run this from a terminal."
    exit 1
  else
    # `read -s` cannot disable echo without a terminal, so the passphrase would be clear.
    if [[ ! -t 0 ]]; then
      echo "Refusing: no terminal attached, so the passphrase would be echoed in clear."
      echo ""
      echo "  Re-run with a TTY:   ssh -t root@<ip> 'bash /tmp/bootstrap.sh --server'"
      echo "  Or non-interactively: ssh root@<ip> 'MC2_SERVER_PASSPHRASE=... bash /tmp/bootstrap.sh --server'"
      exit 1
    fi
    read -rsp "Server passphrase: " supplied
    echo ""
  fi

  if [[ "$(sha256_of "$supplied")" != "$SERVER_PASSPHRASE_SHA256" ]]; then
    echo "Refusing: passphrase does not match."
    exit 1
  fi
  unset supplied MC2_SERVER_PASSPHRASE
  echo "      ✓ Passphrase accepted"
  echo ""

  # --- detect identity ---------------------------------------------------------
  local hostname_here public_ip
  hostname_here="$(hostname)"
  if [[ -n "$IP_OVERRIDE" ]]; then
    public_ip="$IP_OVERRIDE"
  else
    # No provider metadata probe: it cost three seconds of timeout on every other host.
    public_ip="$(curl -sf --max-time 5 https://ifconfig.me 2>/dev/null || true)"
    [[ -z "$public_ip" ]] && public_ip="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}')"
  fi

  if [[ -z "$public_ip" ]]; then
    echo "Could not determine this machine's public IP. Pass it explicitly: --ip <address>"
    exit 1
  fi

  # --- guard 3: confirmation ---------------------------------------------------
  echo "About to install k3s on:"
  echo ""
  echo "      hostname   $hostname_here"
  echo "      public IP  $public_ip"
  echo ""
  echo "This installs a Kubernetes control plane and its bundled Traefik."

  # Unattended still names the machine, just in advance; Terraform fills it in.
  local typed
  if [[ -n "$UNATTENDED" ]]; then
    typed="${MC2_CONFIRM_HOSTNAME:-}"
    if [[ -z "$typed" ]]; then
      echo "Refusing: MC2_UNATTENDED is set but MC2_CONFIRM_HOSTNAME is not."
      echo "  Set it to this machine's hostname ('$hostname_here') to confirm the target."
      exit 1
    fi
    echo "      confirmed by MC2_CONFIRM_HOSTNAME=$typed"
  else
    read -rp "Type the hostname to confirm: " typed
  fi
  if [[ "$typed" != "$hostname_here" ]]; then
    echo "Refusing: '$typed' does not match '$hostname_here'."
    exit 1
  fi
  echo ""

  # --- [1/3] prerequisites -----------------------------------------------------
  echo "[1/3] Checking prerequisites"
  local missing=()
  command -v curl >/dev/null 2>&1 || missing+=("curl")
  command -v systemctl >/dev/null 2>&1 || missing+=("systemd (systemctl)")
  if [[ ${#missing[@]} -gt 0 ]]; then
    echo "      Missing: ${missing[*]}"
    echo "      Install these first, then re-run."
    exit 1
  fi
  echo "      ✓ curl and systemd present"

  # Before k3s: the API certificate is baked at install time and needs the tailnet SAN,
  # and with the firewall closed the tunnel is the only route in.
  if ! command -v tailscale >/dev/null 2>&1; then
    echo "      Installing tailscale (the only way into this host once the firewall is closed)"
    # The vendor script adds the signing key and the release channel; apt alone cannot.
    run_with_spinner "curl tailscale.com/install.sh | sh" \
      bash -c 'curl -fsSL https://tailscale.com/install.sh | sh'
  else
    echo "      ✓ tailscale already installed ($(tailscale version 2>/dev/null | head -1))"
  fi

  # Before `tailscale up`: without it the route is advertised by a node that cannot
  # carry it, and tailscale only warns. In sysctl.d, so a reboot keeps it.
  echo "      Enabling IP forwarding (a subnet router cannot work without it)"
  printf 'net.ipv4.ip_forward = 1\nnet.ipv6.conf.all.forwarding = 1\n' >/etc/sysctl.d/99-tailscale.conf
  sysctl --system >/dev/null 2>&1 || true
  if [[ "$(sysctl -n net.ipv4.ip_forward 2>/dev/null)" == "1" ]]; then
    echo "      ✓ net.ipv4.ip_forward = 1 (persisted in /etc/sysctl.d/99-tailscale.conf)"
  else
    echo "      ! IP forwarding is still off — the subnet route will be advertised but"
    echo "        carry no traffic, and tailscale will only warn about it."
  fi

  # Tailscale caps throughput without it, and warns in every future log.
  _iface="$(ip -4 route show default 2>/dev/null | awk '{print $5; exit}')"
  if [[ -n "$_iface" ]] && command -v ethtool >/dev/null 2>&1; then
    ethtool -K "$_iface" rx-udp-gro-forwarding on rx-gro-list off >/dev/null 2>&1 \
      && echo "      ✓ UDP GRO forwarding tuned on $_iface"
  fi

  # Without a key this blocks on a browser URL, so it is never spinner-wrapped.
  # --accept-dns=false is not optional: tailnet DNS rewrites the resolv.conf CoreDNS
  # forwards to.
  _ts_routes="$(tailscale debug prefs 2>/dev/null | grep -A2 '"AdvertiseRoutes"' | grep -c "$K3S_SERVICE_CIDR" || true)"
  if tailscale status >/dev/null 2>&1 && [[ "${_ts_routes:-0}" -gt 0 ]]; then
    echo "      ✓ tailscale up, already advertising $K3S_SERVICE_CIDR"
  else
    echo ""
    if [[ -n "${MC2_TS_AUTHKEY:-}" ]]; then
      echo "      Bringing tailscale up with a pre-authorized key as '$MC2_TS_HOSTNAME'."
      tailscale up \
        --authkey="$MC2_TS_AUTHKEY" \
        --hostname="$MC2_TS_HOSTNAME" \
        --advertise-routes="$K3S_SERVICE_CIDR" \
        --accept-dns=false
      echo ""
      echo "      ✓ tailscale up as '$MC2_TS_HOSTNAME', advertising $K3S_SERVICE_CIDR"
      echo "        The route is approved by the tailnet policy (autoApprovers for"
      echo "        tag:server), which mc2-terraform/server owns. Nothing to click."
    elif [[ -n "$UNATTENDED" ]]; then
      echo "Refusing: MC2_UNATTENDED is set but MC2_TS_AUTHKEY is not."
      echo "  Without a key 'tailscale up' blocks on a browser URL nobody is watching,"
      echo "  and the run would hang until the provider's hook timed out."
      exit 1
    else
      echo "      Bringing tailscale up and advertising the cluster service CIDR."
      echo "      A URL follows — open it to authenticate this machine."
      echo ""
      tailscale up --hostname="$MC2_TS_HOSTNAME" --advertise-routes="$K3S_SERVICE_CIDR" --accept-dns=false
      echo ""
      echo "      ✓ tailscale up as '$MC2_TS_HOSTNAME', advertising $K3S_SERVICE_CIDR"
      echo "      ! The route is NOT live until it is approved — by the tailnet policy"
      echo "        if tag:server auto-approves it, otherwise in the admin console:"
      echo "        Machines -> this host -> Subnets -> approve $K3S_SERVICE_CIDR"
      echo "        Until then a laptop can reach this host but not the ClusterIPs behind it."
    fi
  fi

  # Resolved once: two later steps read it, and on a re-run an unset value silently
  # rewrote a working kubeconfig to a blocked address.
  ts_ip="$(tailscale ip -4 2>/dev/null | head -1 || true)"

  # ufw filters flannel VXLAN and the pod/service CIDRs: the cluster looks healthy and
  # DNS quietly fails. Firewall at the provider layer instead.
  if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -qi '^Status: active'; then
    echo ""
    echo "      ! ufw is active. k3s needs it off, or explicitly opened for the"
    echo "        pod (10.42.0.0/16) and service (10.43.0.0/16) CIDRs — otherwise"
    echo "        pod networking and CoreDNS break in ways that look like app bugs."
    echo ""
    echo "        Recommended: disable ufw and firewall at the PROVIDER layer, in"
    echo "        front of the host (rules printed at the end of this script)."
    echo ""
    local ufw_ans
    if [[ -n "$UNATTENDED" ]]; then
      if [[ -z "${MC2_DISABLE_UFW:-}" ]]; then
        echo "Refusing: ufw is active, MC2_UNATTENDED is set and MC2_DISABLE_UFW is not."
        echo "  Leaving ufw on breaks pod networking and CoreDNS in ways that look like"
        echo "  application bugs, so this is not something to decide by defaulting."
        exit 1
      fi
      ufw_ans=y
      echo "      answered by MC2_DISABLE_UFW"
    else
      read -rp "      Disable ufw now? [y/N] " ufw_ans
    fi
    if [[ "$ufw_ans" =~ ^[Yy]$ ]]; then
      ufw --force disable
      echo "      ✓ ufw disabled"
    else
      echo "      Leaving ufw on. Add the CIDR rules yourself before trusting the cluster:"
      echo "        ufw allow from 10.42.0.0/16 && ufw allow from 10.43.0.0/16"
    fi
  else
    echo "      ✓ ufw not active"
  fi

  # 128 instances is a desktop default and kubelet alone holds 25-40; past it a watcher
  # fails with "too many open files" and nothing crashes. OpenShift's numbers.
  echo ""
  echo "      Raising inotify limits (128 instances is a desktop default)"
  _sysctl_file=/etc/sysctl.d/99-mc2-inotify.conf
  printf 'fs.inotify.max_user_instances = 8192\nfs.inotify.max_user_watches = 524288\n' >"$_sysctl_file"
  sysctl --system >/dev/null 2>&1 || true
  _inst="$(cat /proc/sys/fs/inotify/max_user_instances 2>/dev/null || echo 0)"
  _watch="$(cat /proc/sys/fs/inotify/max_user_watches 2>/dev/null || echo 0)"
  if [[ "$_inst" -ge 8192 ]]; then
    echo "      ✓ inotify: $_inst instances, $_watch watches (persisted in $_sysctl_file)"
  else
    echo "      ! inotify still reports $_inst instances after applying $_sysctl_file."
    echo "        Something else is overriding it — check /etc/sysctl.conf and"
    echo "        /etc/sysctl.d/*. Leaving it will cost you pod logs, silently."
  fi

  # /var also holds every image, the BuildKit cache and the datastore, and Alloy ships
  # the logs to Loki anyway. A drop-in, because upgrades rewrite the main config.
  echo "      Capping the systemd journal (default is 10% of /var)"
  mkdir -p /etc/systemd/journald.conf.d
  printf '[Journal]\nSystemMaxUse=500M\nSystemKeepFree=2G\n' >/etc/systemd/journald.conf.d/99-mc2.conf
  systemctl restart systemd-journald >/dev/null 2>&1 || true
  echo "      ✓ journal capped at 500M (/etc/systemd/journald.conf.d/99-mc2.conf)"

  # Image GC does not count the BuildKit cache PVC, so the defaults fill the disk before
  # it acts. In config.yaml, not INSTALL_K3S_EXEC, so a re-run fixes an existing node.
  echo "      Setting kubelet image-GC thresholds (defaults fill this disk)"
  mkdir -p /etc/rancher/k3s
  _k3s_cfg=/etc/rancher/k3s/config.yaml
  if [[ -f "$_k3s_cfg" ]] && ! grep -q "image-gc-high-threshold" "$_k3s_cfg" 2>/dev/null; then
    echo "      ! $_k3s_cfg already exists and does not set image-gc thresholds."
    echo "        Not overwriting it — merge these by hand:"
    echo "          kubelet-arg:"
    echo "            - \"image-gc-high-threshold=70\""
    echo "            - \"image-gc-low-threshold=60\""
    echo "          etcd-snapshot-schedule-cron: \"0 */6 * * *\""
    echo "          etcd-snapshot-retention: 20"
  else
    # Snapshots stay local: off-server upload needs credentials, and this repo is public.
    printf 'kubelet-arg:\n  - "image-gc-high-threshold=70"\n  - "image-gc-low-threshold=60"\netcd-snapshot-schedule-cron: "0 */6 * * *"\netcd-snapshot-retention: 20\n' >"$_k3s_cfg"
    echo "      ✓ image GC at 70%%/60%%, etcd snapshots every 6h keeping 20 ($_k3s_cfg)"
    if systemctl is-active --quiet k3s 2>/dev/null; then
      echo "      ! k3s is already running — this file is read at startup, so the"
      echo "        change is pending until:  systemctl restart k3s"
      echo "        (brief API-server outage; running pods are not restarted)"
    fi
  fi
  echo ""

  # --- [2/3] k3s ---------------------------------------------------------------
  echo "[2/3] Installing k3s"
  if systemctl is-active --quiet k3s 2>/dev/null; then
    echo "      ✓ k3s already running — leaving it alone"
  else
    # Three install-time-only decisions: --tls-san (the certificate is baked now),
    # --secrets-encryption (or a stolen snapshot hands over every password) and
    # --cluster-init (SQLite has no snapshot mechanism at all).
    tls_sans="--tls-san $public_ip"
    if [[ -n "$ts_ip" ]]; then
      echo "      ✓ tailscale detected ($ts_ip) — adding it to the API certificate"
      tls_sans+=" --tls-san $ts_ip"
    else
      # A certificate without the tailnet SAN cannot be fixed without reinstalling k3s.
      echo "      ✗ tailscale is installed but has no IPv4 address."
      echo "        The API certificate is baked at install time and the tailnet"
      echo "        address can only go in as a --tls-san, so continuing would"
      echo "        produce a cluster this machine cannot be administered through."
      echo ""
      echo "        Fix it, then re-run:  tailscale up --advertise-routes=$K3S_SERVICE_CIDR --accept-dns=false"
      exit 1
    fi
    run_with_spinner "curl get.k3s.io | sh ($K3S_VERSION)" \
      env INSTALL_K3S_VERSION="$K3S_VERSION" \
      INSTALL_K3S_EXEC="$tls_sans --secrets-encryption --cluster-init" \
      bash -c 'curl -sfL https://get.k3s.io | sh -'

    run_with_spinner "waiting for the node to become Ready" \
      bash -c 'for i in $(seq 1 60); do
                 /usr/local/bin/k3s kubectl get nodes 2>/dev/null | grep -q " Ready " && exit 0
                 sleep 2
               done
               exit 1'
  fi
  echo ""

  # Absolute path: /usr/local/bin is not always on root's PATH in a non-login shell,
  # and a swallowed command-not-found would report a false negative.
  if /usr/local/bin/k3s secrets-encrypt status 2>/dev/null | grep -qi "enabled"; then
    echo "      ✓ secrets encrypted at rest"
  else
    echo "      ! secrets-at-rest encryption is NOT active. Check: k3s secrets-encrypt status"
    echo "        If k3s pre-dated this script, enabling it needs a restart:"
    echo "        add 'secrets-encryption: true' to /etc/rancher/k3s/config.yaml && systemctl restart k3s"
  fi

  # The only signal that --cluster-init took: a SQLite cluster looks healthy and simply
  # has no backup mechanism.
  if /usr/local/bin/k3s etcd-snapshot ls >/dev/null 2>&1; then
    echo "      ✓ datastore is etcd — k3s etcd-snapshot works"
  else
    echo "      ! datastore is NOT etcd, so no snapshot mechanism exists at all."
    echo "        On an existing node: add 'cluster-init: true' to"
    echo "        /etc/rancher/k3s/config.yaml and restart k3s, which migrates in place."
  fi
  echo ""

  # --- [3/3] kubeconfig --------------------------------------------------------
  echo "[3/3] Writing a remote-ready kubeconfig"
  # k3s names cluster, context and user all "default", which is how you deploy to the
  # wrong cluster. The address is the tailnet one; nothing else reaches 6443.
  if [[ -z "$ts_ip" ]]; then
    echo "      ! No tailscale address — falling back to the public IP $public_ip."
    echo "        The firewall drops 6443 there, so this kubeconfig will time out."
    echo "        Fix tailscale and re-run rather than trusting the file below."
  fi
  kube_addr="${ts_ip:-$public_ip}"
  sed -e "s|https://127.0.0.1:6443|https://${kube_addr}:6443|" \
    -e "s|name: default|name: ${KUBE_CONTEXT_NAME}|g" \
    -e "s|cluster: default|cluster: ${KUBE_CONTEXT_NAME}|g" \
    -e "s|user: default|user: ${KUBE_CONTEXT_NAME}|g" \
    -e "s|current-context: default|current-context: ${KUBE_CONTEXT_NAME}|g" \
    /etc/rancher/k3s/k3s.yaml >"$KUBECONFIG_OUT"
  chmod 600 "$KUBECONFIG_OUT"
  echo "      ✓ $KUBECONFIG_OUT (context: $KUBE_CONTEXT_NAME)"
  echo ""

  echo "--------------------"
  echo "k3s is up. What is left, and who does it:"
  echo ""
  echo "1. Fetch the kubeconfig — its own file, never merged into ~/.kube/config:"
  echo ""
  echo "     scp root@${kube_addr}:${KUBECONFIG_OUT} ~/.kube/${KUBE_CONTEXT_NAME}.yaml"
  echo "     export KUBECONFIG=~/.kube/${KUBE_CONTEXT_NAME}.yaml && kubectl get nodes"
  echo ""
  echo "2. Close the provider firewall: NO inbound rules, and ATTACHED — an"
  echo "   unattached firewall filters nothing. Nothing needs to be open. ssh, the"
  echo "   Kubernetes API and https all ride the tailscale tunnel, which is"
  echo "   outbound-only, and certificates come from dns01, so not even :80 is"
  echo "   needed. Postgres stays a ClusterIP, reached over the subnet route."
  echo ""
  echo "   Verify BOTH, in this order — closing it before the tunnel is proven"
  echo "   locks you out of a box with no other way in:"
  echo "     kubectl get nodes                                  # over the tailnet"
  echo "     for p in 22 80 443 6443; do nc -z -G4 ${public_ip} \$p; done   # all fail"
  echo ""
  echo "3. The subnet route ($K3S_SERVICE_CIDR) is approved by the tailnet policy in"
  echo "   mc2-terraform/server, not by hand. Without it a laptop reaches this host"
  echo "   but not the pinned ClusterIPs every dev connection string dials, and a"
  echo "   connection hangs rather than refuses. Check:"
  echo "     tailscale status --json | jq '.Self.PrimaryRoutes'"
  echo ""
  echo "4. Bring the platform up from mc2-k8s — see docs/05-production-server.md."
  echo "   The order there is a dependency chain, not a preference, and secrets now"
  echo "   come from Infisical rather than a laptop."
}

# ==============================================================================
# MEMBER MODE
# ==============================================================================
run_member_bootstrap() {
  if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "Refusing: --member sets up a developer Mac, but this is $(uname -s)."
    echo "Did you mean --server (the k3s host setup)?"
    exit 1
  fi

  banner "Local dev bootstrap"

  # Before prerequisites: the answer decides which toolchains are installed.
  echo "[1/5] What will you be working on?"
  echo ""
  echo "  1) Frontend   — the three apps and the shared UI layer. No backend, no database."
  echo "  2) Backend    — the APIs, the gateway, migrations and the shared Python library."
  echo "  3) Everything — the whole platform."
  echo "  4) Choose repositories myself"
  echo ""
  read -rp "  Select [1-4]: " _role
  echo ""

  # mc2-configs is retired: settings in mc2-k8s, credentials in Infisical.
  BASE_REPOS=(mc2-wrappers mc2-k8s)
  FRONTEND_REPOS=(mc2-ui mc2-operation-frontend mc2-accounting-frontend mc2-platform-frontend)
  # mc2-mailer-api is absent on purpose (empty repo); mc2-rust is not optional, the
  # Rust services declare it as a path dependency.
  BACKEND_REPOS=(mc2-core mc2-python mc2-rust mc2-gateway mc2-account-api mc2-operation-api mc2-accounting-api mc2-agent-api mc2-crons)

  ROLE="everything"
  case "$_role" in
    1)
      ROLE="frontend"
      REPOS=("${BASE_REPOS[@]}" "${FRONTEND_REPOS[@]}")
      ;;
    2)
      ROLE="backend"
      REPOS=("${BASE_REPOS[@]}" "${BACKEND_REPOS[@]}")
      ;;
    3) REPOS=("${BASE_REPOS[@]}" "${FRONTEND_REPOS[@]}" "${BACKEND_REPOS[@]}") ;;
    4)
      echo "  (↑/↓ move, space toggle, enter confirm)"
      echo ""
      checkbox_menu "${REPOS[@]}"
      REPOS=("${CHECKED_ITEMS[@]}")
      ROLE="custom"
      ;;
    *)
      echo "  Unrecognised choice — cloning everything."
      REPOS=("${BASE_REPOS[@]}" "${FRONTEND_REPOS[@]}" "${BACKEND_REPOS[@]}")
      ;;
  esac
  echo "  Selected: $ROLE (${#REPOS[@]} repositories)"
  echo ""

  # Derived from the repos, not the role label, so "choose myself" behaves like a preset.
  NEEDS_PYTHON=false
  NEEDS_RUST=false
  NEEDS_NODE=false
  for repo in "${REPOS[@]}"; do
    case "$repo" in
      mc2-operation-api | mc2-accounting-api | mc2-agent-api | mc2-crons | mc2-python | mc2-mailer-api) NEEDS_PYTHON=true ;;
      mc2-gateway | mc2-account-api | mc2-core) NEEDS_RUST=true ;;
      mc2-ui | mc2-operation-frontend | mc2-accounting-frontend | mc2-platform-frontend) NEEDS_NODE=true ;;
    esac
  done
  # Only a backend checkout runs a local cluster; the frontend points at deployed dev.
  NEEDS_CLUSTER=false
  [[ "$NEEDS_PYTHON" == true || "$NEEDS_RUST" == true ]] && NEEDS_CLUSTER=true

  echo "[2/5] Checking prerequisites"

  if ! command -v brew >/dev/null 2>&1; then
    read -rp "      Homebrew isn't installed (needed for the tools below). Install it now? [y/N] " ans
    if [[ "$ans" =~ ^[Yy]$ ]]; then
      /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
      if [[ -x /opt/homebrew/bin/brew ]]; then
        eval "$(/opt/homebrew/bin/brew shellenv)"
      elif [[ -x /usr/local/bin/brew ]]; then
        eval "$(/usr/local/bin/brew shellenv)"
      fi
    fi
    echo ""
  fi

  # Offers, never silent installs. Declining returns 0, or `set -e` reads it as failure.
  tool_offer() {
    local tool="$1" cmd="$2" why="$3"
    command -v "$tool" >/dev/null 2>&1 && return 0
    read -rp "      $tool isn't installed ($why). Install it now? [y/N] " ans
    if [[ "$ans" =~ ^[Yy]$ ]]; then
      run_with_spinner "$cmd" sh -c "$cmd"
    fi
    return 0
  }

  brew_offer() {
    local tool="$1" formula="$2" why="$3"
    command -v "$tool" >/dev/null 2>&1 && return 0
    command -v brew >/dev/null 2>&1 || return 0
    read -rp "      $tool isn't installed ($why). Install it now via Homebrew? [y/N] " ans
    if [[ "$ans" =~ ^[Yy]$ ]]; then
      run_with_spinner "brew install $formula" brew install "$formula"
    fi
    return 0
  }

  # Every preset: a machine off the tailnet cannot reach dev at all. A cask, not a
  # formula — the formula installs a daemon that fights the GUI app.
  if ! command -v tailscale >/dev/null 2>&1 && [[ ! -d /Applications/Tailscale.app ]]; then
    if command -v brew >/dev/null 2>&1; then
      read -rp "      tailscale isn't installed (the only route to the dev tier). Install it now? [y/N] " ans
      if [[ "$ans" =~ ^[Yy]$ ]]; then
        run_with_spinner "brew install --cask tailscale" brew install --cask tailscale
      fi
    else
      echo "      ! tailscale isn't installed and Homebrew is missing."
      echo "        Install from https://tailscale.com/download/mac — without it the"
      echo "        dev tier is unreachable, and the failure looks like DNS."
    fi
  else
    echo "      ✓ tailscale present"
  fi

  # Installed is not joined, and the difference is invisible until a request times out.
  if command -v tailscale >/dev/null 2>&1 && ! tailscale status >/dev/null 2>&1; then
    echo "      ! tailscale is installed but not signed in."
    echo "        Open Tailscale and sign in, then ask an admin to invite this machine"
    echo "        to the tailnet. Verify with:  tailscale status"
  fi

  # Without the binary the pre-commit hook skips itself: a scanner in name only.
  brew_offer gitleaks gitleaks "blocks a commit that stages a credential"

  # The hooks gate on `command -v`, so without these they quietly do nothing.
  brew_offer shellcheck shellcheck "lints the shell scripts before a push"
  brew_offer shfmt shfmt "formats the shell scripts (pre-commit rewrites and re-stages)"
  brew_offer yamlfmt yamlfmt "formats YAML — k8s manifests, workflows, config templates"
  brew_offer taplo taplo "formats TOML — Cargo.toml and pyproject.toml"
  brew_offer terraform terraform "formats and validates mc2-terraform (its CI checks both)"
  # Its own tap: `brew install tflint` finds nothing.
  brew_offer tflint terraform-linters/tap/tflint "lints mc2-terraform (pre-push runs it)"

  if [[ "$NEEDS_CLUSTER" == true ]]; then
    brew_offer kubectl kubectl "talks to the cluster"
    brew_offer helm helm "installs Traefik locally"
    # Both read the server kubeconfig; argocd additionally wants one `argocd login`.
    brew_offer argo argo "reads the build pipeline (argo workflows)"
    brew_offer argocd argocd "reads what is deployed (argo cd)"
    # Its own tap: `brew install infisical` finds an unrelated formula.
    brew_offer infisical infisical/get-cli/infisical "injects each service's configuration at run time"

    # This exits 0 either way, so the check reads the output. One login per machine;
    # the domain is remembered in the profile.
    if command -v infisical >/dev/null 2>&1 \
      && infisical profile list 2>&1 | grep -q 'No login profiles found'; then
      read -rp "      infisical isn't logged in. Sign in now (opens a browser)? [y/N] " ans
      if [[ "$ans" =~ ^[Yy]$ ]]; then
        infisical login --domain https://infisical.mc2-dev.com || true
      else
        echo "        Later:  infisical login --domain https://infisical.mc2-dev.com"
        echo "        Until then no service can start — its configuration lives there."
      fi
    fi
  fi

  # virtualize --setup shells out to these and would fail partway, after writing configs.
  if [[ "$NEEDS_PYTHON" == true ]]; then
    brew_offer uv uv "Python toolchain — replaces pip and venv"
  fi
  if [[ "$NEEDS_RUST" == true ]]; then
    # Nothing else warns about a declared dependency nothing uses. No formula exists.
    tool_offer cargo-machete "cargo install cargo-machete" \
      "reports a dependency declared in Cargo.toml and never used"
  fi
  if [[ "$NEEDS_NODE" == true ]]; then
    brew_offer node node "runs the Nuxt apps"
    # The major does not matter: packageManager pins it and pnpm self-manages down.
    brew_offer pnpm pnpm "package manager for every JS repo"
  fi
  if [[ "$NEEDS_RUST" == true ]] && ! command -v cargo >/dev/null 2>&1; then
    # Not `brew install rustup`: that formula is keg-only and installs no toolchain, so
    # cargo still would not exist afterwards.
    read -rp "      cargo isn't installed (builds the Rust services). Install rustup now? [y/N] " ans
    if [[ "$ans" =~ ^[Yy]$ ]]; then
      run_with_spinner "rustup (installs the stable toolchain)" \
        bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
      # PATH for the rest of this script; rustup-init handles later sessions.
      if [[ -f "$HOME/.cargo/env" ]]; then
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
      fi
    fi
  fi
  echo ""

  MISSING=()
  FOUND=(git Homebrew tailscale gitleaks)
  command -v git >/dev/null 2>&1 || MISSING+=("git
        Run: xcode-select --install")
  # Unconditional: it gates every preset, since dev resolves only on the tailnet.
  if ! command -v gitleaks >/dev/null 2>&1; then
    MISSING+=("gitleaks
        Run: brew install gitleaks
        The pre-commit hook installed in every repo skips itself without it. Real
        credentials have already reached this org's git history once.")
  fi
  if ! command -v tailscale >/dev/null 2>&1 && [[ ! -d /Applications/Tailscale.app ]]; then
    MISSING+=("tailscale
        Run: brew install --cask tailscale
        Then open it, sign in, and have an admin add this machine to the tailnet.
        Every mc2-dev.com host resolves to a tailnet address — without this the
        dev tier is unreachable and the symptom looks like broken DNS.")
  fi
  command -v brew >/dev/null 2>&1 || MISSING+=("Homebrew
        Needed for the tools above. Install: https://brew.sh")

  if [[ "$NEEDS_CLUSTER" == true ]]; then
    FOUND+=(Docker kubectl helm argo argocd infisical)
    command -v docker >/dev/null 2>&1 || MISSING+=("Docker Desktop
        1. Download and install: https://www.docker.com/products/docker-desktop/
        2. Open it once (finishes first-time setup)
        3. Enable Kubernetes: Docker Desktop -> Settings -> Kubernetes -> Enable Kubernetes")
    command -v kubectl >/dev/null 2>&1 || MISSING+=("kubectl
        Run: brew install kubectl")
    command -v helm >/dev/null 2>&1 || MISSING+=("helm
        Run: brew install helm")
    command -v argo >/dev/null 2>&1 || MISSING+=("argo
        Run: brew install argo")
    command -v argocd >/dev/null 2>&1 || MISSING+=("argocd
        Run: brew install argocd")
    command -v infisical >/dev/null 2>&1 || MISSING+=("infisical
        Run: brew install infisical/get-cli/infisical
        Services read their configuration from it at startup — there is no .env to
        fall back on, so without this nothing starts.")
  fi

  if [[ "$NEEDS_PYTHON" == true ]]; then
    FOUND+=(uv)
    command -v uv >/dev/null 2>&1 || MISSING+=("uv
        Run: brew install uv")
  fi
  if [[ "$NEEDS_NODE" == true ]]; then
    FOUND+=(node pnpm)
    command -v node >/dev/null 2>&1 || MISSING+=("node
        Run: brew install node")
    command -v pnpm >/dev/null 2>&1 || MISSING+=("pnpm
        Run: brew install pnpm")
  fi
  if [[ "$NEEDS_RUST" == true ]]; then
    FOUND+=(cargo)
    command -v cargo >/dev/null 2>&1 || MISSING+=("cargo
        Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh")
  fi

  if [[ ${#MISSING[@]} -gt 0 ]]; then
    echo "      Missing:"
    for m in "${MISSING[@]}"; do echo "      - $m"; done
    echo ""
    echo "      Install these first, then re-run this script."
    exit 1
  fi
  echo "      ✓ ${FOUND[*]} all found"

  if [[ "$NEEDS_CLUSTER" == true ]] && ! kubectl config get-contexts docker-desktop >/dev/null 2>&1; then
    echo "      ! Docker Desktop's Kubernetes doesn't look enabled yet"
    echo "        Docker Desktop -> Settings -> Kubernetes -> Enable Kubernetes"
  fi
  echo ""

  echo "[3/5] Setting up SSH access to GitHub"
  if [[ -f "$SSH_KEY" ]]; then
    echo "      ✓ Found existing key at $SSH_KEY"
  else
    read -rp "      Email for the SSH key comment: " SSH_EMAIL
    echo ""
    ssh-keygen -t ed25519 -C "$SSH_EMAIL" -f "$SSH_KEY" -N ""
    echo ""
    echo "      Key generated. Add the public key below to GitHub:"
    echo "      https://github.com/settings/ssh/new"
    echo ""
    cat "$SSH_KEY.pub"
    echo ""
    read -rp "      Press enter once you've added it..." _
  fi

  # Reaching a private repo, not merely authenticating. Every valid GitHub key passes
  # "successfully authenticated", so the old check went green on a personal account
  # with no access here and the run failed at the clone, blaming the repository.
  whoami_gh() {
    ssh -T git@github.com -o IdentitiesOnly=yes -i "$SSH_KEY" 2>&1 \
      | sed -n 's/^Hi \([^!]*\)!.*/\1/p'
  }

  echo "      Testing access with $SSH_KEY"
  until GIT_SSH_COMMAND="ssh -i $SSH_KEY -o IdentitiesOnly=yes" \
    git ls-remote "git@github.com:${GITHUB_OWNER}/mc2-wrappers.git" >/dev/null 2>&1; do
    _who="$(whoami_gh)"
    echo ""
    if [[ -n "$_who" ]]; then
      echo "      This key authenticates as '$_who', which cannot see ${GITHUB_OWNER}'s"
      echo "      repositories. Either that account needs an invitation, or this is the"
      echo "      wrong key — a Mac with a work and a personal account has one of each:"
      echo ""
      echo "        MC2_SSH_KEY=~/.ssh/id_work ./bootstrap.sh --member"
    else
      echo "      Couldn't authenticate. Add this public key to the GitHub account that"
      echo "      belongs to ${GITHUB_OWNER} (https://github.com/settings/ssh/new):"
      echo ""
      cat "$SSH_KEY.pub"
    fi
    echo ""
    read -rp "      Press enter to retry..." _
  done
  echo "      ✓ access confirmed as $(whoami_gh)"
  echo ""

  echo "[4/5] Cloning into $MC2_DIR"
  mkdir -p "$MC2_DIR"
  for repo in "${REPOS[@]}"; do
    dest="$MC2_DIR/$repo"
    [[ -d "$dest" ]] && continue
    GIT_SSH_COMMAND="ssh -i $SSH_KEY -o IdentitiesOnly=yes" \
      run_with_spinner "$repo" git clone --quiet "git@github.com:${GITHUB_OWNER}/${repo}.git" "$dest"
  done
  echo ""

  # core.hooksPath at one committed directory, so updating a hook is a git pull.
  if [[ -x "$MC2_DIR/mc2-wrappers/virtualize" ]]; then
    echo "      Installing git hooks (core.hooksPath -> mc2-wrappers/hooks)"
    "$MC2_DIR/mc2-wrappers/virtualize" --install-hooks 2>&1 | sed 's/^/      /'
  else
    echo "      ! mc2-wrappers not cloned — no git hooks installed."
    echo "        After cloning it: virtualize --install-hooks"
  fi
  echo ""

  echo "[5/5] Linking CLI tools"
  NEED_LINK=false
  [[ "$(readlink /usr/local/bin/kube 2>/dev/null)" != "$MC2_DIR/mc2-wrappers/kube" ]] && NEED_LINK=true
  [[ "$(readlink /usr/local/bin/virtualize 2>/dev/null)" != "$MC2_DIR/mc2-wrappers/virtualize" ]] && NEED_LINK=true
  [[ "$(readlink /usr/local/bin/flow 2>/dev/null)" != "$MC2_DIR/mc2-wrappers/flow" ]] && NEED_LINK=true

  # A custom selection can leave mc2-wrappers unticked; linking anyway dangles.
  if [[ ! -d "$MC2_DIR/mc2-wrappers" ]]; then
    NEED_LINK=false
    echo "      ! mc2-wrappers was not cloned — skipping. 'kube', 'virtualize' and 'flow' will"
    echo "        not exist until you clone it and re-run this script."
  fi

  if [[ "$NEED_LINK" == true ]]; then
    sudo ln -sf "$MC2_DIR/mc2-wrappers/kube" /usr/local/bin/kube
    sudo ln -sf "$MC2_DIR/mc2-wrappers/virtualize" /usr/local/bin/virtualize
    sudo ln -sf "$MC2_DIR/mc2-wrappers/flow" /usr/local/bin/flow
    echo "      ✓ kube, virtualize and flow linked into /usr/local/bin"
  else
    echo "      ✓ kube, virtualize and flow already correctly linked"
  fi
  echo ""

  # Docker Desktop's ClusterIPs are unreachable from the host, unlike dev and prod where
  # the node advertises the service CIDR. The agent keeps that tunnel up across reboots.
  if [[ -x "$MC2_DIR/mc2-wrappers/virtualize" ]]; then
    if launchctl print "gui/$UID/com.mc2.fwd.local" >/dev/null 2>&1; then
      echo "      ✓ local forwards already running as a launchd agent"
    else
      "$MC2_DIR/mc2-wrappers/virtualize" --fwd-install >/dev/null 2>&1 \
        && echo "      ✓ local forwards installed (starts at login; virtualize --fwd-status)" \
        || echo "      ! could not install the local forward agent — run: virtualize --fwd-install"
    fi
  fi
  echo ""

  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  if [[ "$SCRIPT_DIR" == "$MC2_DIR/mc2-bootstrap" ]]; then
    : # already in place
  elif [[ -d "$MC2_DIR/mc2-bootstrap" ]]; then
    echo "Note: $MC2_DIR/mc2-bootstrap already exists — leaving $SCRIPT_DIR where it is."
  elif [[ "$(basename "$SCRIPT_DIR")" != "mc2-bootstrap" || ! -d "$SCRIPT_DIR/.git" ]]; then
    # This script gets copied around on its own, so $SCRIPT_DIR is often /tmp or
    # ~/Downloads; moving that would drag every unrelated file with it.
    echo "Note: $SCRIPT_DIR is not an mc2-bootstrap checkout — leaving it where it is."
    echo "      Clone it properly if you want it alongside the other repos:"
    echo "        git clone git@github.com:${GITHUB_OWNER}/mc2-bootstrap.git $MC2_DIR/mc2-bootstrap"
  else
    mv "$SCRIPT_DIR" "$MC2_DIR/mc2-bootstrap"
    echo "✓ Moved mc2-bootstrap into $MC2_DIR/mc2-bootstrap, alongside the other repos"
  fi
  echo ""

  echo "--------------------"
  # Keyed on what was cloned, not the preset: advice that cannot work is worse than none.
  if [[ "$NEEDS_CLUSTER" == false ]]; then
    # Name a project that was actually cloned; an example pointing at a missing
    # directory is noise.
    _example_app="mc2-platform-frontend"
    _example_port=":3001"
    for _candidate in mc2-platform-frontend mc2-operation-frontend mc2-accounting-frontend mc2-ui; do
      if [[ -d "$MC2_DIR/$_candidate" ]]; then
        _example_app="$_candidate"
        case "$_candidate" in
          mc2-platform-frontend) _example_port=":3001" ;;
          mc2-operation-frontend) _example_port=":3000" ;;
          mc2-accounting-frontend) _example_port=":3002" ;;
          mc2-ui) _example_port=":6006, Storybook" ;;
        esac
        break
      fi
    done
    echo "You're set up. Two commands and you're running:"
    echo ""
    echo "  cd $MC2_DIR"
    echo "  virtualize --setup -e dev                       # installs deps, points projects at dev"
    echo "  virtualize -p $_example_app --start   # $_example_port"
    echo ""
    echo "Nothing is written to this machine: a service reads its settings from"
    echo "mc2-k8s/overlays/<tier> and its credentials from Infisical, straight into the"
    echo "process. If --start says you are not logged in:"
    echo "  infisical login --domain https://infisical.mc2-dev.com"
    echo ""
    echo "Reaching *-dev.mc2-dev.com needs Tailscale — ask to be added to the network."
    echo "Move one app to another tier with 'virtualize -p <project> --switch-dev' (or"
    echo "--switch-local / --switch-prod); 'virtualize --envs' shows where each one points."
  else
    echo "You're set up. A few manual steps left to bring the cluster up:"
    echo ""
    echo "  cd $MC2_DIR"
    echo "  kube --install-traefik   # one-time"
    echo "  kube --reboot"
    echo "  virtualize --setup       # venvs, IDE configs, forwards"
    echo ""
    echo "Nothing is copied to this machine. A service reads its settings from"
    echo "mc2-k8s/overlays/<tier> and its credentials from Infisical, both straight into"
    echo "the process:"
    echo ""
    echo "  virtualize -p mc2-gateway --switch-dev"
    echo "  virtualize -p mc2-gateway --start      # or --ide, so the IDE inherits them"
  fi
}

case "$MODE" in
  server) run_server_bootstrap ;;
  *) run_member_bootstrap ;;
esac
