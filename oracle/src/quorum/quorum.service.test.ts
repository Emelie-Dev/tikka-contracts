import { QuorumService } from './quorum.service';
import { rpc as SorobanRpc, scValToNative } from '@stellar/stellar-sdk';

jest.mock('@stellar/stellar-sdk', () => {
  const original = jest.requireActual('@stellar/stellar-sdk');
  const mock = Object.create(original);

  Object.defineProperty(mock, 'scValToNative', {
    value: jest.fn(),
    writable: true,
    configurable: true,
  });

  const mockRpc = Object.create(original.rpc);
  Object.defineProperty(mockRpc, 'assembleTransaction', {
    value: jest.fn().mockImplementation((tx: unknown) => ({
      build: () => tx,
    })),
    writable: true,
    configurable: true,
  });
  mock.rpc = mockRpc;

  return mock;
});

describe('QuorumService', () => {
  const rpcUrl = 'http://localhost:8000';
  const networkPassphrase = 'Test Passphrase';
  const oracleAddress = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHB';
  const raffleContract = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4';

  /** Helper: mock simulateTransaction to return a well-formed Quorum raffle. */
  function mockQuorumRaffle(oracles: string[] = [oracleAddress, 'GOTHERADDRESS'], k = 2): void {
    jest.spyOn(SorobanRpc.Server.prototype, 'simulateTransaction').mockResolvedValue({
      result: { retval: {} },
    } as ReturnType<typeof SorobanRpc.Server.prototype.simulateTransaction>);

    (scValToNative as jest.Mock).mockReturnValue({
      randomness_source: { Quorum: { k, oracles } },
    });
  }

  beforeEach(() => {
    jest.clearAllMocks();
  });

  // ---------------------------------------------------------------------------
  // Basic functionality
  // ---------------------------------------------------------------------------

  it('generates unique secure bigint seeds', () => {
    const service = new QuorumService(rpcUrl, networkPassphrase, oracleAddress);
    const seed1 = service.generateSecureSeed();
    const seed2 = service.generateSecureSeed();
    expect(typeof seed1).toBe('bigint');
    expect(seed1).not.toBe(seed2);
  });

  it('detects participation when oracle is in quorum list', async () => {
    const service = new QuorumService(rpcUrl, networkPassphrase, oracleAddress);
    mockQuorumRaffle();

    const result = await service.checkQuorumParticipation(raffleContract);

    expect(result.isParticipant).toBe(true);
    expect(result.k).toBe(2);
    expect(result.oracles).toContain(oracleAddress);
  });

  it('detects non-participation when oracle is not in list', async () => {
    const service = new QuorumService(rpcUrl, networkPassphrase, oracleAddress);
    mockQuorumRaffle(['GOTHERADDRESS1', 'GOTHERADDRESS2']);

    const result = await service.checkQuorumParticipation(raffleContract);

    expect(result.isParticipant).toBe(false);
  });

  it('returns isParticipant: false for a non-Quorum raffle', async () => {
    const service = new QuorumService(rpcUrl, networkPassphrase, oracleAddress);

    jest.spyOn(SorobanRpc.Server.prototype, 'simulateTransaction').mockResolvedValue({
      result: { retval: {} },
    } as ReturnType<typeof SorobanRpc.Server.prototype.simulateTransaction>);

    (scValToNative as jest.Mock).mockReturnValue({
      randomness_source: { External: null },
    });

    const result = await service.checkQuorumParticipation(raffleContract);

    expect(result.isParticipant).toBe(false);
    expect(result.k).toBe(0);
    expect(result.oracles).toHaveLength(0);
  });

  // ---------------------------------------------------------------------------
  // Memoisation
  // ---------------------------------------------------------------------------

  it('returns the cached result on a second call without an extra RPC round-trip', async () => {
    const service = new QuorumService(rpcUrl, networkPassphrase, oracleAddress);
    mockQuorumRaffle();

    const first = await service.checkQuorumParticipation(raffleContract);
    const second = await service.checkQuorumParticipation(raffleContract);

    // simulateTransaction must only have been called once.
    expect(SorobanRpc.Server.prototype.simulateTransaction).toHaveBeenCalledTimes(1);
    expect(second).toEqual(first);
  });

  it('performs separate RPC calls for different raffle contracts', async () => {
    const service = new QuorumService(rpcUrl, networkPassphrase, oracleAddress);
    const raffleContract2 = 'CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBSC4';
    mockQuorumRaffle();

    await service.checkQuorumParticipation(raffleContract);
    await service.checkQuorumParticipation(raffleContract2);

    expect(SorobanRpc.Server.prototype.simulateTransaction).toHaveBeenCalledTimes(2);
  });

  // ---------------------------------------------------------------------------
  // Timeout
  // ---------------------------------------------------------------------------

  it('rejects with a descriptive error when simulateTransaction hangs past the deadline', async () => {
    // Use a 50 ms deadline so the test completes quickly.
    const service = new QuorumService(rpcUrl, networkPassphrase, oracleAddress, 50);

    // simulateTransaction returns a promise that never settles — simulates a
    // hung RPC endpoint.
    jest
      .spyOn(SorobanRpc.Server.prototype, 'simulateTransaction')
      .mockReturnValue(new Promise(() => undefined));

    await expect(service.checkQuorumParticipation(raffleContract)).rejects.toThrow(
      /checkQuorumParticipation timed out after 50 ms/,
    );
  }, 2_000 /* generous wall-clock budget for CI */);
});
