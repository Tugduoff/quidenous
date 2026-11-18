import { fetchExternal } from '@/lib/api/client';


export async function latestRelease(): Promise<Release | null> {
  const res = await fetchExternal(LATEST, { headers: { Accept: 'application/vnd.github+json' } });
  if (!res.ok) throw new Error(`GitHub ${res.status}`);
  const body = (await res.json()) as {
    tag_name: string;
    body?: string | null;
    draft?: boolean;
    prerelease?: boolean;
    assets: { name: string; size: number; browser_download_url: string; digest?: string | null }[];
  };
  const apk = body.assets.find((a) => a.name.endsWith('.apk'));
  // No checksum to verify the download against: not offered.
  const sha256 = /^sha256:([0-9a-f]{64})$/i.exec(apk?.digest ?? '')?.[1]?.toLowerCase();
  if (!apk || !sha256 || body.draft || body.prerelease) return null;
  return { version: body.tag_name.replace(/^v/, ''), notes: body.body ?? '', url: apk.browser_download_url, size: apk.size, sha256 };
}

const LATEST = 'https://api.github.com/repos/Tugduoff/quidenous/releases/latest';

/** `sha256`: the APK's SHA-256 (hex) that GitHub computed when the asset was uploaded. */
export type Release = { version: string; notes: string; url: string; size: number; sha256: string };
