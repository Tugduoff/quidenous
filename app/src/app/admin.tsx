import { AdminReview } from '@/components/admin/admin-review';
import { useCommunityMe, useReview } from '@/lib/api/packs';
import { useFeatures } from '@/lib/api/features';

/** Admins: reported messages, members, questions and suggestions; packs sent to review by reports (approve or remove with a reason), suggestions voted up (approve or reject). */
export default function AdminScreen() {
  const { data: me } = useCommunityMe();
  const features = useFeatures();
  const { data: review } = useReview(!!me?.admin);
  return <AdminReview admin={me?.admin} showWrappedPreview={!!me?.admin && features.wrapped} review={review} />;
}
