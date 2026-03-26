// Virtual keyboard layout data shared by the on-screen keyboard.

export type KeyboardOsType = 'windows' | 'mac' | 'android'

export const osBottomRows: Record<KeyboardOsType, string[]> = {
  windows: ['ControlLeft', 'MetaLeft', 'AltLeft', 'Space', 'AltRight', 'MetaRight', 'ContextMenu', 'ControlRight'],
  mac: ['ControlLeft', 'AltLeft', 'MetaLeft', 'Space', 'MetaRight', 'AltRight', 'ControlRight'],
  android: ['ControlLeft', 'AltLeft', 'Space', 'AltRight', 'ControlRight'],
}

export const mediaKeys = ['PrevTrack', 'PlayPause', 'NextTrack', 'Stop', 'Mute', 'VolumeDown', 'VolumeUp']

export const mediaKeyLabels: Record<string, string> = {
  PlayPause: '⏯',
  Stop: '⏹',
  NextTrack: '⏭',
  PrevTrack: '⏮',
  Mute: '🔇',
  VolumeUp: '🔊',
  VolumeDown: '🔉',
}
