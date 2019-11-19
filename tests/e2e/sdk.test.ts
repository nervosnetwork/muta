import { muta, CHAIN_CONFIG, delay } from "./utils";

describe("API test via muta-sdk-js", () => {
  test("getLatestEpoch", async () => {
    let current_height = await muta.client.getEpochHeight();
    // console.log(current_height);
    expect(current_height).toBeGreaterThan(0);
  });

  test("transfer work", async () => {
    const from_addr = "0x10f8389d774afdad8755ef8e629e5a154fddc6325a";
    const from_pk =
      "0x45c56be699dca666191ad3446897e0f480da234da896270202514a0e1a587c3f";
    const to_addr = "0x100000000000000000000000000000000000000000";
    const asset_id =
      "0xfee0decb4f6a76d402f200b5642a9236ba455c22aa80ef82d69fc70ea5ba20b5";
    const from_balance_before = await muta.client.getBalance(
      from_addr,
      asset_id
    );
    const to_balance_before = await muta.client.getBalance(to_addr, asset_id);
    const height_before = await muta.client.getEpochHeight();

    // transfer
    const account = muta.accountFromPrivateKey(from_pk);
    // console.log(account.address, from_addr);
    expect(account.address).toBe(from_addr);
    const tx = await muta.client.prepareTransferTransaction({
      carryingAmount: "0x01",
      carryingAssetId: asset_id,
      receiver: to_addr
    });
    const signedTx = account.signTransaction(tx);
    await muta.client.sendTransferTransaction(signedTx);

    // check result
    const retry_times = 3;
    let i: number;
    for (i = 0; i < retry_times; i++) {
      // wait at least 2 blocks. Change to confirm after impl
      await delay(CHAIN_CONFIG.consensus.interval * 2 + 100);
      let height_after = await muta.client.getEpochHeight();
      if (height_after <= height_before) {
        continue;
      }
      let from_balance_after = await muta.client.getBalance(
        from_addr,
        asset_id
      );
      let to_balance_after = await muta.client.getBalance(to_addr, asset_id);
      expect(from_balance_after).toBe(from_balance_before - 1);
      expect(to_balance_after).toBe(to_balance_before + 1);
      break;
    }
    expect(i).toBeLessThan(retry_times);
  });
});
