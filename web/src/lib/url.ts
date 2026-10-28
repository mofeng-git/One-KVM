export function formatHostForUrl(hostname: string): string {
  if (!hostname) return '127.0.0.1'
  return hostname.includes(':') && !hostname.startsWith('[') ? `[${hostname}]` : hostname
}
