/** Keep older polls and requests made during a metadata mutation from publishing stale data. */
let generation = 0;
let mutationsInFlight = 0;
const polls = { metadata: 0, library: 0 };

export function invalidateMetadataRequests(): void {
  generation += 1;
}

export function beginMetadataRequest(kind: keyof typeof polls = 'metadata'): () => boolean {
  const requestGeneration = generation;
  const poll = ++polls[kind];
  return () => mutationsInFlight === 0 && requestGeneration === generation && poll === polls[kind];
}

/** Invalidate both pre-mutation polls and polls that overlap the server update. */
export async function mutateMetadata<T>(action: () => Promise<T>): Promise<T> {
  mutationsInFlight += 1;
  invalidateMetadataRequests();
  try {
    return await action();
  } finally {
    mutationsInFlight -= 1;
    invalidateMetadataRequests();
  }
}
