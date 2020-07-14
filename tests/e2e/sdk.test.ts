import { Account } from '@mutadev/account';
import { AssetService } from '@mutadev/service'
import { toHex } from '@mutadev/utils';
import { retry } from '@mutadev/client';
import * as sdk from '@mutadev/muta-sdk';
import { mutaClient } from './utils';
import { MultiSigService } from './multisig';

describe("API test via @mutadev/muta-sdk-js", () => {
  test("getLatestBlock", async () => {
    let current_height = await mutaClient.getLatestBlockHeight();
    expect(current_height).toBeGreaterThan(0);
  });

  test("getNoneBlock", async () => {
    let block = await mutaClient.getBlock("0xffffffff");
    expect(block).toBe(null);
  })

  test("getNoneTransaction", async () => {
    let tx = await mutaClient.getTransaction("0xf56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c");
    expect(tx).toBe(null);
  })

  test("getNoneReceipt", async () => {
    let receipt = await mutaClient.getReceipt("0xf56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c");
    expect(receipt).toBe(null);
  })

  test("transfer work", async () => {
    const from_addr = "0xf8389d774afdad8755ef8e629e5a154fddc6325a";
    const from_pk =
      "0x45c56be699dca666191ad3446897e0f480da234da896270202514a0e1a587c3f";
    const to_addr = "0x0000000000000000000000000000000000000001";
    const asset_id =
      "0xf56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c";

    const account = sdk.Muta.accountFromPrivateKey(from_pk);
    const assetService = new AssetService(mutaClient, account);

    const from_balance_before = await assetService.read.get_balance({
      user: from_addr,
      asset_id: asset_id
    })!;
    const to_balance_before = await assetService.read.get_balance({
      user: to_addr,
      asset_id: asset_id,
    })!;

    // transfer
    expect(account.address).toBe(from_addr);

    await assetService.write.transfer({
      asset_id: asset_id,
      to: to_addr,
      value: 0x01,
    })

    // check result
    let from_balance_after = await assetService.read.get_balance({
      user: from_addr,
      asset_id: asset_id,
    })!;
    const to_balance_after = await assetService.read.get_balance({
      user: to_addr,
      asset_id: asset_id,
    })!;

    const c1 = from_balance_before.succeedData.balance as number;
    expect(from_balance_after.succeedData.balance).toBe(c1 - 1);
    const c2 = to_balance_before.succeedData.balance as number;
    expect(to_balance_after.succeedData.balance).toBe(c2 + 1);
  });

  test('multisig', async () => {
    const wangYe = Account.fromPrivateKey(
      '0x1000000000000000000000000000000000000000000000000000000000000000',
    );
    const qing = Account.fromPrivateKey(
      '0x2000000000000000000000000000000000000000000000000000000000000000',
    );

    const multiSigService = new MultiSigService(mutaClient, wangYe);

    var GenerateMultiSigAccountPayload = {
      owner: wangYe.address,
      addr_with_weight: [{ address: wangYe.address, weight: 1 }, { address: qing.address, weight: 1 }],
      threshold: 2,
      memo: 'welcome to BiYouCun'
    };
    const generated = await multiSigService.write.generate_account(GenerateMultiSigAccountPayload);
    expect(Number(generated.response.response.code)).toBe(0);

    const multiSigAddress = generated.response.response.succeedData.address;
    const createAssetTx = await mutaClient.composeTransaction({
      method: 'create_asset',
      payload: {
        name:      'miao',
        supply:    2077,
        symbol:    '😺',
      },
      serviceName: 'asset',
      sender: multiSigAddress,
    });

    const signedCreateAssetTx = wangYe.signTransaction(createAssetTx);
    try {
      await mutaClient.sendTransaction(signedCreateAssetTx);
      throw 'should failed';
    } catch(e) {
      expect(String(e)).toContain('CheckAuthorization');
    }

    const bothSignedCreateAssetTx = qing.signTransaction(signedCreateAssetTx);
    const txHash = await mutaClient.sendTransaction(bothSignedCreateAssetTx);
    const receipt = await retry(() => mutaClient.getReceipt(toHex(txHash)));
    expect(Number(receipt.response.response.code)).toBe(0);

    // MultiSig address balance
    const asset = JSON.parse(receipt.response.response.succeedData as string);
    const assetService = new AssetService(mutaClient, wangYe);
    const balance = await assetService.read.get_balance({
        asset_id: asset.id,
        user: multiSigAddress,
    });

    expect(Number(balance.code)).toBe(0);
    expect(Number(balance.succeedData.balance)).toBe(2077);

    const newMultiSigAccountPayload = {
      owner: wangYe.address,
      addr_with_weight: [{ address: wangYe.address, weight: 3 }, { address: qing.address, weight: 1 }],
      threshold: 4,
      memo: 'welcome to BiYouCun'
    };

    const updateAccountPayload = {
      account_address: multiSigAddress,
      new_account_info: newMultiSigAccountPayload,
    };

    const update = await multiSigService.write.update_account(updateAccountPayload);
    expect(Number(update.response.response.code)).toBe(0);

    const fei = Account.fromPrivateKey(
      '0x3000000000000000000000000000000000000000000000000000000000000000',
    );

    var GenerateMultiSigAccountPayload = {
      owner: wangYe.address,
      addr_with_weight: [{ address: multiSigAddress, weight: 2 }, { address: fei.address, weight: 1 }],
      threshold: 2,
      memo: 'welcome to CiYouCun'
    };
    const newGenerate = await multiSigService.write.generate_account(GenerateMultiSigAccountPayload);
    expect(Number(newGenerate.response.response.code)).toBe(0);

    const newMultiSigAddress = newGenerate.response.response.succeedData.address;
    const newAssetTx = await mutaClient.composeTransaction({
      method: 'create_asset',
      payload: {
        name: 'miaomiao',
        supply: 2078,
        symbol: '😺😺',
      },
      serviceName: 'asset',
      sender: newMultiSigAddress,
    });

    const newSignedCreateAssetTx = wangYe.signTransaction(newAssetTx);
    const newBothCreateAssetTx = qing.signTransaction(newSignedCreateAssetTx);
    const newTxHash = await mutaClient.sendTransaction(newBothCreateAssetTx);
    const newReceipt = await retry(() => mutaClient.getReceipt(toHex(newTxHash)));
    expect(Number(newReceipt.response.response.code)).toBe(0);

    const newAsset = JSON.parse(newReceipt.response.response.succeedData as string);
    const newAssetService = new AssetService(mutaClient, wangYe);
    const newBalance = await newAssetService.read.get_balance({
      asset_id: newAsset.id,
      user: newMultiSigAddress,
    });

    expect(Number(newBalance.code)).toBe(0);
    expect(Number(newBalance.succeedData.balance)).toBe(2078);
  });
});
