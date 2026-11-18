import { router } from 'expo-router';
import { ReduceMotion } from 'react-native-reanimated';
import { COLORS } from '@/lib/tokens';

export const INK = COLORS.ink;
export const sys = ReduceMotion.System;
export const closeWrapped = () => (router.canGoBack() ? router.back() : router.replace('/'));
