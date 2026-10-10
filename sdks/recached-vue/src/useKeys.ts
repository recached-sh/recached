import { ref, onUnmounted, type Ref } from 'vue';
import { useRecached } from './plugin';
import { subscribePattern } from './subscribe';

/** A key/value pair from the local store. Collection-typed keys have `null`
 * values — read those with typed accessors. A value that is not valid UTF-8
 * arrives as a `Uint8Array` rather than a mangled string. */
export type KeyValuePair = [key: string, value: string | Uint8Array | null];

/**
 * Live query: reactively read every key matching a glob pattern.
 *
 * On setup, the server sends the current state of all matching keys (merged
 * into the local store) and then streams every change to matching keys —
 * including keys created after subscribing. The returned `Ref` updates on
 * each change, always reading from local WASM memory.
 *
 * Pairs are sorted by key. The server subscription is ref-counted, so any
 * number of components can share a pattern; it ends when the last one
 * unmounts.
 *
 * Under strict sync scoping the pattern must sit inside the connection's
 * granted scopes.
 *
 * ```vue
 * <script setup lang="ts">
 * import { useKeys } from '@recached/vue'
 *
 * const items = useKeys('cart:42:item:*')
 * </script>
 *
 * <template>
 *   <ul>
 *     <li v-for="[key, qty] in items" :key="key">
 *       {{ key.split(':').pop() }}: {{ qty }}
 *     </li>
 *   </ul>
 * </template>
 * ```
 */
export function useKeys(pattern: string): Ref<KeyValuePair[]> {
  const cache = useRecached();
  const value = ref<KeyValuePair[]>(cache.getMatching(pattern));
  const stopQuery = cache.liveQuery(pattern);
  const unsub = subscribePattern(cache, pattern, () => {
    value.value = cache.getMatching(pattern);
  });
  onUnmounted(() => {
    unsub();
    stopQuery();
  });
  return value as Ref<KeyValuePair[]>;
}
