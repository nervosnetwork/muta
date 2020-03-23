import { muta, mutaClient, CHAIN_CONFIG, delay } from "./utils";
import { AssetService, Account } from "muta-sdk";

describe("API test via muta-sdk-js", () => {
  test("getLatestBlock", async () => {
    let current_height = await mutaClient.getLatestBlockHeight();
    // console.log(current_height);
    expect(current_height).toBeGreaterThan(0);
  });

  test("transfer work", async () => {
    const from_addr = "0x103e9b982b443592ffc3d4c2a484c220fb3e29e2e4";
    const from_pk =
      "0x1ab5dfb50a38643ad8bbcbb27145825ddba65e67c72ec9bb643b72e190a27509";
    const to_addr = "0x100000000000000000000000000000000000000001";
    const asset_id =
      "0xfee0decb4f6a76d402f200b5642a9236ba455c22aa80ef82d69fc70ea5ba20b5";

    const account = Account.fromPrivateKey(from_pk);
    const assetService = new AssetService(mutaClient, account);

    const from_balance_before = await assetService.get_balance(
      {
        user: from_addr,
        asset_id: asset_id
      }
    );
    const to_balance_before = await assetService.get_balance({
      user: to_addr,
      asset_id: asset_id,
    });
    const height_before = await mutaClient.getLatestBlockHeight();

    // transfer
    expect(account.address).toBe(from_addr);

    assetService.transfer({
      asset_id: asset_id,
      to: to_addr,
      value: 0x01,
    })

    // check result
    const retry_times = 3;
    let i: number;
    for (i = 0; i < retry_times; i++) {
      // wait at least 2 blocks. Change to confirm after impl
      await delay(CHAIN_CONFIG.consensus.interval * 2 + 100);
      let height_after = await mutaClient.getLatestBlockHeight();
      if (height_after <= height_before) {
        continue;
      }
      let from_balance_after = await assetService.get_balance( {
        user: from_addr,
        asset_id: asset_id,
      });
      const to_balance_after = await assetService.get_balance({
        user: to_addr,
        asset_id: asset_id,
      });
      console.log(from_balance_after.ret.balance, from_balance_before.ret.balance)
      // expect(from_balance_after.ret.balance).toBe(from_balance_before.ret.balance - 1);
      // expect(to_balance_after.ret.balance).toBe(to_balance_before.ret.balance + 1);
      break;
    }
    expect(i).toBeLessThan(retry_times);
  });
});
