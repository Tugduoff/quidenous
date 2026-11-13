import { type BarcodeScanningResult, CameraView, useCameraPermissions } from 'expo-camera';
import { ScanLine, X } from 'lucide-react-native';
import { useRef, useState } from 'react';
import { Modal, Pressable, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export type Scanned = { code?: string; friend?: string };

/** What a qui de nous ? QR code holds: an invite link `/?code=…` or a friend link `/?friend=…`. */
function read(data: string): Scanned | null {
  try {
    const params = new URL(data).searchParams;
    const code = params.get('code') ?? undefined;
    const friend = params.get('friend') ?? undefined;
    return code || friend ? { code, friend } : null;
  } catch {
    return null;
  }
}

/** Round button next to a code field: opens the camera and hands back the code of the QR. */
export function ScanButton({ onScan, label }: { onScan: (s: Scanned) => void; label?: string }) {
  const t = useTheme();
  const tr = useT();
  const [open, setOpen] = useState(false);
  return (
    <>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={label ?? tr('auth.qr.scan')}
        onPress={() => setOpen(true)}
        style={{ width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
        <ScanLine size={20} color={t.txt} />
      </Pressable>
      {open && (
        <QrScanner
          onClose={() => setOpen(false)}
          onScan={(s) => {
            setOpen(false);
            onScan(s);
          }}
        />
      )}
    </>
  );
}

function QrScanner({ onScan, onClose }: { onScan: (s: Scanned) => void; onClose: () => void }) {
  const t = useTheme();
  const tr = useT();
  const [permission, requestPermission] = useCameraPermissions();
  const [wrong, setWrong] = useState(false);
  // The camera reports the same code many times a second: only the first one counts.
  const done = useRef(false);
  const onBarcode = ({ data }: BarcodeScanningResult) => {
    if (done.current) return;
    const scanned = read(data);
    if (!scanned) return setWrong(true);
    done.current = true;
    onScan(scanned);
  };
  const text = { fontFamily: t.font.semibold, fontSize: 15, color: '#fff', textAlign: 'center' as const };
  return (
    <Modal visible animationType="slide" onRequestClose={onClose}>
      <View style={{ flex: 1, backgroundColor: '#000' }}>
        {permission?.granted && (
          <CameraView style={{ flex: 1 }} facing="back" barcodeScannerSettings={{ barcodeTypes: ['qr'] }} onBarcodeScanned={onBarcode} />
        )}
        <SafeAreaView style={{ position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, padding: 20, justifyContent: 'space-between' }}>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={tr('common.close')}
            onPress={onClose}
            style={{ alignSelf: 'flex-end', width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: '#0008' }}>
            <X size={22} color="#fff" />
          </Pressable>
          {permission && !permission.granted ? (
            <View style={{ gap: 14, alignItems: 'center' }}>
              {!permission.canAskAgain && <Text style={text}>{tr('auth.qr.blocked')}</Text>}
              {permission.canAskAgain && (
                <Pressable
                  accessibilityRole="button"
                  onPress={requestPermission}
                  style={{ height: 46, paddingHorizontal: 22, borderRadius: t.radius.pill, alignItems: 'center', justifyContent: 'center', backgroundColor: t.acc }}>
                  <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.accTxt }}>{tr('auth.qr.allow')}</Text>
                </Pressable>
              )}
            </View>
          ) : (
            <View style={{ alignSelf: 'center', paddingHorizontal: 16, paddingVertical: 10, borderRadius: t.radius.pill, backgroundColor: '#0008' }}>
              <Text style={text}>{wrong ? tr('auth.qr.wrong') : tr('auth.qr.aim')}</Text>
            </View>
          )}
        </SafeAreaView>
      </View>
    </Modal>
  );
}
