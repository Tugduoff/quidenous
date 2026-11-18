import { router } from 'expo-router';
import { ReduceMotion } from 'react-native-reanimated';

/** Text on the bright card colours (members' colours and the accents are all light). */
export const INK = '#121216';
export const sys = ReduceMotion.System;
export const closeWrapped = () => (router.canGoBack() ? router.back() : router.replace('/'));
