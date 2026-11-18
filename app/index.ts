// The app's entry: Expo Router, plus the home-screen widget's background tasks (src/widget/task.tsx, docs/WIDGET.md).
// They must be registered here, at load: Android starts them with no screen (the app closed, the launcher asking).
import 'expo-router/entry';

import { registerWidget } from '@/widget/task';

registerWidget();
