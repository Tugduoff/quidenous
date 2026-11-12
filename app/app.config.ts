import type { ConfigContext, ExpoConfig } from 'expo/config';

/**
 * app.json, plus what depends on the distribution. The APK installed by hand (EXPO_PUBLIC_DISTRIBUTION=sideload)
 * updates itself from GitHub Releases, which needs REQUEST_INSTALL_PACKAGES. The Google Play bundle must not
 * have that permission nor self-update (Play policy): it is built without the flag (see .github/workflows/android.yml).
 */
export default ({ config }: ConfigContext): ExpoConfig => {
  if (process.env.EXPO_PUBLIC_DISTRIBUTION !== 'sideload') return config as ExpoConfig;
  const permissions = [...(config.android?.permissions ?? []), 'android.permission.REQUEST_INSTALL_PACKAGES'];
  return { ...config, android: { ...config.android, permissions } } as ExpoConfig;
};
