/** Shared user-facing errors. Raw HTTP bodies belong in diagnostics, never in page alerts. */
export function apiErrorMessage(status:number,body:string):string {
  const known:Record<number,string>={
    401:'Your session has expired. Sign in again.',403:'You do not have permission to do this.',
    404:'This item is unavailable or you no longer have access to it.',
    408:'The request took too long. Please try again.',409:'This conflicts with another change. Refresh and try again.',
    413:'This request is too large. Try selecting fewer items.',429:'Too many requests. Wait a moment and try again.',
    500:'The server could not complete this request. Please try again.',
    502:'An external service is temporarily unavailable. Please try again.',
    503:'The service is temporarily unavailable. Please try again shortly.',
    504:'The service took too long to respond. Please try again.',
  };
  if(known[status])return known[status];
  let detail=body.trim();
  try {
    const value=JSON.parse(detail);
    detail=typeof value==='string' ? value : value.message ?? value.detail ?? value.error?.message ?? value.error;
  } catch { /* Plain-text validation errors are also useful. */ }
  // Keep concise validation explanations, while rejecting HTML, JSON, and stack traces.
  if([400,422].includes(status) && typeof detail==='string' && detail.length<=240 && !/[<>\n{}]|\bat\s+\w+\(/.test(detail) && detail.trim())return detail;
  if([400,422].includes(status))return 'Some information is missing or invalid. Check your entries and try again.';
  return 'The request could not be completed. Please try again.';
}
