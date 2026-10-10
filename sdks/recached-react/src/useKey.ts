import { useCallback, useRef, useSyncExternalStore } from 'react';
import { useRecached } from './context';
import { subscribeKey } from './subscribe';

/**
 * Subscribe to changes to one key, counting them.
 *
 * `useSyncExternalStore` compares snapshots with `Object.is` and re-reads on
 * every render, so a `getSnapshot` that allocates — `getJSON` parses a fresh
 * object, `getBytes` copies a fresh array out of wasm — reports a change every
 * single time and re-renders forever. Counting mutations gives those hooks a
 * cheap way to answer "has anything happened since I last read?" without
 * touching the value, which is what makes a stable snapshot possible.
 *
 * Returns the subscribe function plus the counter ref.
 */
function useKeyVersion(cache: ReturnType<typeof useRecached>, key: string) {
  const version = useRef(0);
  const subscribe = useCallback(
    (onStoreChange: () => void) =>
      subscribeKey(cache, key, () => {
        version.current += 1;
        onStoreChange();
      }),
    [cache, key],
  );
  return { subscribe, version };
}

/** Byte-wise equality, so an unchanged binary value keeps its identity. */
function sameBytes(a: Uint8Array | null, b: Uint8Array | null): boolean {
  if (a === b) return true;
  if (!a || !b || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

/**
 * Reactively read a string key from the Recached store.
 *
 * The component re-renders automatically whenever the key is written or
 * deleted — whether the mutation originated locally, from another tab via
 * BroadcastChannel, or from another client via the server WebSocket.
 *
 * Returns `null` when the key does not exist, has expired, or holds a value that
 * is not valid UTF-8 — a binary value has no string form, so read it with
 * {@link useKeyBytes} instead.
 *
 * Built on React 18's `useSyncExternalStore` — safe with concurrent features.
 *
 * ```tsx
 * function ThemeButton() {
 *   const cache = useRecached();
 *   const theme = useKey('theme');
 *   return (
 *     <button onClick={() => cache.set('theme', theme === 'dark' ? 'light' : 'dark')}>
 *       {theme ?? 'light'}
 *     </button>
 *   );
 * }
 * ```
 */
export function useKey(key: string): string | null {
  const cache = useRecached();
  const subscribe = useCallback(
    (onStoreChange: () => void) => subscribeKey(cache, key, onStoreChange),
    [cache, key],
  );
  return useSyncExternalStore(
    subscribe,
    // `get` throws on a binary value. This runs as a `getSnapshot`, so letting
    // it propagate would take down the render tree over a value the component
    // simply cannot display — report it as absent and let `useKeyBytes` read it.
    () => {
      try {
        return cache.get(key);
      } catch {
        return null;
      }
    },
    () => null,
  );
}

/**
 * Reactively read a key as raw bytes.
 *
 * Behaves identically to {@link useKey} but returns the value's bytes, so it
 * works for binary values a backend wrote — compressed payloads, protobuf,
 * images. Text values come back as their UTF-8 bytes.
 *
 * ```tsx
 * function Thumbnail() {
 *   const bytes = useKeyBytes('thumb:42');
 *   const src = useMemo(
 *     () => (bytes ? URL.createObjectURL(new Blob([bytes])) : null),
 *     [bytes],
 *   );
 *   return src ? <img src={src} /> : null;
 * }
 * ```
 */
export function useKeyBytes(key: string): Uint8Array | null {
  const cache = useRecached();
  const { subscribe, version } = useKeyVersion(cache, key);
  const memo = useRef<{ key: string; version: number; value: Uint8Array | null }>(undefined);

  return useSyncExternalStore(
    subscribe,
    () => {
      const prev = memo.current;
      // Nothing has happened since the last read: hand back the same array.
      if (prev && prev.key === key && prev.version === version.current) return prev.value;

      const next = cache.getBytes(key);
      // A mutation elsewhere in the store must not hand this component a new
      // buffer identity — that would rebuild every object URL downstream.
      const value = prev && prev.key === key && sameBytes(prev.value, next) ? prev.value : next;
      memo.current = { key, version: version.current, value };
      return value;
    },
    () => null,
  );
}

/**
 * Reactively read a JSON-parsed value from the Recached store.
 *
 * Behaves identically to {@link useKey} but parses the stored string as JSON.
 * Returns `null` when the key is missing, expired, or contains invalid JSON.
 *
 * ```tsx
 * interface User { id: number; name: string }
 *
 * function UserCard() {
 *   const user = useKeyJSON<User>('user:42');
 *   if (!user) return <Spinner />;
 *   return <p>{user.name}</p>;
 * }
 * ```
 */
export function useKeyJSON<T>(key: string): T | null {
  const cache = useRecached();
  const { subscribe, version } = useKeyVersion(cache, key);
  const memo = useRef<{ key: string; version: number; raw: string | null; value: T | null }>(
    undefined,
  );

  return useSyncExternalStore(
    subscribe,
    () => {
      const prev = memo.current;
      if (prev && prev.key === key && prev.version === version.current) return prev.value;

      // Compare the stored text before parsing: a mutation to some other key
      // must not produce a new object identity here. `get` throws on a binary
      // value, which is not JSON by definition — that is a miss, not an error.
      let raw: string | null;
      try {
        raw = cache.get(key);
      } catch {
        raw = null;
      }
      if (prev && prev.key === key && prev.raw === raw) {
        memo.current = { ...prev, version: version.current };
        return prev.value;
      }

      const value = cache.getJSON<T>(key);
      memo.current = { key, version: version.current, raw, value };
      return value;
    },
    () => null,
  );
}
