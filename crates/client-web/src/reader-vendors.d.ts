/** Describes the small foliate-js interface used by the typed reader adapter. */
declare module 'foliate-js/view.js' {
  export function makeBook(file: File): Promise<unknown>;
}
