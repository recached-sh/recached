import path from 'node:path';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  oxc: { jsx: { runtime: 'automatic' } },
  resolve: {
    alias: {
      // `recached-edge` is a peer dependency and is not installed here, but
      // `context.tsx` imports it at module scope. See the stub for details.
      'recached-edge': path.resolve(import.meta.dirname, 'test/recached-edge.stub.ts'),
    },
  },
  test: {
    environment: 'jsdom',
    globals: false,
  },
});
