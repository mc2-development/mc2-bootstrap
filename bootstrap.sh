#!/bin/bash
# MC2 local dev bootstrap.
#
# Sets up SSH access to GitHub, creates ~/Work/mc2, clones the repos needed to
# run Quotance locally, and symlinks the kube/virtualize CLI tools. Does not
# touch the cluster itself — that's the last manual step, printed at the end.
set -e

GITHUB_OWNER="${GITHUB_OWNER:-mc2-development}"

WORK_DIR="$HOME/Work"
MC2_DIR="$WORK_DIR/mc2"
REPOS=(mc2-wrappers mc2-k8s mc2-configs mc2-cors mc2-operation-api mc2-accounting-api mc2-agent-api mc2-quotance-frontend)

# Default filename — ssh tries this automatically with no ~/.ssh/config needed,
# as long as it's the only key on the machine.
SSH_KEY="$HOME/.ssh/id_ed25519"

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
  wait "$pid"
  local status=$?
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
checkbox_menu() {
  set +e
  local items=("$@")
  local n=${#items[@]}
  local checked=() locked=()
  local i cur key key2

  for ((i = 0; i < n; i++)); do
    checked[i]=1
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
      if [[ $i -eq $cur ]]; then
        printf "\r\033[K  > [%s] %s\n" "$mark" "${items[i]}"
      else
        printf "\r\033[K    [%s] %s\n" "$mark" "${items[i]}"
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
echo "Local dev bootstrap"
echo "--------------------"
echo ""

echo "[1/4] Checking prerequisites"

if ! command -v brew >/dev/null 2>&1; then
  read -rp "      Homebrew isn't installed (needed for kubectl/helm). Install it now? [y/N] " ans
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

if ! command -v kubectl >/dev/null 2>&1 && command -v brew >/dev/null 2>&1; then
  read -rp "      kubectl isn't installed. Install it now via Homebrew? [y/N] " ans
  [[ "$ans" =~ ^[Yy]$ ]] && run_with_spinner "brew install kubectl" brew install kubectl
fi

if ! command -v helm >/dev/null 2>&1 && command -v brew >/dev/null 2>&1; then
  read -rp "      helm isn't installed. Install it now via Homebrew? [y/N] " ans
  [[ "$ans" =~ ^[Yy]$ ]] && run_with_spinner "brew install helm" brew install helm
fi
echo ""

MISSING=()
command -v git >/dev/null 2>&1 || MISSING+=("git
        Run: xcode-select --install")
command -v brew >/dev/null 2>&1 || MISSING+=("Homebrew
        Needed for kubectl/helm below. Install: https://brew.sh")
command -v docker >/dev/null 2>&1 || MISSING+=("Docker Desktop
        1. Download and install: https://www.docker.com/products/docker-desktop/
        2. Open it once (finishes first-time setup)
        3. Enable Kubernetes: Docker Desktop -> Settings -> Kubernetes -> Enable Kubernetes")
command -v kubectl >/dev/null 2>&1 || MISSING+=("kubectl
        Run: brew install kubectl")
command -v helm >/dev/null 2>&1 || MISSING+=("helm
        Run: brew install helm")

if [[ ${#MISSING[@]} -gt 0 ]]; then
  echo "      Missing:"
  for m in "${MISSING[@]}"; do echo "      - $m"; done
  echo ""
  echo "      Install these first, then re-run this script."
  exit 1
fi
echo "      ✓ git, Homebrew, Docker, kubectl, helm all found"

if ! kubectl config get-contexts docker-desktop >/dev/null 2>&1; then
  echo "      ! Docker Desktop's Kubernetes doesn't look enabled yet"
  echo "        Docker Desktop -> Settings -> Kubernetes -> Enable Kubernetes"
fi
echo ""

echo "[2/4] Setting up SSH access to GitHub"
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

echo "[3/4] Choose repositories to clone (↑/↓ move, space toggle, enter confirm)"
echo ""
checkbox_menu "${REPOS[@]}"
REPOS=("${CHECKED_ITEMS[@]}")
echo ""

mkdir -p "$MC2_DIR"
echo "Cloning into $MC2_DIR"
for repo in "${REPOS[@]}"; do
  dest="$MC2_DIR/$repo"
  [[ -d "$dest" ]] && continue
  GIT_SSH_COMMAND="ssh -i $SSH_KEY -o IdentitiesOnly=yes" \
    run_with_spinner "$repo" git clone --quiet "git@github.com:${GITHUB_OWNER}/${repo}.git" "$dest"
done
echo ""

SECRETS_EXAMPLE="$MC2_DIR/mc2-configs/local.secrets.example.yaml"
SECRETS_FILE="$MC2_DIR/mc2-configs/local.secrets.yaml"
if [[ -f "$SECRETS_EXAMPLE" && ! -f "$SECRETS_FILE" ]]; then
  cp "$SECRETS_EXAMPLE" "$SECRETS_FILE"
  echo "✓ Created $SECRETS_FILE from the template (gitignored)"
  echo "  Only needed if you ever run 'virtualize --install' without --local — fill in"
  echo "  your own GitHub token there if that comes up. See mc2-configs/README.md."
  echo ""
fi

echo "[4/4] Linking CLI tools"
NEED_LINK=false
[[ "$(readlink /usr/local/bin/kube 2>/dev/null)" != "$MC2_DIR/mc2-wrappers/kube" ]] && NEED_LINK=true
[[ "$(readlink /usr/local/bin/virtualize 2>/dev/null)" != "$MC2_DIR/mc2-wrappers/virtualize" ]] && NEED_LINK=true

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
else
  mv "$SCRIPT_DIR" "$MC2_DIR/mc2-bootstrap"
  echo "✓ Moved mc2-bootstrap into $MC2_DIR/mc2-bootstrap, alongside the other repos"
fi
echo ""

echo "--------------------"
echo "You're set up. A few manual steps left to bring the cluster up:"
echo ""
echo "  cd $MC2_DIR"
echo "  kube --install-traefik   # one-time"
echo "  kube --reboot"
echo "  virtualize --setup"
