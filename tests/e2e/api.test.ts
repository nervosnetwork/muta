import gql from "graphql-tag";
import { client, getNonce, delay, CHAIN_CONFIG } from "./utils";

describe("query API works", () => {
  test("getLatestEpoch works", async () => {
    let q = `
        query {
            getLatestEpoch {
                header {
                    epochId
                }
            }
        }
        `;
    let res = await client.query({ query: gql(q) });
    expect(typeof res.data.getLatestEpoch.header.epochId).toBe("string");
  });

  test("getLatestEpoch with epochId works", async () => {
    let q = `
        query {
            getLatestEpoch(epochId: "0x0") {
                header {
                    epochId
                }
            }
        }
        `;
    let res = await client.query({ query: gql(q) });
    expect(res.data.getLatestEpoch.header.epochId).toBe("0000000000000000");
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
            height: getLatestEpoch {
                header {
                  epochId
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
    const current_height_before = parseInt(res.data.height.header.epochId, 16);

    // transfer
    let q_transfer = `
mutation {
  sendUnsafeTransferTransaction(
    inputRaw: {
      chainId: "0xb6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036", 
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
      const current_height_after = parseInt(res.data.height.header.epochId, 16);
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
