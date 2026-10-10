import { ref, onUnmounted, type Ref } from 'vue';
import { useRecached } from './plugin';
import { subscribeKey } from './subscribe';

/**
 * Reactively read a string key from the Recached store.
 *
 * The returned `Ref` updates automatically whenever the key is written or
 * deleted — whether the mutation originated locally, from another tab via
 * BroadcastChannel, or from another client via the server WebSocket.
 *
 * Returns `null` when the key does not exist or has expired.
 * Use `cache.set()` to write; the ref itself is not intended as a writable target.
 *
 * ```vue
 * <script setup lang="ts">
 * import { useKey, useRecached } from '@recached/vue'
 *
 * const theme = useKey('theme')
 * const cache = useRecached()
 * function toggle() {
 *   cache.set('theme', theme.value === 'dark' ? 'light' : 'dark')
 * }
 * </script>
 *
 * <template>
 *   <button @click="toggle">{{ theme ?? 'light' }}</button>
 * </template>
 * ```
 */
export function useKey(key: string): Ref<string | null> {
  const cache = useRecached();
  const value = ref<string | null>(null);
  // `get` throws on a binary value, which has no string form. Report it as
  // absent rather than propagating out of a reactive update; read those with
  // {@link useKeyBytes}.
  const read = (): string | null => {
    try {
      return cache.get(key);
    } catch {
      return null;
    }
  };
  const unsub = subscribeKey(cache, key, () => {
    value.value = read();
  });
  value.value = read();
  onUnmounted(unsub);
  return value;
}

/**
 * Reactively read a key as raw bytes.
 *
 * Behaves identically to {@link useKey} but returns the value's bytes, so it
 * works for binary values a backend wrote. Text values come back as their
 * UTF-8 bytes.
 */
export function useKeyBytes(key: string): Ref<Uint8Array | null> {
  const cache = useRecached();
  const value = ref<Uint8Array | null>(null);
  const unsub = subscribeKey(cache, key, () => {
    value.value = cache.getBytes(key);
  });
  value.value = cache.getBytes(key);
  onUnmounted(unsub);
  return value as Ref<Uint8Array | null>;
}

/**
 * Reactively read a JSON-parsed value from the Recached store.
 *
 * Behaves identically to {@link useKey} but parses the stored string as JSON.
 * Returns `null` when the key is missing, expired, or contains invalid JSON.
 *
 * ```vue
 * <script setup lang="ts">
 * import { useKeyJSON } from '@recached/vue'
 *
 * interface User { id: number; name: string }
 * const user = useKeyJSON<User>('user:42')
 * </script>
 *
 * <template>
 *   <p v-if="user">{{ user.name }}</p>
 *   <Spinner v-else />
 * </template>
 * ```
 */
export function useKeyJSON<T>(key: string): Ref<T | null> {
  const cache = useRecached();
  const value = ref<T | null>(null) as Ref<T | null>;
  const unsub = subscribeKey(cache, key, () => {
    value.value = cache.getJSON<T>(key);
  });
  value.value = cache.getJSON<T>(key);
  onUnmounted(unsub);
  return value;
}
