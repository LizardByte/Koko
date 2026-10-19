import type { MediaItemSummary } from '../api';

export type RepeatMode = 'off' | 'all' | 'one';
export interface QueueEntry { key: number; item: MediaItemSummary }

/** Entry identities preserve duplicates, played history, and explicit Play next placement. */
export class MusicQueue {
  entries: QueueEntry[] = [];
  order: number[] = [];
  cursor = 0;
  shuffle = false;
  repeat: RepeatMode = 'off';
  private sequence = 0;
  constructor(private random = Math.random) {}
  get current(): QueueEntry | undefined { return this.entries.find((entry) => entry.key === this.order[this.cursor]); }
  get ordered(): QueueEntry[] { const entries = new Map(this.entries.map((entry) => [entry.key, entry])); return this.order.flatMap((key) => entries.get(key) ?? []); }
  replace(items: MediaItemSummary[], start = 0): void {
    this.entries = items.slice(0, 5000).map((item) => ({ key: ++this.sequence, item }));
    this.order = this.entries.map((entry) => entry.key);
    this.cursor = Math.max(0, Math.min(start, this.order.length - 1));
    if (this.shuffle) this.shuffleUpcoming();
  }
  enqueue(items: MediaItemSummary[], next = false): void {
    const added = items.slice(0, 5000 - this.entries.length).map((item) => ({ key: ++this.sequence, item }));
    const insert = next ? this.entries.findIndex((entry) => entry.key === this.current?.key) + 1 : this.entries.length;
    this.entries.splice(insert, 0, ...added);
    this.order.splice(next ? this.cursor + 1 : this.order.length, 0, ...added.map((entry) => entry.key));
  }
  toggleShuffle(): void {
    this.shuffle = !this.shuffle;
    if (this.shuffle) this.shuffleUpcoming();
    else {
      const played = new Set(this.order.slice(0, this.cursor + 1));
      this.order = [...this.order.slice(0, this.cursor + 1), ...this.entries.filter((entry) => !played.has(entry.key)).map((entry) => entry.key)];
    }
  }
  private shuffleUpcoming(): void {
    for (let index = this.order.length - 1; index > this.cursor + 1; index--) {
      const other = this.cursor + 1 + Math.floor(this.random() * (index - this.cursor));
      [this.order[index], this.order[other]] = [this.order[other], this.order[index]];
    }
  }
  advance(ended = false): boolean {
    if (!this.current) return false;
    if (ended && this.repeat === 'one') return true;
    if (this.cursor + 1 < this.order.length) { this.cursor++; return true; }
    if (this.repeat !== 'all') return false;
    this.cursor = 0;
    if (this.shuffle) this.shuffleUpcoming();
    return true;
  }
  previous(): boolean {
    if (this.cursor > 0) { this.cursor--; return true; }
    if (this.repeat === 'all' && this.order.length > 1) { this.cursor = this.order.length - 1; return true; }
    return false;
  }
  select(key: number): boolean { const index = this.order.indexOf(key); if (index < 0) return false; this.cursor = index; return true; }
  remove(key: number): void {
    const index = this.order.indexOf(key);
    if (index < 0 || key === this.current?.key) return;
    this.order.splice(index, 1);
    this.entries = this.entries.filter((entry) => entry.key !== key);
    if (index < this.cursor) this.cursor--;
  }
  move(key: number, direction: -1 | 1): void {
    const index = this.order.indexOf(key), target = index + direction;
    if (index <= this.cursor || target <= this.cursor || target >= this.order.length) return;
    [this.order[index], this.order[target]] = [this.order[target], this.order[index]];
  }
  clearUpcoming(): void {
    const keep = new Set(this.order.slice(0, this.cursor + 1));
    this.order = this.order.filter((key) => keep.has(key));
    this.entries = this.entries.filter((entry) => keep.has(entry.key));
  }
}
