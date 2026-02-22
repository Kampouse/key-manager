const fetch = require('node-fetch');

async function main() {
  const txHash = '3geooukGUg1NBMYeMx7Rp1F5bsQHqUEEU1S2wR9bNfd4';
  
  const response = await fetch('https://rpc.mainnet.near.org', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'tx',
      params: [txHash, 'kampouse.near'],
    }),
  });
  
  const data = await response.json();
  
  // Look for logs in receipts
  for (const receipt of data.result.receipts_outcome) {
    for (const log of receipt.outcome.logs) {
      if (log.includes('Output: Json')) {
        console.log('FULL LOG:');
        console.log(log);
        console.log('\n---\n');
      }
    }
  }
}

main().catch(console.error);
