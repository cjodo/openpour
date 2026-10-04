#!/bin/sh
# Runs the Wokwi simulation of the firmware with wokwi-cli.
#
#   firmware/wokwi/run.sh [--build] [wokwi-cli options...]
#
#   --build    rebuild first (otherwise it builds only if the image or chips are missing)
#   anything else goes to wokwi-cli, e.g. --timeout 120000, --scenario x.yaml,
#   --serial-log-file serial.log, --interactive
#
# The token (free, from https://wokwi.com/dashboard/ci) comes from, in order:
#   1. $WOKWI_CLI_TOKEN
#   2. the file $WOKWI_TOKEN_FILE, default ~/.wokwi/token. It may hold the bare
#      token or a line like `WOKWI_CLI_TOKEN=...` / `export WOKWI_CLI_TOKEN="..."`;
#      blank lines and # comments are ignored.
# The token is never printed.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
token_file=${WOKWI_TOKEN_FILE:-$HOME/.wokwi/token}

die() {
  printf 'run.sh: %s\n' "$1" >&2
  exit 1
}

setup_help() {
  cat >&2 <<EOF

Get a token at https://wokwi.com/dashboard/ci (free account), then save it
in your own terminal (not in a chat or a committed file):

  mkdir -p ~/.wokwi && printf %s 'YOUR_TOKEN' > ~/.wokwi/token && chmod 600 ~/.wokwi/token

or export WOKWI_CLI_TOKEN for this shell.
EOF
}

# Prints the token found in the file named by $1, or nothing.
read_token_file() {
  # First line that isn't blank or a comment; drop `export `, everything up to
  # the first `=`, surrounding whitespace and quotes.
  sed -n '/^[[:space:]]*\(#\|$\)/d; p; q' "$1" |
    sed 's/^[[:space:]]*export[[:space:]]\{1,\}//; s/^[A-Za-z_][A-Za-z0-9_]*[[:space:]]*=//' |
    sed "s/^[[:space:]]*//; s/[[:space:]]*\$//; s/^[\"']//; s/[\"']\$//" |
    tr -d '\r'
}

# ---------------------------------------------------------------- the token

token=${WOKWI_CLI_TOKEN:-}
source="WOKWI_CLI_TOKEN"
if [ -z "$token" ]; then
  if [ ! -e "$token_file" ]; then
    printf 'run.sh: no Wokwi token: WOKWI_CLI_TOKEN is unset and %s does not exist.\n' "$token_file" >&2
    setup_help
    exit 1
  fi
  [ -r "$token_file" ] || die "cannot read $token_file (check its permissions)."
  token=$(read_token_file "$token_file")
  source=$token_file
  [ -n "$token" ] || { printf 'run.sh: %s has no token in it.\n' "$token_file" >&2; setup_help; exit 1; }

  # Readable by others? Warn, don't refuse.
  perms=$(stat -c %a "$token_file" 2>/dev/null || stat -f %Lp "$token_file" 2>/dev/null || echo 600)
  case $perms in
    *00) ;;
    *) printf 'run.sh: warning: %s is readable by other users (mode %s); run: chmod 600 %s\n' \
         "$token_file" "$perms" "$token_file" >&2 ;;
  esac
fi
case $token in
  *[[:space:]]*) die "the token from $source contains whitespace; check it holds only the token." ;;
  YOUR_TOKEN | *YOUR_TOKEN*) die "the token from $source is still the placeholder text." ;;
esac

# ---------------------------------------------------------------- wokwi-cli

if command -v wokwi-cli >/dev/null 2>&1; then
  cli=wokwi-cli
elif [ -x "$HOME/.wokwi/bin/wokwi-cli" ]; then
  cli=$HOME/.wokwi/bin/wokwi-cli
else
  die "wokwi-cli not found. Install it: https://docs.wokwi.com/wokwi-ci/cli-installation"
fi

# ---------------------------------------------------------------- build

build=false
if [ "${1:-}" = "--build" ]; then
  build=true
  shift
fi
for f in openpour.bin chips/flowmeter.chip.wasm chips/endstop.chip.wasm; do
  [ -e "$here/$f" ] || build=true
done
if $build; then
  "$here/build.sh"
fi

# ---------------------------------------------------------------- run

# wokwi-cli's output is shown as it runs and kept to explain failures.
out=$(mktemp)
status_file=$(mktemp)
trap 'rm -f "$out" "$status_file"' EXIT INT TERM
{
  st=0
  WOKWI_CLI_TOKEN=$token "$cli" "$here" "$@" 2>&1 || st=$?
  echo "$st" >"$status_file"
} | tee "$out"
status=$(cat "$status_file")

scenario=false
for a in "$@"; do
  [ "$a" = "--scenario" ] && scenario=true
done

if grep -qi 'unauthorized\|invalid token\|token.*expired' "$out"; then
  printf '\nrun.sh: Wokwi rejected the token from %s. It may be mistyped, revoked or\n' "$source" >&2
  printf 'expired; check it at https://wokwi.com/dashboard/ci or create a new one.\n' >&2
elif grep -qi 'connection timed out after' "$out"; then
  printf '\nrun.sh: the simulation runs on Wokwi'"'"'s servers, and the plan'"'"'s per-run limit was\n' >&2
  printf 'reached (5 minutes of real time on the free plan; the ESP32 simulates at roughly\n' >&2
  printf 'a quarter of real speed). Shorten the run or see https://wokwi.com/pricing.\n' >&2
  [ "$status" = 0 ] && status=1
elif [ "$status" = 42 ]; then
  if $scenario; then
    printf '\nrun.sh: the scenario did not finish within --timeout.\n' >&2
  else
    printf '\nrun.sh: stopped at the time limit (pass --timeout <ms> for longer).\n' >&2
    status=0
  fi
fi
exit "$status"
