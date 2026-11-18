#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Outside uptime check: GET <url>/api/health; posts to a Discord webhook after 2 failed checks in a row, and once
when it answers again. Run it every 5 minutes from a machine OUTSIDE the server's network, so a dead host or a
cut connection is noticed too.

  UPTIME_URL=https://your.domain DISCORD_WEBHOOK=https://discord.com/api/webhooks/... deploy/uptime-check.sh

UPTIME_URL       the server's public URL (required)
DISCORD_WEBHOOK  Discord webhook URL (required)
UPTIME_STATE     file holding the count of failures in a row (default: ~/.local/state/quidenous-uptime)
UPTIME_TIMEOUT   seconds per check (default: 10)
UPTIME_FAILURES  failures in a row before alerting (default: 2)

Install it with the units next to this script (quidenous-uptime.service reads UPTIME_URL and DISCORD_WEBHOOK
from /etc/quidenous-uptime.env):
  sudo install -m 755 deploy/uptime-check.sh /opt/quidenous/uptime-check.sh
  sudo install -m 644 deploy/quidenous-uptime.service deploy/quidenous-uptime.timer /etc/systemd/system/
  sudo systemctl daemon-reload && sudo systemctl enable --now quidenous-uptime.timer
USAGE
}

case "${1:-}" in
  -h | --help)
    usage
    exit 0
    ;;
esac

url="${UPTIME_URL:?set UPTIME_URL (https://your.domain)}"
url="${url%/}"
webhook="${DISCORD_WEBHOOK:?set DISCORD_WEBHOOK}"
state="${UPTIME_STATE:-${XDG_STATE_HOME:-$HOME/.local/state}/quidenous-uptime}"
timeout="${UPTIME_TIMEOUT:-10}"
threshold="${UPTIME_FAILURES:-2}"

notify() {
  local text body
  text="$(printf '%s' "$1" | tr -d '\000-\037' | sed 's/\\/\\\\/g; s/"/\\"/g')"
  body="{\"content\":\"$text\",\"allowed_mentions\":{\"parse\":[]}}"
  curl -fsS -m 10 -H 'content-type: application/json' -d "$body" "$webhook" >/dev/null ||
    echo "uptime-check: the Discord webhook failed" >&2
}

mkdir -p "$(dirname "$state")"
failures=0
if [ -f "$state" ]; then
  failures="$(cat "$state")"
  case "$failures" in '' | *[!0-9]*) failures=0 ;; esac
fi

code=0
answer="$(curl -sS -m "$timeout" -w '\n%{http_code}' "$url/api/health" 2>&1)" || code=$?
status="$(printf '%s' "$answer" | tail -n1)"
body="$(printf '%s' "$answer" | sed '$d' | head -c 200)"

if [ "$code" -eq 0 ] && [ "$status" = "200" ] && [ "$body" = "ok" ]; then
  if [ "$failures" -ge "$threshold" ]; then
    notify "Quidenous is back up ($url)"
  fi
  echo 0 >"$state"
  exit 0
fi

failures=$((failures + 1))
echo "$failures" >"$state"
if [ "$code" -ne 0 ]; then
  why="no answer (curl exit $code)"
else
  why="HTTP $status ${body//$'\n'/ }"
fi
echo "uptime-check: $url/api/health failed ($failures in a row): $why" >&2
if [ "$failures" -eq "$threshold" ]; then
  notify "Quidenous is DOWN ($url/api/health: $why, $failures checks in a row)"
fi
exit 1
