import type { ConfigContext, ExpoConfig } from 'expo/config';

export default ({ config }: ConfigContext): ExpoConfig => {
  if (process.env.EXPO_PUBLIC_DISTRIBUTION !== 'sideload') return config as ExpoConfig;
  const permissions = [...(config.android?.permissions ?? []), 'android.permission.REQUEST_INSTALL_PACKAGES'];
  return { ...config, android: { ...config.android, permissions } } as ExpoConfig;
};
