#!/bin/bash
# MC2 bootstrap — prepares a machine. Two modes, one script.
#
#   --member  (default)  A developer's Mac. Sets up SSH access to GitHub, creates
#                        ~/Work/mc2, clones the repos needed to run Quotance
#                        locally, and symlinks the kube/virtualize CLI tools.
#                        Does not touch the cluster — that's printed at the end.
#
#   --server             A Linux host that will run the k3s cluster. Installs k3s
#                        (with its bundled Traefik) and nothing else: no repos, no
#                        GitHub SSH, no Homebrew, no CLI symlinks. A server holding
#                        the database has no business holding a git checkout.
#
# Server mode runs ON the box. The repos are private, so there is no curl|bash URL;
# copy this one file over and run it from a shell on the server:
#
#   scp mc2-bootstrap/bootstrap.sh root@<server-ip>:/tmp/
#   ssh root@<server-ip>
#   bash /tmp/bootstrap.sh --server
#
# Provisioning one box once is not worth automating, and if k3s fails you want to
# already be on the machine with journalctl. The one-liner form is for later, when
# this is repeatable (a rebuild, a second node, CI) — and it needs a TTY, or the
# passphrase prompt echoes in clear:
#
#   ssh -t root@<ip> 'bash /tmp/bootstrap.sh --server'
#   ssh root@<ip> 'MC2_SERVER_PASSPHRASE=... bash /tmp/bootstrap.sh --server'
set -e

GITHUB_OWNER="${GITHUB_OWNER:-mc2-development}"

WORK_DIR="$HOME/Work"
MC2_DIR="$WORK_DIR/mc2"
REPOS=(mc2-wrappers mc2-k8s mc2-configs mc2-core mc2-python mc2-gateway mc2-account-api mc2-crons mc2-operation-api mc2-accounting-api mc2-agent-api mc2-mailer-api mc2-operation-frontend mc2-accounting-frontend mc2-platform-frontend mc2-ui)

# Default filename — ssh tries this automatically with no ~/.ssh/config needed,
# as long as it's the only key on the machine.
SSH_KEY="$HOME/.ssh/id_ed25519"

# --- server mode settings -----------------------------------------------------

# SHA-256 of the passphrase that unlocks --server. Only the hash lives here, so
# reading this file does not hand anyone the phrase.
#
# This is an ACCIDENT GUARD, not a security control: anyone with root on a box can
# edit the check out. Its whole job is to stop a teammate running --server by
# mistake on a machine that should have been --member.
#
# To change it:  printf '%s' 'your passphrase' | shasum -a 256
SERVER_PASSPHRASE_SHA256="61c41eff37596494eb833b148164f5e297c10c5b02ca33d57fe2f34fc1dd97c3"

KUBECONFIG_OUT="/root/mc2-hetzner.kubeconfig"
KUBE_CONTEXT_NAME="mc2-hetzner"

# Pinned so a rebuild, or a second node added months from now, reproduces THIS
# cluster rather than whatever "stable" happens to point at that day. Unpinned,
# the install line is a moving target and two nodes can end up on different
# minor versions.
#
# Update path: check the channel list, bump, re-provision a throwaway box, test.
#     curl -s https://update.k3s.io/v1-release/channels | grep -o 'v1[^"]*k3s1' | head
K3S_VERSION="v1.36.4+k3s1"

MODE="member"
IP_OVERRIDE=""
usage() {
  cat << 'EOF'
Usage: bootstrap.sh [--member | --server] [options]

Modes:
  --member          Set up a developer Mac (default). Repos, GitHub SSH, CLI tools.
  --server          Set up a Linux k3s host. Installs k3s only — no repos, no SSH keys.

Options:
  --ip <address>    Server mode: override the auto-detected public IP.
  -h, --help        Show this message.

Server mode reads MC2_SERVER_PASSPHRASE from the environment if set, so it can run
unattended; otherwise it prompts.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --server) MODE="server"; shift ;;
    --member) MODE="member"; shift ;;
    --ip)     IP_OVERRIDE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1"; echo ""; usage; exit 1 ;;
  esac
done

# Runs a command in the background with a spinner, prints a check/cross when done.
run_with_spinner() {
  local msg="$1"; shift
  local log; log="$(mktemp)"
  "$@" >"$log" 2>&1 &
  local pid=$!
  local frames='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
  local i=0
  while kill -0 "$pid" 2>/dev/null; do
    printf "\r      %s %s" "${frames:$i:1}" "$msg"
    i=$(( (i + 1) % ${#frames} ))
    sleep 0.1
  done
  # `wait` under `set -e` would abort the script on a non-zero child before the
  # failure branch below could run — so the log would never be printed and the
  # temp file would leak. Capture the status instead of letting errexit have it.
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

# Arrow-key checkbox menu. Items passed as args, all ticked by default.
# Items already present in $MC2_DIR show dimmed/locked — skipped by navigation,
# always included in the result. Space toggles, enter confirms.
# Result left in $CHECKED_ITEMS array.
# Repos that exist in the org but hold nothing yet. Shown, selectable, but never
# checked by default — cloning one gets you an empty directory, and finding that out
# afterwards is the kind of small confusion a first day does not need.
repo_note() {
  case "$1" in
    mc2-mailer-api) echo " — placeholder, not started" ;;
    *) echo "" ;;
  esac
}

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
    if [[ ${locked[i]} -eq 0 ]]; then cur=$i; break; fi
  done

  draw() {
    for ((i = 0; i < n; i++)); do
      if [[ ${locked[i]} -eq 1 ]]; then
        printf "\r\033[K    \033[2m[x] %s (already installed)\033[0m\n" "${items[i]}"
        continue
      fi
      local mark=" "
      [[ ${checked[i]} -eq 1 ]] && mark="x"
      local note; note="$(repo_note "${items[i]}")"
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
          [[ ${locked[i]} -eq 0 ]] && { cur=$i; break; }
        done
      elif [[ $key2 == "[B" ]]; then
        for ((i = cur + 1; i < n; i++)); do
          [[ ${locked[i]} -eq 0 ]] && { cur=$i; break; }
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
  cat << 'EOF'
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

  # --- guard 1: platform -------------------------------------------------------
  # Checked before the passphrase: running --server on a Mac is the likeliest
  # mistake, and it deserves a clearer message than "wrong passphrase".
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
  else
    # `read -s` can only disable echo when stdin is a terminal. Plain
    # `ssh host 'bash script'` does NOT allocate one, so the passphrase would be
    # typed in clear on screen — it still works, which is exactly why it would go
    # unnoticed. Refuse and name the fix rather than leak it.
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
    # Hetzner's metadata service is authoritative on a Hetzner box; fall back to
    # asking the outside world, then to whatever the default route uses.
    public_ip="$(curl -sf --max-time 3 http://169.254.169.254/hetzner/v1/metadata/public-ipv4 2>/dev/null || true)"
    [[ -z "$public_ip" ]] && public_ip="$(curl -sf --max-time 5 https://ifconfig.me 2>/dev/null || true)"
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
  read -rp "Type the hostname to confirm: " typed
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

  # ufw is on by default in some Ubuntu images and is a known k3s footgun: it
  # filters the flannel VXLAN traffic and the pod/service CIDRs, so the cluster
  # comes up looking healthy and then DNS and pod-to-pod networking quietly fail.
  # We firewall at the Hetzner Cloud layer instead, which sits in front of the host
  # and cannot break the cluster's internal networking.
  if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -qi '^Status: active'; then
    echo ""
    echo "      ! ufw is active. k3s needs it off, or explicitly opened for the"
    echo "        pod (10.42.0.0/16) and service (10.43.0.0/16) CIDRs — otherwise"
    echo "        pod networking and CoreDNS break in ways that look like app bugs."
    echo ""
    echo "        Recommended: disable ufw and firewall at the Hetzner Cloud layer"
    echo "        (rules printed at the end of this script)."
    echo ""
    read -rp "      Disable ufw now? [y/N] " ufw_ans
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
  echo ""

  # --- [2/3] k3s ---------------------------------------------------------------
  echo "[2/3] Installing k3s"
  if systemctl is-active --quiet k3s 2>/dev/null; then
    echo "      ✓ k3s already running — leaving it alone"
  else
    # --tls-san puts the public IP in the API server certificate. Without it a
    # kubeconfig pointed at the public address fails verification, since k3s only
    # signs for 127.0.0.1 and the internal IP by default.
    #
    # --secrets-encryption encrypts Secrets at rest. Off by default, and this is the
    # cheap moment: turning it on later needs a server restart. Without it, Secrets
    # sit base64-encoded in the embedded SQLite datastore — so a stolen disk, a
    # cloned volume or a Hetzner snapshot hands over every database password in
    # plaintext. It does not defend against root on this node (the key lives on the
    # same disk, and with no cloud KMS on Hetzner that is the ceiling), but it does
    # defend against the backup-and-snapshot cases, which are the realistic ones.
    #
    # Traefik and servicelb are deliberately NOT disabled: Traefik is the ingress
    # we want, and klipper (servicelb) is what gives it an address on a single node.
    #
    # If Tailscale is up, its address goes into the certificate too: with the
    # firewall fully closed, kubectl reaches the API server over the tunnel, and
    # a SAN can only be added later by editing k3s config and restarting.
    tls_sans="--tls-san $public_ip"
    ts_ip="$(command -v tailscale >/dev/null 2>&1 && tailscale ip -4 2>/dev/null | head -1 || true)"
    if [[ -n "$ts_ip" ]]; then
      echo "      ✓ tailscale detected ($ts_ip) — adding it to the API certificate"
      tls_sans+=" --tls-san $ts_ip"
    else
      echo "      ! no tailscale interface — the API certificate will only cover $public_ip."
      echo "        For VPN-only kubectl access, install tailscale first: see NEXT STEPS."
    fi
    run_with_spinner "curl get.k3s.io | sh ($K3S_VERSION)" \
      env INSTALL_K3S_VERSION="$K3S_VERSION" \
          INSTALL_K3S_EXEC="$tls_sans --secrets-encryption" \
      bash -c 'curl -sfL https://get.k3s.io | sh -'

    run_with_spinner "waiting for the node to become Ready" \
      bash -c 'for i in $(seq 1 60); do
                 /usr/local/bin/k3s kubectl get nodes 2>/dev/null | grep -q " Ready " && exit 0
                 sleep 2
               done
               exit 1'
  fi
  echo ""

  # Confirm at-rest encryption actually came up, rather than assuming the flag took.
  # Absolute path on purpose: /usr/local/bin is not always on root's PATH in a
  # non-login shell, and a swallowed command-not-found here would report a false
  # negative on a cluster that is in fact encrypted.
  if /usr/local/bin/k3s secrets-encrypt status 2>/dev/null | grep -qi "enabled"; then
    echo "      ✓ secrets encrypted at rest"
  else
    echo "      ! secrets-at-rest encryption is NOT active. Check: k3s secrets-encrypt status"
    echo "        If k3s pre-dated this script, enabling it needs a restart:"
    echo "        add 'secrets-encryption: true' to /etc/rancher/k3s/config.yaml && systemctl restart k3s"
  fi
  echo ""

  # --- [3/3] kubeconfig --------------------------------------------------------
  echo "[3/3] Writing a remote-ready kubeconfig"
  # k3s names the cluster, context and user all "default". Renaming matters: a Mac
  # is likely to already hold other kubeconfigs using that same name, and picking
  # the wrong "default" is how you deploy to the wrong cluster.
  # Prefer the tailscale address: with the firewall fully closed, the tunnel is
  # the only route that reaches 6443 at all.
  kube_addr="${ts_ip:-$public_ip}"
  sed -e "s|https://127.0.0.1:6443|https://${kube_addr}:6443|" \
      -e "s|name: default|name: ${KUBE_CONTEXT_NAME}|g" \
      -e "s|cluster: default|cluster: ${KUBE_CONTEXT_NAME}|g" \
      -e "s|user: default|user: ${KUBE_CONTEXT_NAME}|g" \
      -e "s|current-context: default|current-context: ${KUBE_CONTEXT_NAME}|g" \
      /etc/rancher/k3s/k3s.yaml > "$KUBECONFIG_OUT"
  chmod 600 "$KUBECONFIG_OUT"
  echo "      ✓ $KUBECONFIG_OUT (context: $KUBE_CONTEXT_NAME)"
  echo ""

  echo "--------------------"
  echo "k3s is up. Two things left, both from your Mac:"
  echo ""
  echo "1. Fetch the kubeconfig — keep it as its own file, do not merge it:"
  echo ""
  echo "     scp root@${public_ip}:${KUBECONFIG_OUT} ~/.kube/hetzner.yaml"
  echo "     export KUBECONFIG=~/.kube/hetzner.yaml"
  echo "     kubectl get nodes"
  echo ""
  echo "2. Lock the firewall down (Hetzner Cloud Console → Firewalls, or hcloud):"
  echo ""
  echo "     NO inbound rules at all. Create the firewall empty and ATTACH it to"
  echo "     the server — an unattached firewall filters nothing."
  echo ""
  echo "   Nothing needs to be open: ssh, the kubernetes API and (later) https all"
  echo "   ride the tailscale tunnel, which is outbound-only. A port scan of the"
  echo "   public IP should find nothing. Certificates come from cert-manager over"
  echo "   dns01, so not even :80 is needed for ACME."
  echo ""
  echo "   Postgres (5432) is never exposed either — it stays a ClusterIP service"
  echo "   reached with 'kubectl port-forward' over the tunnel."
  echo ""
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

  # A role preset, because "which of these sixteen repos do I need?" is a question a
  # new developer cannot answer on their first day — and the answer decides how much
  # setup they face afterwards. A frontend checkout holds nothing that reads a secret,
  # so virtualize asks it for no credentials at all.
  #
  # Asked first, before prerequisites, because the answer decides which toolchains are
  # needed: there is no reason to make a frontend developer install helm.
  echo "[1/5] What will you be working on?"
  echo ""
  echo "  1) Frontend   — the three apps and the shared UI layer. No backend, no database."
  echo "  2) Backend    — the APIs, the gateway, migrations and the shared Python library."
  echo "  3) Everything — the whole platform."
  echo "  4) Choose repositories myself"
  echo ""
  read -rp "  Select [1-4]: " _role
  echo ""

  BASE_REPOS=(mc2-wrappers mc2-k8s mc2-configs)
  FRONTEND_REPOS=(mc2-ui mc2-operation-frontend mc2-accounting-frontend mc2-platform-frontend)
  # mc2-mailer-api is deliberately absent: it is an empty repo until the core platform
  # is stable (its own README says so). Still reachable through "choose myself".
  BACKEND_REPOS=(mc2-core mc2-python mc2-gateway mc2-account-api mc2-operation-api mc2-accounting-api mc2-agent-api mc2-crons)

  ROLE="everything"
  case "$_role" in
    1) ROLE="frontend"; REPOS=("${BASE_REPOS[@]}" "${FRONTEND_REPOS[@]}") ;;
    2) ROLE="backend";  REPOS=("${BASE_REPOS[@]}" "${BACKEND_REPOS[@]}") ;;
    3) REPOS=("${BASE_REPOS[@]}" "${FRONTEND_REPOS[@]}" "${BACKEND_REPOS[@]}") ;;
    4) echo "  (↑/↓ move, space toggle, enter confirm)"
       echo ""
       checkbox_menu "${REPOS[@]}"
       REPOS=("${CHECKED_ITEMS[@]}")
       ROLE="custom" ;;
    *) echo "  Unrecognised choice — cloning everything."
       REPOS=("${BASE_REPOS[@]}" "${FRONTEND_REPOS[@]}" "${BACKEND_REPOS[@]}") ;;
  esac
  echo "  Selected: $ROLE (${#REPOS[@]} repositories)"
  echo ""

  # Which toolchains the selection actually needs. Derived from the repos rather than
  # the role label, so "choose myself" gets the same treatment as a preset.
  NEEDS_PYTHON=false; NEEDS_RUST=false; NEEDS_NODE=false
  for repo in "${REPOS[@]}"; do
    case "$repo" in
      mc2-operation-api|mc2-accounting-api|mc2-agent-api|mc2-crons|mc2-python|mc2-mailer-api) NEEDS_PYTHON=true ;;
      mc2-gateway|mc2-account-api|mc2-core) NEEDS_RUST=true ;;
      mc2-ui|mc2-operation-frontend|mc2-accounting-frontend|mc2-platform-frontend) NEEDS_NODE=true ;;
    esac
  done
  # Only a backend checkout runs the cluster locally. A frontend developer points at
  # the shared dev namespace and needs no kubeconfig at all.
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

  # Offers to install anything missing that this checkout will actually use. Kept as
  # offers rather than silent installs: this runs on someone's own machine.
  #
  # Declining must leave status 0: under `set -e` a non-zero return from a function
  # call aborts the whole script — silently, since the "failure" is just answering N.
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

  if [[ "$NEEDS_CLUSTER" == true ]]; then
    brew_offer kubectl kubectl "talks to the cluster"
    brew_offer helm    helm    "installs Traefik locally"
    # The pipeline runs in the cluster, so reading it means talking to the cluster.
    # Both CLIs work off ~/.kube/hetzner.yaml — argo needs nothing else; argocd wants
    # one `argocd login argocd.mc2-dev.com` whose session then persists.
    brew_offer argo    argo    "reads the build pipeline (argo workflows)"
    brew_offer argocd  argocd  "reads what is deployed (argo cd)"
  fi

  # The language toolchains virtualize --setup shells out to. Without these it fails
  # partway through, after the configs are already written — so they are checked here,
  # before anything is cloned, rather than discovered later.
  if [[ "$NEEDS_PYTHON" == true ]]; then
    brew_offer uv uv "Python toolchain — replaces pip and venv"
  fi
  if [[ "$NEEDS_NODE" == true ]]; then
    brew_offer node node "runs the Nuxt apps"
    # brew ships pnpm 12; the repos pin pnpm 11 via package.json's packageManager
    # field, and pnpm self-manages down to it (manage-package-manager-versions,
    # on by default since pnpm 10). So the major version here does not matter.
    brew_offer pnpm pnpm "package manager for every JS repo"
  fi
  if [[ "$NEEDS_RUST" == true ]] && ! command -v cargo >/dev/null 2>&1; then
    # Deliberately NOT `brew install rustup`: that formula is keg-only, no longer ships
    # rustup-init, and installs no toolchain on its own — so cargo still would not
    # exist afterwards. The upstream installer places cargo at ~/.cargo/bin and adds it
    # to the shell profile itself, which is what every Rust toolchain doc assumes.
    read -rp "      cargo isn't installed (builds the Rust services). Install rustup now? [y/N] " ans
    if [[ "$ans" =~ ^[Yy]$ ]]; then
      run_with_spinner "rustup (installs the stable toolchain)" \
        bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
      # Puts cargo on PATH for the rest of THIS script; rustup-init has already added
      # it to the shell profile for later sessions.
      if [[ -f "$HOME/.cargo/env" ]]; then
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
      fi
    fi
  fi
  echo ""

  MISSING=()
  FOUND=(git Homebrew)
  command -v git >/dev/null 2>&1 || MISSING+=("git
        Run: xcode-select --install")
  command -v brew >/dev/null 2>&1 || MISSING+=("Homebrew
        Needed for the tools above. Install: https://brew.sh")

  if [[ "$NEEDS_CLUSTER" == true ]]; then
    FOUND+=(Docker kubectl helm argo argocd)
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

  echo "      Testing connection..."
  until ssh -T git@github.com -o IdentitiesOnly=yes -i "$SSH_KEY" 2>&1 | grep -q "successfully authenticated"; do
    echo ""
    echo "      Couldn't authenticate yet. Make sure this public key is added to your"
    echo "      GitHub account (https://github.com/settings/ssh/new):"
    echo ""
    cat "$SSH_KEY.pub"
    echo ""
    read -rp "      Press enter to retry..." _
  done
  echo "      ✓ SSH access confirmed"
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

  echo "[5/5] Linking CLI tools"
  NEED_LINK=false
  [[ "$(readlink /usr/local/bin/kube 2>/dev/null)" != "$MC2_DIR/mc2-wrappers/kube" ]] && NEED_LINK=true
  [[ "$(readlink /usr/local/bin/virtualize 2>/dev/null)" != "$MC2_DIR/mc2-wrappers/virtualize" ]] && NEED_LINK=true

  # A custom selection can leave mc2-wrappers unticked. Linking anyway would create a
  # dangling symlink and report success — the worst of both.
  if [[ ! -d "$MC2_DIR/mc2-wrappers" ]]; then
    NEED_LINK=false
    echo "      ! mc2-wrappers was not cloned — skipping. 'kube' and 'virtualize' will"
    echo "        not exist until you clone it and re-run this script."
  fi

  if [[ "$NEED_LINK" == true ]]; then
    sudo ln -sf "$MC2_DIR/mc2-wrappers/kube" /usr/local/bin/kube
    sudo ln -sf "$MC2_DIR/mc2-wrappers/virtualize" /usr/local/bin/virtualize
    echo "      ✓ kube and virtualize linked into /usr/local/bin"
  else
    echo "      ✓ kube and virtualize already correctly linked"
  fi
  echo ""

  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  if [[ "$SCRIPT_DIR" == "$MC2_DIR/mc2-bootstrap" ]]; then
    : # already in place
  elif [[ -d "$MC2_DIR/mc2-bootstrap" ]]; then
    echo "Note: $MC2_DIR/mc2-bootstrap already exists — leaving $SCRIPT_DIR where it is."
  elif [[ "$(basename "$SCRIPT_DIR")" != "mc2-bootstrap" || ! -d "$SCRIPT_DIR/.git" ]]; then
    # The whole premise of this script is that it gets copied around on its own,
    # so $SCRIPT_DIR is often just whatever directory it was dropped in — /tmp,
    # ~/Downloads. Moving THAT would drag every unrelated file with it.
    echo "Note: $SCRIPT_DIR is not an mc2-bootstrap checkout — leaving it where it is."
    echo "      Clone it properly if you want it alongside the other repos:"
    echo "        git clone git@github.com:${GITHUB_OWNER}/mc2-bootstrap.git $MC2_DIR/mc2-bootstrap"
  else
    mv "$SCRIPT_DIR" "$MC2_DIR/mc2-bootstrap"
    echo "✓ Moved mc2-bootstrap into $MC2_DIR/mc2-bootstrap, alongside the other repos"
  fi
  echo ""

  echo "--------------------"
  # Keyed on what was actually cloned, not the preset name: a custom selection of only
  # frontend repos was never offered kubectl, so telling it to run `kube --reboot`
  # would be advice that cannot work.
  if [[ "$NEEDS_CLUSTER" == false ]]; then
    # No local cluster in this path on purpose: the dev tier's APIs are deployed, so
    # a frontend developer needs an app and a network, not Kubernetes on their Mac.
    # Name a project that was actually cloned — a custom selection may not include
    # platform-frontend, and an example pointing at a missing directory is noise.
    _example_app="mc2-platform-frontend"; _example_port=":3001"
    for _candidate in mc2-platform-frontend mc2-operation-frontend mc2-accounting-frontend mc2-ui; do
      if [[ -d "$MC2_DIR/$_candidate" ]]; then
        _example_app="$_candidate"
        case "$_candidate" in
          mc2-platform-frontend)   _example_port=":3001" ;;
          mc2-operation-frontend)  _example_port=":3000" ;;
          mc2-accounting-frontend) _example_port=":3002" ;;
          mc2-ui)                  _example_port=":6006, Storybook" ;;
        esac
        break
      fi
    done
    echo "You're set up. Two commands and you're running:"
    echo ""
    echo "  cd $MC2_DIR"
    echo "  virtualize --setup -e dev                       # installs deps, writes dev configs"
    echo "  virtualize -p $_example_app --start   # $_example_port"
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
    echo "  virtualize --setup       # generates your configs, then venvs + IDE + forwards"
    echo ""
    echo "To work against the dev cluster instead of your own, take its credentials"
    echo "from the cluster rather than from anyone's keyboard:"
    echo ""
    echo "  virtualize --pull-configs -e dev"
    echo "  virtualize --setup -e dev"
  fi
}

case "$MODE" in
  server) run_server_bootstrap ;;
  *)      run_member_bootstrap ;;
esac
