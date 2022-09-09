'use strict';
const nearAPI = require('near-api-js');
const BN = require('bn.js');
const fs = require('fs').promises;
const assert = require('assert').strict;

const config = {
  networkId: 'sandbox',
  nodeUrl: 'http://0.0.0.0:3030',
  keyPath: '/tmp/near-sandbox/validator_key.json',
  contractPath: './target/wasm32-unknown-unknown/release/tether_token.wasm',
  accountId: 'test.near',
  contractId: 'test.near',
};

const methods = {
  viewMethods: ['version'],
};

(async function () {
  const keyFile = require(config.keyPath);
  const privKey = nearAPI.utils.KeyPair.fromString(keyFile.secret_key);

  const keyStore = new nearAPI.keyStores.InMemoryKeyStore();
  keyStore.setKey(config.networkId, config.accountId, privKey);

  const near = await nearAPI.connect({
    deps: {
      keyStore,
    },
    networkId: config.networkId,
    nodeUrl: config.nodeUrl,
  });

  const wasm = await fs.readFile(config.contractPath);
  const account = new nearAPI.Account(near.connection, config.accountId);

  // Upgrade the contract.
  await account.signAndSendTransaction({
    receiverId: config.contractId,
    actions: [
      nearAPI.transactions.functionCall('upgrade', wasm, 100000000000000, '0'),
    ],
  });

  // Check that the contract has been upgraded.
  // Change the `version` method returning 'UPGRADED:VERSION' to test this.
  const contract = new nearAPI.Contract(account, config.contractId, methods);
  assert.equal(await contract.version(), 'UPGRADED:VERSION');
})();
