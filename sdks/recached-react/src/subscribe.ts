import type { Cache } from 'recached-edge';

// The peer range admits recached-edge releases from before per-key change
// routing. On those, every mutation is a possible change to every key, so the
// hooks fall back to listening to all of them: slower, never stale.

/** Listen for changes to one key. */
export function subscribeKey(cache: Cache, key: string, cb: () => void): () => void {
  return typeof cache.onKeyChange === 'function'
    ? cache.onKeyChange(key, cb)
    : cache.onMutation(cb);
}

/** Listen for changes to any key matching a glob pattern. */
export function subscribePattern(cache: Cache, pattern: string, cb: () => void): () => void {
  return typeof cache.onPatternChange === 'function'
    ? cache.onPatternChange(pattern, cb)
    : cache.onMutation(cb);
}
