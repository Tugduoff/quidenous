#!/usr/bin/env bash
# Deploy gate: is CI green on this commit?
#
#   deploy/wait-green.sh <sha> [--wait <seconds>]
#
# Exit 0 when the latest run of every required check on <sha> succeeded, 1 when one failed (or was cancelled),
# 2 when one is still running or missing (after --wait seconds of polling, default 0: answer right away).
# Required checks: $REQUIRED_CHECKS (space-separated job names), default "check app apk": the server tests
# (server.yml), the app type check (app.yml) and the Android build (android.yml). Several runs of a check on the same
# commit (main push + tag push, a re-run): the most recent one counts, so a red tag run blocks even if main was green.
#
# Env: GITHUB_REPOSITORY (default Tugduoff/quidenous), GITHUB_TOKEN (optional, public repo: raises the API rate
# limit from 60 to 5000 requests an hour). Needs curl and python3.
set -euo pipefail

repo="${GITHUB_REPOSITORY:-Tugduoff/quidenous}"
required="${REQUIRED_CHECKS:-check app apk}"
sha="${1:-}"
wait=0
if [ "${2:-}" = "--wait" ]; then wait="${3:-0}"; fi
if ! [[ "$sha" =~ ^[0-9a-f]{7,40}$ ]]; then
  echo "usage: $0 <commit sha> [--wait <seconds>]" >&2
  exit 64
fi

# One line per required check: "<name> <status> <conclusion>" of its most recent run.
status() {
  local auth=()
  if [ -n "${GITHUB_TOKEN:-}" ]; then auth=(-H "Authorization: Bearer $GITHUB_TOKEN"); fi
  curl -fsSL ${auth[@]+"${auth[@]}"} -H "Accept: application/vnd.github+json" \
    "https://api.github.com/repos/$repo/commits/$sha/check-runs?per_page=100" |
    REQUIRED="$required" python3 -I -c '
import json, os, sys
want = os.environ["REQUIRED"].split()
latest = {}
for run in json.load(sys.stdin)["check_runs"]:
    name = run["name"]
    if name in want and (run.get("started_at") or "") >= (latest.get(name, {}).get("started_at") or ""):
        latest[name] = run
for name in want:
    run = latest.get(name)
    print(name, run["status"], run.get("conclusion") or "-") if run else print(name, "missing", "-")
'
}

deadline=$(($(date +%s) + wait))
while :; do
  if ! lines=$(status); then
    echo "GitHub API unreachable" >&2
    result=2
  else
    result=0
    while read -r name state conclusion; do
      case "$state/$conclusion" in
        completed/success) ;;
        completed/*)
          # Failed, cancelled, timed out, or skipped: a skipped required job is not a green light either.
          echo "$name: $conclusion" >&2
          result=1
          ;;
        *)
          echo "$name: $state" >&2
          if [ "$result" -eq 0 ]; then result=2; fi
          ;;
      esac
    done <<<"$lines"
  fi
  if [ "$result" -ne 2 ] || [ "$(date +%s)" -ge "$deadline" ]; then
    break
  fi
  sleep 30
done
case "$result" in
  0) echo "CI green on $sha ($required)" ;;
  1) echo "CI failed on $sha: not deploying" >&2 ;;
  2) echo "CI not finished on $sha: try again later" >&2 ;;
esac
exit "$result"
