import { parse } from 'toml';
import { find } from 'lodash';
import { AssetService, MultiSignatureService } from '@mutadev/service'
import { readFileSync } from 'fs';
import * as sdk from '@mutadev/muta-sdk';
import { Muta } from "@mutadev/muta-sdk";

const genesis = parse(readFileSync('../../examples/genesis.toml', 'utf-8'));
const metadata = JSON.parse(
  find(genesis.services, (s) => s.name === 'metadata').payload,
);
const chain_id = metadata.chain_id;
const client_0 = get_client('../../examples/config-1.toml', chain_id);
const client_1 = get_client('../../examples/config-2.toml', chain_id);
const client_2 = get_client('../../examples/config-3.toml', chain_id);

describe("Byzantine test via @mutadev/muta-sdk-js", () => {
  test("getLatestBlock", async () => {
    const timeoutLoopTimes = process.env.TIMEOUT | 600;  // seconds
    var last_height = 0;
    var cnt = 0;
    for (var i = 0; i < timeoutLoopTimes; i++) {
      let height_0 = await client_0.getLatestBlockHeight();
      let height_1 = await client_1.getLatestBlockHeight();
      let height_2 = await client_2.getLatestBlockHeight();
      let max_height = Math.max(height_0, height_1, height_2);
      console.log(max_height);
      if (max_height > last_height) {
        last_height = max_height;
        cnt = 0;
      } else if (max_height == last_height) {
        cnt += 1;
        if (cnt > 600) {
          throw new Error('break liveness');
        }
      } else {
        throw new Error('break safety');
      }
      await sleep(1000);
    }
  });
});

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function get_client(file_path: string, chain_id: string) {
  const config = parse(readFileSync(file_path, 'utf-8'));
  const graphql_port = config.graphql.listening_address.split(':')[1];
  const muta = new Muta({
    endpoint: 'http://localhost:' + graphql_port + '/graphql',
    chainId: chain_id
  });
  return muta.client();
}
