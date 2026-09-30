/**
 * Narrow view of the on-chain Raffle struct decoded from a `get_raffle`
 * simulation result.  Only the fields the oracle actually needs are modelled
 * here — everything else is intentionally omitted.
 *
 * Keep this in sync with the canonical definition in:
 *   contracts/raffle-instance/src/lib.rs  →  struct Raffle
 *   contracts/raffle-shared/src/lib.rs    →  enum RandomnessSource / struct QuorumConfig
 *
 * RandomnessSource variants (as decoded by scValToNative):
 *   { Internal: void | null }
 *   { External: void | null }
 *   { CommitReveal: void | null }
 *   { Quorum: { k: number; oracles: string[] } }
 *
 * scValToNative maps a Soroban enum variant to a plain object with a single
 * key equal to the variant name; the value is either the inner payload or null
 * for unit variants.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface QuorumConfig {
  /** Minimum number of oracle submissions required to reach quorum (u32). */
  k: number;
  /** Ordered list of registered oracle Stellar addresses. */
  oracles: string[];
}

export type RandomnessSource =
  | { Internal: null }
  | { External: null }
  | { CommitReveal: null }
  | { Quorum: QuorumConfig };

/**
 * The subset of Raffle fields used by the oracle.
 * `randomness_source` drives the quorum-vs-single-oracle dispatch decision.
 */
export interface RaffleView {
  randomness_source: RandomnessSource;
}

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

/**
 * Parses the raw value returned by `scValToNative` for a `get_raffle` call
 * into a typed `RaffleView`.
 *
 * Throws a descriptive error for any unexpected shape so that a schema change
 * in the contract is surfaced immediately instead of silently falling through
 * to single-oracle mode.
 *
 * @param raw - The `unknown` value produced by `scValToNative(retval)`.
 */
export function parseRaffleView(raw: unknown): RaffleView {
  if (raw === null || typeof raw !== 'object') {
    throw new Error(
      `parseRaffleView: expected an object, got ${JSON.stringify(raw)}`
    );
  }

  const obj = raw as Record<string, unknown>;

  if (!('randomness_source' in obj)) {
    throw new Error(
      `parseRaffleView: missing field 'randomness_source' in ${JSON.stringify(Object.keys(obj))}`
    );
  }

  const source = obj['randomness_source'];
  const parsedSource = parseRandomnessSource(source);

  return { randomness_source: parsedSource };
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

function parseRandomnessSource(raw: unknown): RandomnessSource {
  if (raw === null || typeof raw !== 'object') {
    throw new Error(
      `parseRaffleView: 'randomness_source' must be an object, got ${JSON.stringify(raw)}`
    );
  }

  const obj = raw as Record<string, unknown>;
  const keys = Object.keys(obj);

  if (keys.length !== 1) {
    throw new Error(
      `parseRaffleView: 'randomness_source' must have exactly one variant key, got ${JSON.stringify(keys)}`
    );
  }

  const variant = keys[0] as string;

  switch (variant) {
    case 'Internal':
      return { Internal: null };
    case 'External':
      return { External: null };
    case 'CommitReveal':
      return { CommitReveal: null };
    case 'Quorum':
      return { Quorum: parseQuorumConfig(obj['Quorum']) };
    default:
      throw new Error(
        `parseRaffleView: unknown 'randomness_source' variant '${variant}'. ` +
          `Update raffle.schema.ts to match contracts/raffle-shared/src/lib.rs RandomnessSource.`
      );
  }
}

function parseQuorumConfig(raw: unknown): QuorumConfig {
  if (raw === null || typeof raw !== 'object') {
    throw new Error(
      `parseRaffleView: Quorum payload must be an object, got ${JSON.stringify(raw)}`
    );
  }

  const obj = raw as Record<string, unknown>;

  if (!('k' in obj)) {
    throw new Error(`parseRaffleView: Quorum config missing field 'k'`);
  }
  if (!('oracles' in obj)) {
    throw new Error(`parseRaffleView: Quorum config missing field 'oracles'`);
  }

  const k = Number(obj['k']);
  if (!Number.isInteger(k) || k < 1) {
    throw new Error(
      `parseRaffleView: Quorum 'k' must be a positive integer, got ${JSON.stringify(obj['k'])}`
    );
  }

  const rawOracles = obj['oracles'];
  if (!Array.isArray(rawOracles)) {
    throw new Error(
      `parseRaffleView: Quorum 'oracles' must be an array, got ${JSON.stringify(rawOracles)}`
    );
  }

  const oracles = rawOracles.map((addr: unknown, i: number) => {
    if (addr === null || addr === undefined || typeof (addr as { toString?: unknown }).toString !== 'function') {
      throw new Error(
        `parseRaffleView: Quorum oracle at index ${i} is not addressable: ${JSON.stringify(addr)}`
      );
    }
    return String(addr);
  });

  return { k, oracles };
}
