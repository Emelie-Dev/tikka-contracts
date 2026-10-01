import {
  Account,
  Contract,
  Keypair,
  rpc as SorobanRpc,
  scValToNative,
  TransactionBuilder,
} from '@stellar/stellar-sdk';
import { randomBytes } from 'crypto';
import { parseRaffleView } from './raffle.schema';

/**
 * A single throwaway source account used for all simulation transactions.
 * `randomness_source` is immutable after raffle init, so the same keypair can
 * be reused indefinitely — we only need a valid Stellar account shape to
 * satisfy TransactionBuilder; it is never submitted on-chain.
 */
const DUMMY_ACCOUNT = new Account(Keypair.random().publicKey(), '0');

export interface QuorumParticipation {
  isParticipant: boolean;
  k: number;
  oracles: string[];
}

export class QuorumService {
  private readonly server: SorobanRpc.Server;
  /**
   * Memoisation cache: raffleContractId → participation result.
   *
   * `randomness_source` is set at raffle initialisation and never mutated, so
   * the result of `checkQuorumParticipation` is stable for the lifetime of a
   * raffle.  Caching avoids a redundant RPC round-trip on every job for the
   * same contract.
   */
  private readonly participationCache = new Map<string, QuorumParticipation>();

  constructor(
    private readonly rpcUrl: string,
    private readonly networkPassphrase: string,
    private readonly oracleAddress: string,
    private readonly rpcSimulateTimeoutMs: number = 10_000,
  ) {
    this.server = new SorobanRpc.Server(rpcUrl, { allowHttp: rpcUrl.startsWith('http://') });
  }

  /**
   * Generates a cryptographically secure random u64 seed.
   */
  generateSecureSeed(): bigint {
    const bytes = randomBytes(8);
    return bytes.readBigUInt64BE(0);
  }

  /**
   * Queries the raffle contract's `get_raffle` view and checks whether this
   * oracle is a registered participant in a Quorum raffle.
   *
   * Results are memoised per raffle contract ID because `randomness_source` is
   * immutable after initialisation — repeated calls for the same contract
   * return the cached value without an additional RPC round-trip.
   *
   * Throws if:
   * - The RPC call does not complete within `rpcSimulateTimeoutMs`.
   * - The simulation returns an error response.
   * - The decoded return value does not match the expected Raffle shape.
   */
  async checkQuorumParticipation(raffleContractId: string): Promise<QuorumParticipation> {
    const cached = this.participationCache.get(raffleContractId);
    if (cached !== undefined) {
      return cached;
    }

    const contract = new Contract(raffleContractId);
    // Sequence numbers on the dummy account are not validated during simulation,
    // but TransactionBuilder increments the in-memory counter; reset to '0' on
    // each call so the object stays valid across multiple concurrent raffles.
    DUMMY_ACCOUNT.incrementSequenceNumber();
    const tx = new TransactionBuilder(DUMMY_ACCOUNT, {
      // Fee value is irrelevant for a simulate-only call but must be non-empty.
      fee: '100',
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(contract.call('get_raffle'))
      .setTimeout(30)
      .build();

    // Race the simulation against a hard deadline so a hung RPC endpoint cannot
    // stall the serial queue-processing loop indefinitely.
    const timeoutSignal = AbortSignal.timeout(this.rpcSimulateTimeoutMs);
    const timeoutPromise = new Promise<never>((_, reject) => {
      timeoutSignal.addEventListener('abort', () => {
        reject(
          new Error(
            `checkQuorumParticipation timed out after ${this.rpcSimulateTimeoutMs} ms ` +
              `for raffle ${raffleContractId}`,
          ),
        );
      });
    });

    const simulated = await Promise.race([
      this.server.simulateTransaction(tx),
      timeoutPromise,
    ]);

    if (SorobanRpc.Api.isSimulationError(simulated)) {
      throw new Error(`Failed to simulate get_raffle: ${JSON.stringify(simulated)}`);
    }
    if (!simulated.result?.retval) {
      throw new Error(`get_raffle returned empty result for raffle ${raffleContractId}`);
    }

    const raffle = parseRaffleView(scValToNative(simulated.result.retval));
    const source = raffle.randomness_source;

    const result: QuorumParticipation = 'Quorum' in source
      ? { isParticipant: source.Quorum.oracles.includes(this.oracleAddress), ...source.Quorum }
      : { isParticipant: false, k: 0, oracles: [] };

    this.participationCache.set(raffleContractId, result);
    return result;
  }
}
