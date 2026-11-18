import type { PackTag } from '@/lib/api/packs';
import { type LucideIcon, Beer, Flame, Briefcase, House, PartyPopper, BookOpen, Sparkles } from 'lucide-react-native';

export const TAG_ICONS: Record<PackTag, LucideIcon> = {
  potes: Beer,
  pimente: Flame,
  boulot: Briefcase,
  famille: House,
  soiree: PartyPopper,
  culture: BookOpen,
  absurde: Sparkles,
};
