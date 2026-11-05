
export type KeyboardOsType = 'windows' | 'linux' | 'mac' | 'android'

export const osBottomRows: Record<KeyboardOsType, string[]> = {
  windows: ['ControlLeft', 'MetaLeft', 'AltLeft', 'Space', 'AltRight', 'MetaRight', 'ContextMenu', 'ControlRight'],
  linux: ['ControlLeft', 'MetaLeft', 'AltLeft', 'Space', 'AltRight', 'MetaRight', 'ContextMenu', 'ControlRight'],
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
