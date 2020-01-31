import gql from "graphql-tag";
import { client, getNonce, delay, CHAIN_CONFIG, CHAIN_ID } from "./utils";

describe("query API works", () => {
  test("getLatestBlock works", async () => {
    let q = `
        query {
            getLatestBlock {
                header {
                    height
                }
            }
        }
        `;
    let res = await client.query({ query: gql(q) });
    expect(typeof res.data.getLatestBlock.header.height).toBe("string");
  });

  test("getLatestBlock with height works", async () => {
    let q = `
        query {
            getLatestBlock(height: "0x0") {
                header {
                    height
                }
            }
        }
        `;
    let res = await client.query({ query: gql(q) });
    expect(res.data.getLatestBlock.header.height).toBe("0000000000000000");
  });
});

describe("transfer work", () => {
  test("transfer work", async () => {
    const from_addr = "10f8389d774afdad8755ef8e629e5a154fddc6325a";
    const from_pk =
      "0x45c56be699dca666191ad3446897e0f480da234da896270202514a0e1a587c3f";
    const to_addr = "100000000000000000000000000000000000000000";
    const asset_id =
      "fee0decb4f6a76d402f200b5642a9236ba455c22aa80ef82d69fc70ea5ba20b5";
    const q_balance = `
          query {
            height: getLatestBlock {
                header {
                  height
                }
              }
            from: getBalance(
              address: "${from_addr}", 
              id: "${asset_id}"
            ),
            to: getBalance(
              address: "${to_addr}", 
              id: "${asset_id}"
            ),
          }
        `;
    let res = await client.query({ query: gql(q_balance) });
    const from_balance_before = parseInt(res.data.from, 16);
    const to_balance_before = parseInt(res.data.to, 16);
    const current_height_before = parseInt(res.data.height.header.height, 16);

    // transfer
    let q_transfer = `
mutation {
  sendUnsafeTransferTransaction(
    inputRaw: {
      chainId: "${CHAIN_ID}", 
      feeCycle: "0xff", 
      feeAssetId: "0x0000000000000000000000000000000000000000000000000000000000000000", 
      nonce: "${getNonce()}", 
      timeout: "${(current_height_before + 100).toString(16)}"
    }, 
    inputAction: {
      carryingAmount: "0x01", 
      carryingAssetId: "fee0decb4f6a76d402f200b5642a9236ba455c22aa80ef82d69fc70ea5ba20b5", 
      receiver: "100000000000000000000000000000000000000000"
    }, 
    inputPrivkey: "${from_pk}")
}
        `;
    await client.mutate({ mutation: gql(q_transfer) });

    // check result
    const retry_times = 3;
    let i;
    for (i = 0; i < retry_times; i++) {
      // wait at least 2 blocks. Change to confirm after impl
      await delay(CHAIN_CONFIG.consensus.interval * 2 + 100);
      res = await client.query({ query: gql(q_balance) });
      // console.log(Date.now(), res, res.data.height);
      const current_height_after = parseInt(res.data.height.header.height, 16);
      const from_balance_after = parseInt(res.data.from, 16);
      const to_balance_after = parseInt(res.data.to, 16);
      if (current_height_after <= current_height_before) {
        continue;
      }
      expect(from_balance_after).toBe(from_balance_before - 1);
      expect(to_balance_after).toBe(to_balance_before + 1);
      break;
    }
    expect(i).toBeLessThan(retry_times);
  });
});
