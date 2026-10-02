import { DeduplicationStore } from './deduplication.store';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

describe('DeduplicationStore', () => {
  let testStorePath: string;
  let store: DeduplicationStore;

  beforeEach(() => {
    // Create a unique temporary file for each test
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'dedup-test-'));
    testStorePath = path.join(tempDir, 'seen-requests.json');
    store = new DeduplicationStore(testStorePath);
  });

  afterEach(() => {
    // Cleanup
    if (fs.existsSync(testStorePath)) {
      fs.unlinkSync(testStorePath);
    }
    const tempDir = path.dirname(testStorePath);
    if (fs.existsSync(tempDir)) {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
  });

  // ---------------------------------------------------------------------------
  // has() — pure predicate, must never mutate
  // ---------------------------------------------------------------------------

  describe('has()', () => {
    it('returns false for an unseen request', () => {
      expect(store.has(1n, 'addr1')).toBe(false);
    });

    it('does not mutate state — repeated calls still return false', () => {
      expect(store.has(1n, 'addr1')).toBe(false);
      expect(store.has(1n, 'addr1')).toBe(false);
    });

    it('returns true only after markProcessed is called', () => {
      expect(store.has(1n, 'addr1')).toBe(false);
      store.markProcessed(1n, 'addr1');
      expect(store.has(1n, 'addr1')).toBe(true);
    });
  });

  // ---------------------------------------------------------------------------
  // markProcessed() — explicit mutation
  // ---------------------------------------------------------------------------

  describe('markProcessed()', () => {
    it('marks a request so subsequent has() returns true', () => {
      store.markProcessed(1n, 'addr1');
      expect(store.has(1n, 'addr1')).toBe(true);
    });

    it('is idempotent — calling twice does not throw', () => {
      store.markProcessed(1n, 'addr1');
      expect(() => store.markProcessed(1n, 'addr1')).not.toThrow();
      expect(store.has(1n, 'addr1')).toBe(true);
    });

    it('distinct ids remain independent', () => {
      store.markProcessed(1n, 'addr1');

      // Different requestId, same address — should not be marked
      expect(store.has(2n, 'addr1')).toBe(false);

      // Same requestId, different address — should not be marked
      expect(store.has(1n, 'addr2')).toBe(false);
    });

    it('persists to disk so a restarted store still sees marked requests', () => {
      store.markProcessed(1n, 'addr1');
      store.markProcessed(2n, 'addr2');

      const restarted = new DeduplicationStore(testStorePath);
      expect(restarted.has(1n, 'addr1')).toBe(true);
      expect(restarted.has(2n, 'addr2')).toBe(true);
      expect(restarted.has(3n, 'addr1')).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // Retry safety — the key contract from issue #1035
  // ---------------------------------------------------------------------------

  describe('retry safety', () => {
    it('a failed submission can be retried because has() alone did not mark it', () => {
      // Simulate: check before attempt
      expect(store.has(1n, 'addr1')).toBe(false);

      // Simulate: submission throws — markProcessed is never called
      // Next retry: check again
      expect(store.has(1n, 'addr1')).toBe(false); // NOT skipped as duplicate

      // Simulate: retry succeeds — now mark
      store.markProcessed(1n, 'addr1');
      expect(store.has(1n, 'addr1')).toBe(true);
    });
  });

  // ---------------------------------------------------------------------------
  // Misc
  // ---------------------------------------------------------------------------

  it('behaves in-memory if disk is unavailable (e.g., bad path)', () => {
    // Create a store that cannot write to disk because the path is a directory
    const dirPath = path.dirname(testStorePath);
    const badStore = new DeduplicationStore(dirPath);

    // It should still work in-memory (disk write errors are caught and logged)
    expect(badStore.has(1n, 'addr1')).toBe(false);
    badStore.markProcessed(1n, 'addr1');
    expect(badStore.has(1n, 'addr1')).toBe(true);
  });

  it('no eviction behavior under many entries (unbounded growth)', () => {
    const entriesToInsert = 1000;
    for (let i = 0; i < entriesToInsert; i++) {
      store.markProcessed(BigInt(i), 'addr1');
    }

    // Verify the first entry is still remembered (no eviction)
    expect(store.has(0n, 'addr1')).toBe(true);

    // Verify the last entry is remembered
    expect(store.has(BigInt(entriesToInsert - 1), 'addr1')).toBe(true);

    // Disk file should contain all entries
    const data = JSON.parse(fs.readFileSync(testStorePath, 'utf8'));
    expect(data.seen.length).toBe(entriesToInsert);
  });
});
