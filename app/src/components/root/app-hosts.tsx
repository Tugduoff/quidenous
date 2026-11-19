import { TermsGate } from '@/components/auth/terms';
import { CelebrationPreview } from '@/components/celebration';
import { AppLock } from '@/components/security/app-lock';
import { ConfirmHost } from '@/components/ui/confirm';
import { ToastHost } from '@/components/ui/toast';
import { OtaToast, UpdateDialog } from '@/components/update/update';
import { UpdateRequired } from '@/components/update/update-required';
import type { Stage } from '@/lib/hooks/use-stage-route';

export function AppHosts({ stage, signedIn }: { stage: Stage; signedIn: boolean }) {
  return (
    <>
      <CelebrationPreview />
      <AppLock active={signedIn} />
      {(stage === 'in' || stage === 'room' || stage === 'intro') && <TermsGate />}
      <ConfirmHost />
      <UpdateDialog />
      {stage === 'in' && <OtaToast />}
      <UpdateRequired />
      <ToastHost />
    </>
  );
}
