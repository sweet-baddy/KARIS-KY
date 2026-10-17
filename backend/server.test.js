const test = require('node:test');
const assert = require('node:assert/strict');
const { StrKey } = require('@stellar/stellar-sdk');
const { ENTRYPOINTS, createBackendServer } = require('./server');

async function withServer(options, run) {
  const server = createBackendServer(options);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    await run(`http://127.0.0.1:${server.address().port}`);
  } finally {
    await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  }
}

test('contract entrypoints include the current funding methods', () => {
  assert.ok(ENTRYPOINTS.length > 0);
  assert.ok(ENTRYPOINTS.includes('fund_with_commitment'));
});

test('summary endpoint validates IDs, returns summaries, and limits CORS origins', async () => {
  const contractId = StrKey.encodeContract(Buffer.alloc(32));
  await withServer({
    summaryReader: async (id) => ({ status: 'open', id }),
    allowedOrigins: ['http://localhost:5500']
  }, async (baseUrl) => {
    const response = await fetch(`${baseUrl}/api/contracts/${contractId}/summary`, {
      headers: { Origin: 'http://localhost:5500' }
    });
    assert.equal(response.status, 200);
    assert.equal(response.headers.get('access-control-allow-origin'), 'http://localhost:5500');
    assert.deepEqual(await response.json(), {
      contractId,
      summary: { status: 'open', id: contractId }
    });

    const invalid = await fetch(`${baseUrl}/api/contracts/not-a-contract/summary`);
    assert.equal(invalid.status, 400);

    const invalidChecksumId = `${contractId.slice(0, -1)}${contractId.endsWith('A') ? 'B' : 'A'}`;
    const invalidChecksum = await fetch(`${baseUrl}/api/contracts/${invalidChecksumId}/summary`);
    assert.equal(invalidChecksum.status, 400);

    const deniedOrigin = await fetch(`${baseUrl}/api/health`, {
      headers: { Origin: 'https://attacker.example' }
    });
    assert.equal(deniedOrigin.headers.get('access-control-allow-origin'), null);
  });
});

test('summary endpoint returns 404 for unknown contracts', async () => {
  await withServer({
    summaryReader: async () => { throw new Error('HostError: MissingValue'); },
    allowedOrigins: []
  }, async (baseUrl) => {
    const contractId = StrKey.encodeContract(Buffer.alloc(32));
    const response = await fetch(`${baseUrl}/api/contracts/${contractId}/summary`);
    assert.equal(response.status, 404);
  });
});