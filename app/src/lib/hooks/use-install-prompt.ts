import { useEffect, useState } from 'react';
import { forgetInstallPrompt, installPrompt, onInstallPrompt } from '@/lib/device/install-prompt';

export function useInstallPrompt() {
  const [prompt, setPrompt] = useState(installPrompt);
  const [installed, setInstalled] = useState(false);
  useEffect(() => {
    const done = () => setInstalled(true);
    const off = onInstallPrompt(() => setPrompt(installPrompt()));
    window.addEventListener('appinstalled', done);
    return () => {
      off();
      window.removeEventListener('appinstalled', done);
    };
  }, []);
  const install = async () => {
    if (!prompt) return;
    await prompt.prompt();
    if ((await prompt.userChoice).outcome === 'accepted') setInstalled(true);
    forgetInstallPrompt();
    setPrompt(null);
  };
  return { canPrompt: !!prompt, installed, install };
}
