import * as fs from 'fs';
import * as path from 'path';
import { logger } from '../logging/logger';

export class DeduplicationStore {
  private seen: Set<string> = new Set();
  private filePath: string;
  private inMemoryMode: boolean;

  constructor(storePath: string = path.join(__dirname, '../../data/seen-requests.json')) {
    this.filePath = storePath;
    this.inMemoryMode = storePath === ':memory:';

    if (!this.inMemoryMode) {
      this.loadFromDisk();
    }
  }

  private loadFromDisk() {
    try {
      if (fs.existsSync(this.filePath)) {
        const data = JSON.parse(fs.readFileSync(this.filePath, 'utf8'));
        this.seen = new Set(data.seen || []);
      }
    } catch (error) {
      logger.warn('Failed to load deduplication store, starting fresh:', error);
      // Ensure directory exists
      const dir = path.dirname(this.filePath);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }
    }
  }

  private saveToDisk() {
    if (this.inMemoryMode) {
      return; // Skip disk I/O in memory mode
    }

    try {
      const dir = path.dirname(this.filePath);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }
      fs.writeFileSync(this.filePath, JSON.stringify({ seen: Array.from(this.seen) }));
    } catch (error) {
      logger.error('Failed to save deduplication store:', error);
    }
  }

  /**
   * Pure predicate — returns true if the request has already been processed.
   * Does NOT mutate state.
   */
  has(requestId: bigint, raffleAddress: string): boolean {
    const key = `${raffleAddress}:${requestId.toString()}`;
    return this.seen.has(key);
  }

  /**
   * Marks a request as successfully processed and persists to disk.
   * Call this only after a successful on-chain submission so that a
   * mid-flight failure does not permanently suppress retries.
   */
  markProcessed(requestId: bigint, raffleAddress: string): void {
    const key = `${raffleAddress}:${requestId.toString()}`;
    if (!this.seen.has(key)) {
      this.seen.add(key);
      this.saveToDisk();
    }
  }

  /**
   * @deprecated Use `has` + `markProcessed` instead.
   * Kept for backwards-compatibility only; will be removed in a future release.
   *
   * Returns true if the request was already seen AND adds it to the seen set
   * on first call — a combined read/write that prevents safe retries on failure.
   */
  isDuplicate(requestId: bigint, raffleAddress: string): boolean {
    const key = `${raffleAddress}:${requestId.toString()}`;
    if (this.seen.has(key)) {
      return true;
    }
    this.seen.add(key);
    this.saveToDisk();
    return false;
  }
}
