// Unit tests for the Vue bindings.
//
// A fake `Cache` stands in for the engine; what is under test is the Vue
// contract: refs that track store mutations, `onUnmounted` releasing every
// subscription, and the plugin's inject/provide wiring.
//
// The composables all call `onUnmounted`, which only works inside a component
// instance — so each one is exercised through a real mounted component rather
// than called bare.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { defineComponent, h, type Ref } from 'vue';
import { mount } from '@vue/test-utils';

import { CACHE_KEY, RecachedPlugin, useRecached } from '../src/plugin';
import { useKey, useKeyBytes, useKeyJSON } from '../src/useKey';
import { useKeys } from '../src/useKeys';
import { usePubSub } from '../src/usePubSub';
import { makeFakeCache, type FakeCache } from './fake-cache';
import { resetCreateCache, setCreateCache } from './recached-edge.stub';

let cache: FakeCache;

beforeEach(() => {
  cache = makeFakeCache();
});

afterEach(() => resetCreateCache());

/**
 * Mount a component whose setup runs `body`, with the fake cache provided.
 * Returns the composable's result plus the wrapper, so a test can unmount.
 */
function withSetup<T>(body: () => T) {
  let result!: T;
  const wrapper = mount(
    defineComponent({
      setup() {
        result = body();
        return () => h('div');
      },
    }),
    { global: { provide: { [CACHE_KEY as symbol]: cache } } },
  );
  return { result: () => result, wrapper };
}

describe('plugin', () => {
  it('provides the cache it creates', async () => {
    const provide = vi.fn();
    setCreateCache(async () => cache);

    await RecachedPlugin.install({ provide } as never, { persistence: true });

    expect(provide).toHaveBeenCalledWith(CACHE_KEY, cache);
  });

  it('forwards options to createCache', async () => {
    const createCache = vi.fn(async () => cache);
    setCreateCache(createCache as never);
    const options = { persistence: true, broadcastChannel: 'app' };

    await RecachedPlugin.install({ provide: vi.fn() } as never, options);

    expect(createCache).toHaveBeenCalledWith(options);
  });

  it('explains itself when the plugin was never installed', () => {
    expect(() => mount(defineComponent({ setup: () => (useRecached(), () => h('div')) }))).toThrow(
      /no cache found/,
    );
  });
});

describe('useKey', () => {
  it('reads the current value and tracks mutations', () => {
    cache.seed('theme', 'dark');
    const { result } = withSetup(() => useKey('theme'));
    expect(result().value).toBe('dark');

    cache.seed('theme', 'light');
    cache.emit();
    expect(result().value).toBe('light');
  });

  it('is null for a missing key', () => {
    const { result } = withSetup(() => useKey('nope'));
    expect(result().value).toBeNull();
  });

  it('reports a binary value as absent rather than throwing out of a reactive update', () => {
    cache.get.mockImplementation(() => {
      throw new Error('not valid UTF-8');
    });
    const { result } = withSetup(() => useKey('bin'));
    expect(result().value).toBeNull();
  });

  it('stops listening when the component unmounts', () => {
    const { wrapper } = withSetup(() => useKey('k'));
    expect(cache.listenerCount).toBe(1);
    wrapper.unmount();
    expect(cache.listenerCount).toBe(0);
  });

  it('useKeyJSON parses, and treats invalid JSON as a miss', () => {
    cache.seed('u', '{"id":42}');
    const { result } = withSetup(() => useKeyJSON<{ id: number }>('u'));
    expect(result().value).toEqual({ id: 42 });

    cache.seed('u', 'not json');
    cache.emit();
    expect(result().value).toBeNull();
  });

  it('useKeyBytes exposes the raw bytes', () => {
    cache.seed('b', 'hi');
    const { result } = withSetup(() => useKeyBytes('b'));
    expect(result().value).toEqual(new TextEncoder().encode('hi'));
  });
});

describe('useKeys', () => {
  it('seeds from the local store and starts a live query', () => {
    cache.getMatching.mockReturnValue([['cart:1', 'x']]);
    const { result } = withSetup(() => useKeys('cart:*'));

    expect(cache.liveQuery).toHaveBeenCalledWith('cart:*');
    expect(result().value).toEqual([['cart:1', 'x']]);
  });

  it('refreshes on mutation', () => {
    let pairs: Array<[string, string | null]> = [];
    cache.getMatching.mockImplementation(() => pairs);
    const { result } = withSetup(() => useKeys('p:*'));
    expect(result().value).toEqual([]);

    pairs = [['p:1', 'x']];
    cache.emit();
    expect(result().value).toEqual([['p:1', 'x']]);
  });

  it('stops the live query and the listener on unmount', () => {
    const stop = vi.fn();
    cache.liveQuery.mockReturnValue(stop);

    const { wrapper } = withSetup(() => useKeys('p:*'));
    expect(cache.listenerCount).toBe(1);

    wrapper.unmount();
    expect(stop).toHaveBeenCalledTimes(1);
    expect(cache.listenerCount).toBe(0);
  });
});

describe('change routing', () => {
  // A composable listens to its own key or pattern. Before routing, a write
  // anywhere in the store re-read every composable, and useKeyJSON,
  // useKeyBytes and useKeys assigned a fresh object each time — re-rendering
  // their components on writes that had nothing to do with them.
  it.each([
    ['useKey', () => useKey('theme'), 'get'],
    ['useKeyJSON', () => useKeyJSON('theme'), 'getJSON'],
    ['useKeyBytes', () => useKeyBytes('theme'), 'getBytes'],
  ] as const)('%s ignores a write to another key', (_name, composable, read) => {
    cache.seed('theme', '"dark"');
    withSetup(composable);
    cache[read].mockClear();

    cache.emit(['other']);
    expect(cache[read]).not.toHaveBeenCalled();

    cache.emit(['theme']);
    expect(cache[read]).toHaveBeenCalledTimes(1);
  });

  it('useKeys refreshes only when a matching key changes', () => {
    cache.getMatching.mockReturnValue([['cart:1', 'x']]);
    withSetup(() => useKeys('cart:*'));
    cache.getMatching.mockClear();

    cache.emit(['user:1']);
    expect(cache.getMatching).not.toHaveBeenCalled();

    cache.emit(['cart:2']);
    expect(cache.getMatching).toHaveBeenCalledTimes(1);
  });

  it('falls back to every mutation on a recached-edge without routing', () => {
    cache = makeFakeCache({ legacy: true });
    cache.seed('theme', 'dark');
    const { result } = withSetup(() => useKey('theme'));
    expect(cache.onMutation).toHaveBeenCalled();

    cache.seed('theme', 'light');
    cache.emit(['theme']);
    expect(result().value).toBe('light');
  });
});

describe('usePubSub', () => {
  it('subscribes on setup and unsubscribes on unmount', () => {
    const { wrapper } = withSetup(() => usePubSub('alerts', vi.fn()));
    expect(cache.subscribe).toHaveBeenCalledWith('alerts');

    wrapper.unmount();
    expect(cache.unsubscribe).toHaveBeenCalledWith('alerts');
  });

  it('delivers messages for its channel only, and stops after unmount', () => {
    const handler = vi.fn();
    const { wrapper } = withSetup(() => usePubSub('alerts', handler));

    cache.deliver('alerts', 'ping');
    cache.deliver('other', 'nope');
    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith('ping');

    wrapper.unmount();
    cache.deliver('alerts', 'ping');
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('passes a binary payload through unchanged', () => {
    const handler = vi.fn();
    withSetup(() => usePubSub('bin', handler));

    const payload = new Uint8Array([0xde, 0xad]);
    cache.deliver('bin', payload);
    expect(handler).toHaveBeenCalledWith(payload);
  });
});

/** Type-level guard: the composables must hand back refs, not raw values. */
describe('types', () => {
  it('returns refs', () => {
    const { result } = withSetup(() => useKey('k'));
    const asRef: Ref<string | null> = result();
    expect(asRef).toHaveProperty('value');
  });
});
