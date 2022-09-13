'use strict';
const nearAPI = require('near-api-js');
const { Contract } = nearAPI;
const BN = require('bn.js');
const fs = require('fs').promises;
const assert = require('assert').strict;

//const config = {
//  networkId: 'testnet',
//  nodeUrl: 'https://rpc.testnet.near.org',
//  keyPath: '/home/user/.near-credentials/testnet/user.testnet.json',
//  contractPath: './res/tether_token.wasm',
//  accountId: 'user.testnet',
//  contractId: 'usdt.token',
//  msafeId: 'vault.multisafe',
//};

const config = {
  networkId: 'mainnet',
  nodeUrl: 'https://rpc.mainnet.near.org',
  keyPath: '/home/user/.near-credentials/mainnet/user.json',
  contractPath: './res/tether_token.wasm',
  accountId: 'user',
  contractId: 'usdt.tether-token.near',
  msafeId: 'tether.multisafe.near',
};

(async function () {
  const keyFile = require(config.keyPath);
  const privKey = nearAPI.utils.KeyPair.fromString(keyFile.private_key);

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
  const contract = new Contract(
    account,
    config.msafeId,
    {
      changeMethods: ['add_request_and_confirm'],
      sender: account,
    }
  );

  // Send Upgrade contract to multisafe.
  await contract.add_request_and_confirm({
    request: {
      receiver_id: config.contractId,
      actions: [
        {
          type: 'FunctionCall',
          method_name: 'upgrade',
          args: wasm.toString('base64'),
          deposit:'0',
          gas: '250000000000000',
        },
      ],
    }
  },
  300000000000000
  );

})();
