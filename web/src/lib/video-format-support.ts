export type VideoFormatSupportContext = 'config' | 'mjpeg' | 'h264' | 'h265' | 'vp8' | 'vp9'

export type VideoFormatState = 'supported' | 'not_recommended' | 'unsupported'

const MJPEG_MODE_SUPPORTED_FORMATS = new Set([
  'MJPEG',
  'JPEG',
  'YUYV',
  'YVYU',
  'NV12',
  'RGB24',
  'BGR24',
])

const CONFIG_SUPPORTED_FORMATS = new Set([
  'MJPEG',
  'JPEG',
  'YUYV',
  'YVYU',
  'NV12',
  'NV21',
  'NV16',
  'YUV420',
  'RGB24',
  'BGR24',
])

const WEBRTC_SUPPORTED_FORMATS = new Set([
  'MJPEG',
  'JPEG',
  'YUYV',
  'NV12',
  'NV21',
  'NV16',
  'YUV420',
  'RGB24',
  'BGR24',
])

function normalizeFormat(formatName: string): string {
  return formatName.trim().toUpperCase()
}

function isCompressedFormat(formatName: string): boolean {
  return formatName === 'MJPEG' || formatName === 'JPEG'
}

function isRkmppBackend(backendId?: string): boolean {
  return backendId?.toLowerCase() === 'rkmpp'
}

export function getVideoFormatState(
  formatName: string,
  context: VideoFormatSupportContext,
  encoderBackend = 'auto',
): VideoFormatState {
  const normalizedFormat = normalizeFormat(formatName)

  if (context === 'mjpeg') {
    return MJPEG_MODE_SUPPORTED_FORMATS.has(normalizedFormat) ? 'supported' : 'unsupported'
  }

  if (context === 'config') {
    if (CONFIG_SUPPORTED_FORMATS.has(normalizedFormat)) {
      return 'supported'
    }
    if (
      normalizedFormat === 'NV24'
      && isRkmppBackend(encoderBackend)
    ) {
      return 'supported'
    }
    return 'unsupported'
  }

  if (WEBRTC_SUPPORTED_FORMATS.has(normalizedFormat)) {
    return isCompressedFormat(normalizedFormat) ? 'not_recommended' : 'supported'
  }

  if (
    normalizedFormat === 'NV24'
    && isRkmppBackend(encoderBackend)
    && (context === 'h264' || context === 'h265')
  ) {
    return 'supported'
  }

  return 'unsupported'
}

export function isVideoFormatSelectable(
  formatName: string,
  context: VideoFormatSupportContext,
  encoderBackend = 'auto',
): boolean {
  return getVideoFormatState(formatName, context, encoderBackend) !== 'unsupported'
}
