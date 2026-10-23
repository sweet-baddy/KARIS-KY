const http = require('http');
const { StrKey } = require('@stellar/stellar-sdk');
const contractSpec = require('../sdk-ts/spec.json');

const HOST = '127.0.0.1';
const PORT = 8000;
const ENTRYPOINTS = contractSpec.entrypoints.map((entrypoint) => entrypoint.name);
const DEFAULT_ALLOWED_ORIGINS = ['http://127.0.0.1:5500', 'http://localhost:5500'];
const CONTRACT_ID_PATTERN = /^C[A-Z2-7]{55}$/;

function parseAllowedOrigins(value = process.env.ALLOWED_ORIGINS) {
  if (value === undefined) return DEFAULT_ALLOWED_ORIGINS;
  return value.split(',').map((origin) => origin.trim()).filter(Boolean);
}

async function fetchEscrowSummary(contractId) {
  const {
    Account,
    Contract,
    Keypair,
    rpc,
    TransactionBuilder,
    scValToNative
  } = require('@stellar/stellar-sdk');
  const rpcUrl = process.env.SOROBAN_RPC_URL || 'https://soroban-testnet.stellar.org';
  const networkPassphrase = process.env.SOROBAN_NETWORK_PASSPHRASE || 'Test SDF Network ; September 2015';
  const rpcServer = new rpc.Server(rpcUrl);
  const source = Keypair.random().publicKey();
  const transaction = new TransactionBuilder(new Account(source, '0'), {
    fee: '100',
    networkPassphrase
  })
    .addOperation(new Contract(contractId).call('get_escrow_summary'))
    .setTimeout(30)
    .build();
  const simulation = await rpcServer.simulateTransaction(transaction);

  if (simulation.error) throw new Error(simulation.error);
  if (!simulation.result?.retval) throw new Error('Soroban RPC returned no contract summary');
  return toJsonSafe(scValToNative(simulation.result.retval));
}

function toJsonSafe(value) {
  if (typeof value === 'bigint') return value.toString();
  if (value instanceof Map) {
    return Object.fromEntries([...value].map(([key, item]) => [String(key), toJsonSafe(item)]));
  }
  if (Array.isArray(value)) return value.map(toJsonSafe);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, toJsonSafe(item)]));
  }
  return value;
}

function isUnknownContractError(error) {
  return /contract.*(?:not found|does not exist|missing)|(?:not found|does not exist).*contract|missingvalue/i
    .test(error.message || String(error));
}

function createBackendServer({
  summaryReader = fetchEscrowSummary,
  allowedOrigins = parseAllowedOrigins()
} = {}) {
  return http.createServer(async (req, res) => {
    const origin = req.headers.origin;
    const headers = { 'Content-Type': 'application/json', Vary: 'Origin' };
    if (origin && allowedOrigins.includes(origin)) {
      headers['Access-Control-Allow-Origin'] = origin;
    }
    if (req.method === 'OPTIONS') {
      res.writeHead(204, {
        ...headers,
        'Access-Control-Allow-Methods': 'GET, OPTIONS'
      });
      res.end();
      return;
    }

    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'GET' && url.pathname === '/api/health') {
      res.writeHead(200, headers);
      res.end(JSON.stringify({
        status: 'ok',
        service: 'karis-ky-backend',
        contracts: ['escrow']
      }));
      return;
    }

    if (req.method === 'GET' && url.pathname === '/api/contract-summary') {
      res.writeHead(200, headers);
      res.end(JSON.stringify({
        name: 'karis-ky Escrow Contracts',
        description: 'Soroban smart contracts for invoice liquidity on Stellar.',
        entrypoints: ENTRYPOINTS
      }));
      return;
    }

    const summaryMatch = req.method === 'GET'
      ? url.pathname.match(/^\/api\/contracts\/([^/]+)\/summary$/)
      : null;
    if (summaryMatch) {
      const contractId = summaryMatch[1];
      if (!CONTRACT_ID_PATTERN.test(contractId) || !StrKey.isValidContract(contractId)) {
        res.writeHead(400, headers);
        res.end(JSON.stringify({ error: 'invalid contract ID' }));
        return;
      }
      try {
        const summary = await summaryReader(contractId);
        res.writeHead(200, headers);
        res.end(JSON.stringify({ contractId, summary }));
      } catch (error) {
        const status = isUnknownContractError(error) ? 404 : 502;
        res.writeHead(status, headers);
        res.end(JSON.stringify({ error: status === 404 ? 'contract not found' : 'Soroban RPC request failed' }));
      }
      return;
    }

    res.writeHead(404, headers);
    res.end(JSON.stringify({ error: 'not found' }));
  });
}

const server = createBackendServer();

if (require.main === module) {
  server.listen(PORT, HOST, () => {
    console.log(`Backend listening on http://${HOST}:${PORT}`);
  });
}

module.exports = { ENTRYPOINTS, createBackendServer, parseAllowedOrigins, server };
