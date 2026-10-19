/** The server owns HTTP error wording; this fallback handles proxies and older servers. */
export function apiErrorMessage(status: number, body: string): string {
  try {
    const error: unknown = JSON.parse(body);
    if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string' && error.message.trim()) {
      return error.message;
    }
  } catch { /* A proxy may return an HTML error page. Never display its body. */ }
  return `The server could not complete this request (HTTP ${status}). Please try again.`;
}
