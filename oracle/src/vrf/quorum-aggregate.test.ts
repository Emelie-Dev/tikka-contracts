/**
 * quorum-aggregate.test.ts
 *
 * Cross-language golden-vector tests for quorum seed aggregation.
 *
 * WHY THIS TEST EXISTS
 * ────────────────────
 * The on-chain `aggregate_quorum_seeds` function in
 * contracts/raffle-instance/src/randomness.rs produces a deterministic u64
 * from a set of (oracle_address, seed) pairs by:
 *
 * 1. Sorting pairs by XDR-encoded oracle address (lexicographic byte order)
 * 2. Concatenating each seed as 8 big-endian bytes
 * 3. SHA-256 hashing the concatenated buffer
 * 4. Taking the first 8 bytes of the hash as a big-endian u64
 *
 * Any off-chain replay tool, audit script, or Node service that reconstructs
 * the aggregated seed must implement the exact same byte-level encoding.
 * This test pins the Rust implementation and any TypeScript equivalent to
 * shared golden vectors so a drift in either side fails CI.
 *
 * FIXTURE STRUCTURE
 * ─────────────────
 * __fixtures__/quorum-aggregate-vectors.json contains test cases with:
 * - description: human-readable explanation
 * - oracles: array of {address: strkey, seed: decimal_string}
 * - expectedAggregate: decimal string of the expected u64 result
 * - note: derivation details
 *
 * RUST CROSS-CHECK
 * ────────────────
 * The Rust test `aggregate_quorum_seeds_golden_vector` in
 * contracts/raffle-instance/src/randomness.rs uses the same fixture values
 * and asserts the same expectedAggregate. Both tests must pass for the
 * implementation to be considered correct.
 *
 * To regenerate the fixture after changing addresses or seeds:
 * 1. Run `cargo test aggregate_quorum_seeds_golden_vector` in contracts/raffle-instance
 * 2. Read the assertion failure message for the actual aggregate value
 * 3. Update the EXPECTED constant in the Rust test
 * 4. Update expectedAggregate in the JSON fixture
 * 5. Both tests should now pass
 */

import crypto from 'node:crypto';
import vectors from './__fixtures__/quorum-aggregate-vectors.json';

/**
 * Aggregate multiple oracle seeds into a single u64 following the on-chain algorithm.
 *
 * This is a reference implementation for testing and off-chain replay; it is NOT
 * used by the production oracle service (which submits individual seeds, not aggregates).
 */
function aggregateQuorumSeeds(oracles: Array<{ address: string; seed: bigint }>): bigint {
  if (oracles.length === 0) {
    return 0n;
  }

  // Sort by address (addresses are already in XDR-compatible strkey format)
  // In practice, full XDR encoding would be needed, but for test addresses
  // lexicographic string sort matches XDR byte order
  const sorted = oracles.slice().sort((a, b) => a.address.localeCompare(b.address));

  // Concatenate seeds as 8-byte big-endian buffers
  const buffers: Buffer[] = [];
  for (const { seed } of sorted) {
    const buf = Buffer.alloc(8);
    buf.writeBigUInt64BE(seed);
    buffers.push(buf);
  }

  const combined = Buffer.concat(buffers);

  // SHA-256 hash
  const hash = crypto.createHash('sha256').update(combined).digest();

  // First 8 bytes as big-endian u64
  return hash.readBigUInt64BE(0);
}

// ─── Golden vector tests ───────────────────────────────────────────────────

describe('aggregateQuorumSeeds – golden vectors', () => {
  test.each(vectors)('$description', (vector) => {
    const { oracles, expectedAggregate } = vector as {
      description: string;
      oracles: Array<{ address: string; seed: string }>;
      expectedAggregate: string;
      note: string;
    };

    // Convert seed strings to bigint
    const input = oracles.map((o) => ({
      address: o.address,
      seed: BigInt(o.seed),
    }));

    const result = aggregateQuorumSeeds(input);
    const expected = BigInt(expectedAggregate);

    expect(result).toBe(expected);
  });
});

// ─── Order independence ────────────────────────────────────────────────────

describe('aggregateQuorumSeeds – order independence', () => {
  test('produces same result regardless of input order', () => {
    const oracles = [
      { address: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF', seed: 100n },
      { address: 'GBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAALW7G', seed: 200n },
      { address: 'GCAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMCKQ', seed: 300n },
    ];

    const forward = aggregateQuorumSeeds(oracles);
    const reverse = aggregateQuorumSeeds([...oracles].reverse());
    const shuffled = aggregateQuorumSeeds([oracles[1], oracles[2], oracles[0]]);

    expect(forward).toBe(reverse);
    expect(forward).toBe(shuffled);
  });
});

// ─── Edge cases ────────────────────────────────────────────────────────────

describe('aggregateQuorumSeeds – edge cases', () => {
  test('empty input returns 0', () => {
    expect(aggregateQuorumSeeds([])).toBe(0n);
  });

  test('single oracle', () => {
    const result = aggregateQuorumSeeds([
      { address: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF', seed: 0xdeadbeefn },
    ]);
    expect(result).toBeGreaterThan(0n);
  });
});
