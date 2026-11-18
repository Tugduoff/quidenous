#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Reference deploy poller for the homelab: deploys the newest v* tag once CI is green on it.

  deploy/deploy.sh    run from cron / a systemd timer every few minutes (one run at a time: flock)
  deploy/deploy.sh v0.31.0    deploy that tag now (still gated on CI)

Steps: newest v* tag newer than the one deployed -> CI green on its commit (wait-green.sh) -> database snapshot
(backup.sh --pre-deploy) -> keep the running image as quidenous:prev -> build the tag -> start -> check
/api/health and that /api/version reports the tag's commit. If the new version doesn't come up healthy: back to
quidenous:prev with the pre-deploy database (migrations may have changed it; an older binary refuses a newer
schema), and exit 1.

Env (or deploy/deploy.env): DEPLOY_URL (default http://127.0.0.1:8080), DEPLOY_WEBHOOK (optional Discord webhook
for deploy and rollback notices), plus what wait-green.sh and backup.sh read.

Schedule it (root, or a user in the docker group that can read deploy/data):
  */5 * * * * cd /srv/quidenous && deploy/deploy.sh >> /var/log/quidenous-deploy.log 2>&1
The deployed tag is recorded in deploy/.deployed; a red CI is reported once per tag (deploy/.ci-red).

Roll back by hand:
  docker compose stop quidenous && deploy/backup.sh --restore-pre-deploy <tag>
  docker tag quidenous:prev quidenous:latest && docker compose up -d --no-build
The failed database is kept as data/app.db.failed-<tag>.
USAGE
}

case "${1:-}" in
  -h | --help)
    usage
    exit 0
    ;;
esac

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"
# shellcheck disable=SC1091
[ -f "$here/deploy.env" ] && . "$here/deploy.env"
url="${DEPLOY_URL:-http://127.0.0.1:8080}"
state="$here/.deployed"

exec 9>"$here/.deploy.lock"
flock -n 9 || { echo "another deploy is running"; exit 0; }

notify() {
  echo "$1"
  if [ -n "${DEPLOY_WEBHOOK:-}" ]; then
    python3 -I -c 'import json,sys; print(json.dumps({"content": sys.argv[1], "allowed_mentions": {"parse": []}}))' "$1" |
      curl -fsS -m 10 -H 'Content-Type: application/json' -d @- "$DEPLOY_WEBHOOK" >/dev/null || true
  fi
}

cd "$repo"
git fetch -q --tags origin
tag="${1:-$(git tag -l 'v*' --sort=-v:refname | head -n1)}"
[ -n "$tag" ] || { echo "no v* tag"; exit 0; }
current="$(cat "$state" 2>/dev/null || true)"
if [ -z "${1:-}" ] && [ -n "$current" ]; then
  newest="$(printf '%s\n%s\n' "$current" "$tag" | sort -V | tail -n1)"
  if [ "$tag" = "$current" ] || [ "$newest" = "$current" ]; then exit 0; fi
fi
sha="$(git rev-list -n1 "$tag")"

gate=0
"$here/wait-green.sh" "$sha" || gate=$?
if [ "$gate" -ne 0 ]; then
  if [ "$gate" -eq 1 ] && [ "$(cat "$here/.ci-red" 2>/dev/null || true)" != "$tag" ]; then
    echo "$tag" >"$here/.ci-red"
    notify "Quidenous $tag not deployed: CI failed on $sha (re-run the failed job or push a new tag)"
  fi
  exit 0
fi

echo "deploying $tag ($sha)"
git -c advice.detachedHead=false checkout -q --detach "$tag"
"$here/backup.sh" --pre-deploy "$tag"
cd "$here"
if docker image inspect quidenous:latest >/dev/null 2>&1; then
  docker image tag quidenous:latest quidenous:prev
fi
QD_VERSION="${tag#v}" QD_GIT_SHA="$sha" docker compose build
docker compose up -d

healthy() {
  for _ in $(seq 1 30); do
    if curl -fsS -m 3 "$url/api/health" >/dev/null 2>&1 &&
      curl -fsS -m 3 "$url/api/version" 2>/dev/null | grep -q "\"sha\":\"$1\""; then
      return 0
    fi
    sleep 4
  done
  return 1
}

if healthy "$sha"; then
  echo "$tag" >"$state"
  notify "Quidenous $tag deployed ($sha)"
  exit 0
fi

docker compose logs --tail 50 quidenous || true
if ! docker image inspect quidenous:prev >/dev/null 2>&1; then
  notify "Quidenous $tag is unhealthy after deploy and there is no previous image to go back to"
  exit 1
fi
docker compose stop quidenous
"$here/backup.sh" --restore-pre-deploy "$tag"
docker image tag quidenous:prev quidenous:latest
docker compose up -d --no-build
prev_sha="$(docker image inspect quidenous:prev --format '{{range .Config.Env}}{{println .}}{{end}}' | sed -n 's/^QUIDENOUS_GIT_SHA=//p')"
if [ -n "$prev_sha" ] && [ "$prev_sha" != "dev" ] && healthy "$prev_sha"; then
  notify "Quidenous $tag failed its health check: rolled back to ${current:-the previous image} (database from before the deploy)"
else
  notify "Quidenous $tag failed its health check, rollback started: check it by hand (docker compose ps / logs)"
fi
exit 1
