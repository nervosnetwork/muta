import { muta, mutaClient, delay } from "./utils";
import { AssetService, Account } from "muta-sdk";

describe("API test via muta-sdk-js", () => {
  test("getLatestBlock", async () => {
    let current_height = await mutaClient.getLatestBlockHeight();
    // console.log(current_height);
    expect(current_height).toBeGreaterThan(0);
  });

  test("transfer work", async () => {
    const from_addr = "0xf8389d774afdad8755ef8e629e5a154fddc6325a";
    const from_pk =
      "0x45c56be699dca666191ad3446897e0f480da234da896270202514a0e1a587c3f";
    const to_addr = "0x0000000000000000000000000000000000000001";
    const asset_id =
      "0xf56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c";

    const account = Account.fromPrivateKey(from_pk);
    const assetService = new AssetService(mutaClient, account);

    const from_balance_before = await assetService.get_balance({
      user: from_addr,
      asset_id: asset_id
    })!;
    const to_balance_before = await assetService.get_balance({
      user: to_addr,
      asset_id: asset_id,
    })!;
    const height_before = await mutaClient.getLatestBlockHeight();

    // transfer
    expect(account.address).toBe(from_addr);

    assetService.transfer({
      asset_id: asset_id,
      to: to_addr,
      value: 0x01,
    })

    // check result
    const retry_times = 5;
    let i: number;
    for (i = 0; i < retry_times; i++) {
      // wait at least 2 blocks. Change to confirm after impl
      await delay(3000 * 2 + 100);
      let height_after = await mutaClient.getLatestBlockHeight();
      if (height_after <= height_before) {
        continue;
      }
      let from_balance_after = await assetService.get_balance({
        user: from_addr,
        asset_id: asset_id,
      })!;
      const to_balance_after = await assetService.get_balance({
        user: to_addr,
        asset_id: asset_id,
      })!;

      const c1 = from_balance_before.succeedData.balance as number;
      expect(from_balance_after.succeedData.balance).toBe(c1 - 1);
      const c2 = to_balance_before.succeedData.balance as number
      expect(to_balance_after.succeedData.balance).toBe(c2 + 1);
      break;
    }
    expect(i).toBeLessThan(retry_times);
  });
});
