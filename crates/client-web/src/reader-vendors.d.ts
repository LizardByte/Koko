/** Describes the small foliate-js interface used by the typed reader adapter. */
declare module 'foliate-js/view.js' {
  export function makeBook(file: File): Promise<unknown>;
}

declare module 'foliate-js/overlayer.js' {
  export class Overlayer {
    static highlight(rects: DOMRect[], options: { color: string }): SVGElement;
  }
}

/** The same byte-weighted section navigation used by Foliate's fraction seek. */
declare module 'foliate-js/progress.js' {
  /** Resolve the section and within-section anchor selected by a whole-book fraction. */
  export class SectionProgress {
    constructor(sections: Array<{ size?: number; linear?: string }>, sizePerLocation: number, sizePerTimeUnit: number);
    sizes: number[];
    getSection(fraction: number): [number, number];
  }
}
