// Unit tests for the TypeScript SDK.
//
// These cover the wrapper, not the engine: null mapping, JSON handling, the
// numeric marshalling across the wasm boundary, listener bookkeeping, live-query
// ref-counting, and the order `createCache` applies its options. The engine
// itself is tested in Rust, and `scripts/verify-package.mjs` exercises the real
// wasm through this same surface against a packed tarball.
//
// Everything here runs against a fake `RawCache`, which is what makes the
// boundary assertions possible: a spy records exactly what the SDK hands to
// wasm-bindgen, including types.

import { beforeEach, describe, expect, it, vi } from 'vitest';

// Must be hoisted above the import of ./sdk.js, whose `createCache` dynamically
// imports this module. `vi.mock` is hoisted by vitest, and the factory is
// evaluated lazily on first import.
vi.mock('./pkg/recached_edge.js', () => ({
  default: vi.fn(async () => undefined),
  // A `function`, not an arrow: `createCache` calls it with `new`, which
  // vitest 4 refuses for an arrow implementation.
  RecachedCache: vi.fn(function () {
    return makeRaw();
  }),
}));

import { Cache, createCache, init } from './sdk.js';

/** A stand-in for the generated `RecachedCache`, with the real signatures. */
function makeRaw() {
  const store = new Map<string, string | Uint8Array>();
  return {
    // storage
    set: vi.fn((k: string, v: string) => (store.set(k, v), 'OK')),
    setBytes: vi.fn((k: string, v: Uint8Array) => (store.set(k, v), 'OK')),
    set_ex: vi.fn(() => 'OK'),
    get: vi.fn((k: string) => store.get(k) as string | undefined),
    getBytes: vi.fn((k: string) => store.get(k) as Uint8Array | undefined),
    del: vi.fn((k: string) => (store.delete(k) ? 1 : 0)),
    exists: vi.fn((k: string) => store.has(k)),
    ttl: vi.fn(() => -1),
    // `i64` in Rust — wasm-bindgen marshals this as bigint in both directions.
    incr_by: vi.fn((_k: string, delta: bigint) => delta),
    // json
    jset: vi.fn(() => 'OK'),
    jget: vi.fn((): string | undefined => undefined),
    jmerge: vi.fn(() => 'OK'),
    // connection + session
    connect: vi.fn(),
    auth: vi.fn(() => 'OK'),
    disconnect: vi.fn(),
    set_auto_reconnect: vi.fn(),
    broadcast: vi.fn(),
    enable_persistence: vi.fn(async () => undefined),
    clear_persistence: vi.fn(async () => undefined),
    sync_token: vi.fn(),
    sync_scopes: vi.fn(),
    // pub/sub + queries
    publish: vi.fn(),
    publishBytes: vi.fn(),
    subscribe: vi.fn(),
    unsubscribe: vi.fn(),
    live_query: vi.fn(),
    live_unquery: vi.fn(),
    get_matching: vi.fn(() => [] as Array<[string, string | Uint8Array | null]>),
    // The engine's glob matcher; `*` and `?` are all these tests use.
    anyKeyMatches: vi.fn((pattern: string, keys: string[]) => {
      const source = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&');
      const re = new RegExp(`^${source.replace(/\*/g, '.*').replace(/\?/g, '.')}$`);
      return keys.some((k) => re.test(k));
    }),
    pending_writes: vi.fn(() => 0),
    // callbacks the Cache constructor installs
    set_mutation_callback: vi.fn<(cb: (keys: string[]) => void) => void>(),
    set_message_callback: vi.fn(),
    set_outbox_full_callback: vi.fn(),
    free: vi.fn(),
  };
}

type Raw = ReturnType<typeof makeRaw>;

let raw: Raw;
let cache: Cache;

/** Invoke the callback the Cache installed on the raw handle. */
const fireMutation = (keys: string[] = ['k']) => raw.set_mutation_callback.mock.calls[0]![0](keys);
const fireMessage = (channel: string, msg: string | Uint8Array) =>
  raw.set_message_callback.mock.calls[0]![0](channel, msg);
const fireOutboxFull = (dropped: number, pending: number) =>
  raw.set_outbox_full_callback.mock.calls[0]![0](dropped, pending);

beforeEach(() => {
  raw = makeRaw();
  cache = new Cache(raw as never);
});

describe('reads', () => {
  it('maps a missing key to null rather than undefined', () => {
    expect(cache.get('nope')).toBeNull();
    expect(cache.getBytes('nope')).toBeNull();
  });

  it('returns the stored value', () => {
    cache.set('k', 'v');
    expect(cache.get('k')).toBe('v');
  });

  it('parses JSON', () => {
    raw.get.mockReturnValue('{"id":42}');
    expect(cache.getJSON<{ id: number }>('u')).toEqual({ id: 42 });
  });

  it('treats unparseable JSON as a miss instead of throwing', () => {
    raw.get.mockReturnValue('not json');
    expect(cache.getJSON('u')).toBeNull();
  });

  it('treats a binary value as a JSON miss', () => {
    // `get` throws on non-UTF-8 rather than returning mangled text.
    raw.get.mockImplementation(() => {
      throw new Error('not valid UTF-8');
    });
    expect(cache.getJSON('bin')).toBeNull();
    expect(() => cache.get('bin')).toThrow();
  });

  it('passes ttl and exists straight through', () => {
    raw.ttl.mockReturnValue(60);
    expect(cache.ttl('s')).toBe(60);
    cache.set('k', 'v');
    expect(cache.exists('k')).toBe(true);
  });
});

describe('writes', () => {
  it('setJSON serializes, and uses set_ex only when a ttl is given', () => {
    cache.setJSON('u', { id: 42 });
    expect(raw.set).toHaveBeenCalledWith('u', '{"id":42}');
    expect(raw.set_ex).not.toHaveBeenCalled();

    cache.setJSON('u', { id: 42 }, 300);
    expect(raw.set_ex).toHaveBeenCalledWith('u', '{"id":42}', 300);
  });

  it('del reports whether the key existed', () => {
    cache.set('k', 'v');
    expect(cache.del('k')).toBe(true);
    expect(cache.del('k')).toBe(false);
  });

  it('setBytes stores the exact bytes', () => {
    const bytes = new Uint8Array([1, 2, 3]);
    cache.setBytes('b', bytes);
    expect(raw.setBytes).toHaveBeenCalledWith('b', bytes);
    expect(cache.getBytes('b')).toEqual(bytes);
  });
});

describe('counters cross the wasm boundary as bigint', () => {
  // Regression: `incr_by` takes an i64, so wasm-bindgen expects a bigint.
  // Passing a plain number threw `TypeError: Cannot convert 1 to a BigInt`
  // on every call, in every runtime, for several releases.
  it('passes a bigint delta and returns a number', () => {
    const result = cache.incr('n');
    expect(raw.incr_by).toHaveBeenCalledWith('n', 1n);
    expect(result).toBe(1);
    expect(typeof result).toBe('number');
  });

  it('negates the delta for decr', () => {
    cache.decr('n', 2);
    expect(raw.incr_by).toHaveBeenCalledWith('n', -2n);
  });

  it('accepts an explicit step', () => {
    cache.incr('n', 5);
    expect(raw.incr_by).toHaveBeenCalledWith('n', 5n);
  });

  it('converts a bigint reply back to a number', () => {
    raw.incr_by.mockReturnValue(9007199254740990n);
    expect(cache.incr('n')).toBe(9007199254740990);
  });
});

describe('JSON documents', () => {
  it('serializes the value and reports the path', () => {
    cache.jset('d', '$.title', 'hello');
    expect(raw.jset).toHaveBeenCalledWith('d', '$.title', '"hello"');
  });

  it('throws when the engine rejects the write', () => {
    raw.jset.mockReturnValue('ERR path not found');
    expect(() => cache.jset('d', '$.a.b', 1)).toThrow('ERR path not found');

    raw.jmerge.mockReturnValue('ERR not an object');
    expect(() => cache.jmerge('d', { a: 1 })).toThrow('ERR not an object');
  });

  it('jget parses, and maps a missing document to null', () => {
    raw.jget.mockReturnValue('{"title":"a"}');
    expect(cache.jget('d')).toEqual({ title: 'a' });

    raw.jget.mockReturnValue(undefined);
    expect(cache.jget('d')).toBeNull();

    raw.jget.mockReturnValue('{oops');
    expect(cache.jget('d')).toBeNull();
  });
});

describe('mutation listeners', () => {
  it('notifies every listener and stops after unsubscribe', () => {
    const a = vi.fn();
    const b = vi.fn();
    const stopA = cache.onMutation(a);
    cache.onMutation(b);

    fireMutation();
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledTimes(1);

    stopA();
    fireMutation();
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledTimes(2);
  });

  it('hands every listener the changed keys', () => {
    const a = vi.fn();
    cache.onMutation(a);
    fireMutation(['x', 'y']);
    expect(a).toHaveBeenCalledWith(['x', 'y']);
  });

  it('notifies a key listener only when its own key changes', () => {
    const theme = vi.fn();
    const stop = cache.onKeyChange('theme', theme);

    fireMutation(['other']);
    expect(theme).not.toHaveBeenCalled();

    fireMutation(['other', 'theme']);
    expect(theme).toHaveBeenCalledTimes(1);

    stop();
    fireMutation(['theme']);
    expect(theme).toHaveBeenCalledTimes(1);
  });

  it('notifies a pattern listener once per mutation that touches a match', () => {
    const cart = vi.fn();
    const stop = cache.onPatternChange('cart:*', cart);

    fireMutation(['user:1']);
    expect(cart).not.toHaveBeenCalled();

    // A snapshot naming many matching keys is still one notification, and the
    // keys cross into wasm once for the pattern, not once per key.
    raw.anyKeyMatches.mockClear();
    fireMutation(['cart:1', 'cart:2', 'cart:3']);
    expect(cart).toHaveBeenCalledTimes(1);
    expect(raw.anyKeyMatches).toHaveBeenCalledTimes(1);
    expect(raw.anyKeyMatches).toHaveBeenCalledWith('cart:*', ['cart:1', 'cart:2', 'cart:3']);

    stop();
    fireMutation(['cart:1']);
    expect(cart).toHaveBeenCalledTimes(1);
  });

  it('stops matching a pattern once its last listener leaves', () => {
    const stopA = cache.onPatternChange('cart:*', vi.fn());
    const stopB = cache.onPatternChange('cart:*', vi.fn());
    stopA();
    stopA(); // idempotent
    raw.anyKeyMatches.mockClear();
    fireMutation(['cart:1']);
    expect(raw.anyKeyMatches).toHaveBeenCalledTimes(1);

    stopB();
    raw.anyKeyMatches.mockClear();
    fireMutation(['cart:1']);
    expect(raw.anyKeyMatches).not.toHaveBeenCalled();
  });

  it('routes nothing for a mutation that changed no keys', () => {
    const all = vi.fn();
    const one = vi.fn();
    cache.onMutation(all);
    cache.onKeyChange('k', one);
    cache.onPatternChange('*', one);
    fireMutation([]);
    expect(all).toHaveBeenCalledWith([]);
    expect(one).not.toHaveBeenCalled();
  });

  it('surfaces outbox overflow with the dropped id and queue depth', () => {
    const onFull = vi.fn();
    cache.onOutboxFull(onFull);
    fireOutboxFull(7, 10_000);
    expect(onFull).toHaveBeenCalledWith(7, 10_000);
  });
});

describe('pub/sub listeners', () => {
  it('routes a message only to listeners of that channel', () => {
    const alerts = vi.fn();
    const other = vi.fn();
    cache.onMessage('alerts', alerts);
    cache.onMessage('other', other);

    fireMessage('alerts', 'ping');
    expect(alerts).toHaveBeenCalledWith('ping');
    expect(other).not.toHaveBeenCalled();
  });

  it('delivers a binary payload unchanged', () => {
    const seen = vi.fn();
    cache.onMessage('bin', seen);
    const payload = new Uint8Array([0xde, 0xad]);
    fireMessage('bin', payload);
    expect(seen).toHaveBeenCalledWith(payload);
  });

  it('stops delivering after unsubscribe, and tolerates a message for an unknown channel', () => {
    const seen = vi.fn();
    const stop = cache.onMessage('alerts', seen);
    stop();
    fireMessage('alerts', 'ping');
    expect(seen).not.toHaveBeenCalled();
    expect(() => fireMessage('never-subscribed', 'x')).not.toThrow();
  });

  it('onMessage is local bookkeeping — it does not SUBSCRIBE on its own', () => {
    cache.onMessage('alerts', vi.fn());
    expect(raw.subscribe).not.toHaveBeenCalled();
    cache.subscribe('alerts');
    expect(raw.subscribe).toHaveBeenCalledWith('alerts');
  });
});

describe('liveQuery ref-counting', () => {
  it('subscribes once for a shared pattern and unsubscribes when the last caller stops', () => {
    const stop1 = cache.liveQuery('cart:*');
    const stop2 = cache.liveQuery('cart:*');
    expect(raw.live_query).toHaveBeenCalledTimes(1);

    stop1();
    expect(raw.live_unquery).not.toHaveBeenCalled();

    stop2();
    expect(raw.live_unquery).toHaveBeenCalledWith('cart:*');
  });

  it('ignores a repeated stop, so one component cannot cancel another', () => {
    const stopA = cache.liveQuery('p:*');
    const stopB = cache.liveQuery('p:*');
    stopA();
    stopA();
    stopA();
    expect(raw.live_unquery).not.toHaveBeenCalled();
    stopB();
    expect(raw.live_unquery).toHaveBeenCalledTimes(1);
  });

  it('re-subscribes after the count returns to zero', () => {
    cache.liveQuery('p:*')();
    cache.liveQuery('p:*');
    expect(raw.live_query).toHaveBeenCalledTimes(2);
  });

  it('tracks patterns independently', () => {
    cache.liveQuery('a:*');
    cache.liveQuery('b:*');
    expect(raw.live_query).toHaveBeenCalledTimes(2);
    expect(raw.live_query).toHaveBeenNthCalledWith(1, 'a:*');
    expect(raw.live_query).toHaveBeenNthCalledWith(2, 'b:*');
  });
});

describe('passthroughs', () => {
  it('joins sync scopes into the comma-separated form the binding wants', () => {
    cache.syncScopes(['cart:*', 'user:42:*']);
    expect(raw.sync_scopes).toHaveBeenCalledWith('cart:*,user:42:*');
  });

  it('forwards tokens, disconnect, pending writes and getMatching', () => {
    cache.syncToken('tok');
    expect(raw.sync_token).toHaveBeenCalledWith('tok');

    cache.disconnect();
    expect(raw.disconnect).toHaveBeenCalled();

    raw.pending_writes.mockReturnValue(3);
    expect(cache.pendingWrites()).toBe(3);

    raw.get_matching.mockReturnValue([['p:1', 'x']]);
    expect(cache.getMatching('p:*')).toEqual([['p:1', 'x']]);
  });
});

describe('createCache', () => {
  beforeEach(() => vi.clearAllMocks());

  it('opens no socket and touches no storage without options', async () => {
    const c = await createCache();
    const r = (c as unknown as { raw: Raw }).raw;
    expect(r.connect).not.toHaveBeenCalled();
    expect(r.enable_persistence).not.toHaveBeenCalled();
    expect(r.broadcast).not.toHaveBeenCalled();
  });

  it('applies persistence and broadcast before connecting', async () => {
    const c = await createCache({
      persistence: true,
      broadcastChannel: 'app',
      connect: { url: 'ws://localhost:6380' },
    });
    const r = (c as unknown as { raw: Raw }).raw;
    expect(r.enable_persistence).toHaveBeenCalled();
    expect(r.broadcast).toHaveBeenCalledWith('app');
    expect(r.connect).toHaveBeenCalledWith('ws://localhost:6380');
    // Ordering matters: a socket that opens before the WAL is loaded can push
    // state into a store that is about to be replayed over.
    expect(r.enable_persistence.mock.invocationCallOrder[0]!).toBeLessThan(
      r.connect.mock.invocationCallOrder[0]!,
    );
  });

  it('authenticates only when a password is supplied', async () => {
    const c = await createCache({ connect: { url: 'ws://x', password: 'hunter2' } });
    expect((c as unknown as { raw: Raw }).raw.auth).toHaveBeenCalledWith('hunter2');

    vi.clearAllMocks();
    const d = await createCache({ connect: { url: 'ws://x' } });
    expect((d as unknown as { raw: Raw }).raw.auth).not.toHaveBeenCalled();
  });

  it('prefers a signed token over bandwidth-filter scopes', async () => {
    const c = await createCache({
      connect: { url: 'ws://x', syncToken: 'signed', syncScopes: ['cart:*'] },
    });
    const r = (c as unknown as { raw: Raw }).raw;
    expect(r.sync_token).toHaveBeenCalledWith('signed');
    expect(r.sync_scopes).not.toHaveBeenCalled();
  });

  it('falls back to scopes when there is no token', async () => {
    const c = await createCache({ connect: { url: 'ws://x', syncScopes: ['a:*', 'b:*'] } });
    expect((c as unknown as { raw: Raw }).raw.sync_scopes).toHaveBeenCalledWith('a:*,b:*');
  });

  it('disables auto-reconnect only when explicitly set to false', async () => {
    const c = await createCache({ connect: { url: 'ws://x', reconnect: false } });
    expect((c as unknown as { raw: Raw }).raw.set_auto_reconnect).toHaveBeenCalledWith(false);

    vi.clearAllMocks();
    const d = await createCache({ connect: { url: 'ws://x' } });
    expect((d as unknown as { raw: Raw }).raw.set_auto_reconnect).not.toHaveBeenCalled();
  });

  it('ignores connect options when no url block is given', async () => {
    const c = await createCache({ persistence: true });
    expect((c as unknown as { raw: Raw }).raw.connect).not.toHaveBeenCalled();
  });

  it('init resolves without constructing a cache', async () => {
    await expect(init()).resolves.toBeUndefined();
  });
});
