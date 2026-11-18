#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Backups of deploy/data.

  deploy/backup.sh    nightly: snapshot, copy off-host, prune
  deploy/backup.sh --pre-deploy <tag>    database only, kept locally (deploy.sh, before building <tag>)
  deploy/backup.sh --restore-pre-deploy <tag>    puts that database back (container stopped; deploy.sh rollback)

A snapshot is a directory qd-<UTC date>/ with app.db.gz (a consistent copy: `VACUUM INTO`, safe while the server
runs) and files.tar.gz (auth.key, photos/, chat/ (chat images), updates/, the Firebase JSON). Run as root (the data dir belongs to the
container's uid 10001).

Env (or deploy/backup.env):
  BACKUP_RCLONE_DEST   rclone destination, e.g. `b2:quidenous-backups` (any rclone remote; a `crypt` remote
                       encrypts at rest)
  BACKUP_RSYNC_DEST    or an rsync/ssh destination, e.g. `backup@nas:/srv/backups/quidenous`
  BACKUP_AGE_RECIPIENT optional age public key (age1…): files are encrypted before they leave the host
  BACKUP_KEEP_DAYS     off-host retention in days (default 30)
  BACKUP_KEEP_LOCAL    snapshots kept on this disk (default 3; the same disk is not a backup)
  BACKUP_DIR           local staging dir (default deploy/backups), DATA_DIR (default deploy/data)
  BACKUP_SKIP_UPDATES=1  leave updates/ out (OTA bundles, ~150 MB; CI can publish them again)

Schedule it nightly with an off-host destination (a copy on the same disk is not a backup):
  30 3 * * * cd /srv/quidenous && deploy/backup.sh >> /var/log/quidenous-backup.log 2>&1
Without Docker, set DATA_DIR to the data folder: the snapshot is taken with the host's python3.

A snapshot holds auth.key next to the database: encrypt it (BACKUP_AGE_RECIPIENT or a crypt remote) or keep the
destination private. With AUTH_SECRET set there is no auth.key: keep AUTH_SECRET in a password manager.

Restore drill, on a machine that is not production (once now, then every few months):
  rclone copy <dest>/qd-<date> ./restore && cd restore && sha256sum -c SHA256SUMS
  for f in *.age; do age -d -i <key file> -o "${f%.age}" "$f"; done    (encrypted snapshots only)
  mkdir data && gunzip -c app.db.gz > data/app.db && tar -xzf files.tar.gz -C data
  sudo chown -R 10001:10001 data
  docker run --rm -p 127.0.0.1:8099:8080 -v "$PWD/data:/data" --env-file <deploy/.env> quidenous:latest
Then sign in at http://127.0.0.1:8099 with a real account, 2FA included (that proves the auth key matches), and
check rooms, history and photos. A real restore is the same in deploy/data with the container stopped.
USAGE
}

case "${1:-}" in
  -h | --help)
    usage
    exit 0
    ;;
esac

here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck disable=SC1091
[ -f "$here/backup.env" ] && . "$here/backup.env"
data="${DATA_DIR:-$here/data}"
dir="${BACKUP_DIR:-$here/backups}"
keep_days="${BACKUP_KEEP_DAYS:-30}"
keep_local="${BACKUP_KEEP_LOCAL:-3}"
umask 077
mkdir -p "$dir"

snapshot_db() {
  local out="$1" owner
  if curl -fsS -m 5 "${DEPLOY_URL:-http://127.0.0.1:8080}/api/version" 2>/dev/null | grep -q '"sha"' &&
    (cd "$here" && docker compose exec -T quidenous sh -c \
    'f=/tmp/qd-backup-$$.db; server --backup "$f" >/dev/null && cat "$f"; s=$?; rm -f "$f"; exit $s') >"$out" 2>/dev/null &&
    [ -s "$out" ]; then
    return 0
  fi
  rm -f "$out"
  owner="$(stat -c %u:%g "$data/app.db")"
  python3 -I - "$data/app.db" "$out" <<'PY'
import sqlite3, sys
src, dst = sys.argv[1], sys.argv[2]
db = sqlite3.connect(f"file:{src}?mode=rw", uri=True)
db.execute("VACUUM INTO ?", (dst,))
db.close()
check = sqlite3.connect(dst).execute("PRAGMA quick_check").fetchone()[0]
if check != "ok":
    sys.exit(f"backup check failed: {check}")
PY
  chown "$owner" "$data/app.db-wal" "$data/app.db-shm" 2>/dev/null || true
}

case "${1:-}" in
  --pre-deploy)
    tag="${2:?usage: $0 --pre-deploy <tag>}"
    mkdir -p "$dir/pre-deploy"
    snapshot_db "$dir/pre-deploy/$tag.db.tmp"
    mv "$dir/pre-deploy/$tag.db.tmp" "$dir/pre-deploy/$tag.db"
    ls -1t "$dir/pre-deploy"/*.db | tail -n +4 | xargs -r rm -f
    echo "database saved to $dir/pre-deploy/$tag.db"
    exit 0
    ;;
  --restore-pre-deploy)
    tag="${2:?usage: $0 --restore-pre-deploy <tag>}"
    src="$dir/pre-deploy/$tag.db"
    [ -f "$src" ] || { echo "no $src" >&2; exit 1; }
    owner="$(stat -c %u:%g "$data/app.db" 2>/dev/null || echo 10001:10001)"
    for ext in "" -wal -shm; do
      mv "$data/app.db$ext" "$data/app.db.failed-$tag$ext" 2>/dev/null || true
    done
    cp "$src" "$data/app.db"
    chown "$owner" "$data/app.db"
    echo "database from before $tag restored (the other one kept as app.db.failed-$tag)"
    exit 0
    ;;
  "") ;;
  *)
    sed -n '2,8p' "$0" >&2
    exit 64
    ;;
esac

name="qd-$(date -u +%Y%m%d-%H%M%S)"
snap="$dir/$name.partial"
mkdir "$snap"
trap 'rm -rf "$snap"' EXIT
snapshot_db "$snap/app.db"
gzip -9 "$snap/app.db"
files=()
for f in auth.key photos chat updates; do
  if [ "$f" = updates ] && [ "${BACKUP_SKIP_UPDATES:-}" = 1 ]; then continue; fi
  if [ -e "$data/$f" ]; then files+=("$f"); fi
done
while IFS= read -r f; do files+=("$f"); done < <(cd "$data" && find . -maxdepth 1 -name '*.json' -printf '%f\n')
tar -czf "$snap/files.tar.gz" -C "$data" ${files[@]+"${files[@]}"}
if [ -n "${BACKUP_AGE_RECIPIENT:-}" ]; then
  for f in "$snap"/*.gz; do
    age -r "$BACKUP_AGE_RECIPIENT" -o "$f.age" "$f" && rm "$f"
  done
fi
(cd "$snap" && sha256sum -- * >SHA256SUMS)
mv "$snap" "$dir/$name"
trap - EXIT
snap="$dir/$name"
echo "snapshot $snap ($(du -sh "$snap" | cut -f1))"

if [ -n "${BACKUP_RCLONE_DEST:-}" ]; then
  rclone copy "$snap" "$BACKUP_RCLONE_DEST/$name"
  rclone delete --min-age "${keep_days}d" "$BACKUP_RCLONE_DEST"
  rclone rmdirs --leave-root "$BACKUP_RCLONE_DEST"
  echo "copied to $BACKUP_RCLONE_DEST/$name"
elif [ -n "${BACKUP_RSYNC_DEST:-}" ]; then
  rsync -a "$snap" "$BACKUP_RSYNC_DEST/"
  host="${BACKUP_RSYNC_DEST%%:*}"
  path="${BACKUP_RSYNC_DEST#*:}"
  # shellcheck disable=SC2029
  ssh "$host" "find '$path' -mindepth 1 -maxdepth 1 -name 'qd-*' -mtime +$keep_days -exec rm -rf {} +"
  echo "copied to $BACKUP_RSYNC_DEST/$name"
else
  echo "WARNING: BACKUP_RCLONE_DEST / BACKUP_RSYNC_DEST not set: this snapshot is on the same disk only" >&2
fi

ls -1d "$dir"/qd-*[0-9] | sort | head -n -"$keep_local" | xargs -r rm -rf
