import gql from 'graphql-tag';
import fetch from 'node-fetch'
import { createHttpLink } from 'apollo-link-http'
import { InMemoryCache } from 'apollo-cache-inmemory'
import ApolloClient from 'apollo-client'

// sk:    67df77adbc271f558df88504cf3a3f87bfdd2d55b11e86c55c4fd978a935a836
// addr:  10f3f6097052714c4a84a525e4975e7b9241a06e98

const API_URL = 'http://localhost:8000/graphql';
const client = new ApolloClient({
  link: createHttpLink({
    uri: API_URL,
    fetch: fetch,
  }),
  cache: new InMemoryCache(),
});

function makeid(length) {
  var result           = '';
  var characters       = 'abcdef0123456789';
  var charactersLength = characters.length;
  for ( var i = 0; i < length; i++ ) {
     result += characters.charAt(Math.floor(Math.random() * charactersLength));
  }
  return result;
}

function delay(ms: number) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

async function dex_call(method: string, args, pk?) {
  let args_str = JSON.stringify(args).replace(/\\([\s\S])|(")/g,"\\$1$2");
  if (method.startsWith('get')) {
    let q = `
    query {
      Readonly(inputReadonly: {
        contract: "0x230000000000000000000000000000000000000003",
        method: "${method}",
        args: ["${args_str}"],
      })
    }
  `
    let res = await client.query({ query: gql(q) });
    console.log(q, res);
    return JSON.parse(res.data.Readonly); 
  } else {
    if (pk === null) {
      throw new Error("pk can not be null in send tx");
    }
    let nonce = makeid(64);
    let q = `
    mutation {
      sendUnsafeCallTransaction(
        inputRaw: {
          chainId: "0xb6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036", 
          feeCycle: "0xff", 
          feeAssetId: "0x0000000000000000000000000000000000000000000000000000000000000000", 
          nonce: "${nonce}", 
          timeout: "0x4E20"
        }, 
        inputAction: {
          contract: "0x230000000000000000000000000000000000000003", 
          method: "${method}", 
          args: ["${args_str}"],
        }, 
        inputPrivkey: "${pk}"
      )
    }
  `
    let res = await client.mutate({ mutation: gql(q) });
    console.log(q, res);
    return res.data.sendUnsafeCallTransaction;
  }
}

const admin = '67df77adbc271f558df88504cf3a3f87bfdd2d55b11e86c55c4fd978a935a831';
const admin_addr = '1039971536a94b25bc06a39459a2299940e0f5215f';

const btc_holder = '67df77adbc271f558df88504cf3a3f87bfdd2d55b11e86c55c4fd978a935a830';
const btc_holder_addr = '10131f7e99d36a4b333a843f7f4b1e6e222903fca6';

const usdt_holder = '67df77adbc271f558df88504cf3a3f87bfdd2d55b11e86c55c4fd978a935a836';
const usdt_holder_addr = '10f3f6097052714c4a84a525e4975e7b9241a06e98';

const btc = '0x0000000000000000000000000000000000000000000000000000000000000001';
const usdt = '0x0000000000000000000000000000000000000000000000000000000000000002';

async function main() {
  // init
  await Promise.all([
    dex_call('update_fee_account', admin_addr , admin),
    dex_call('new_config', {fee_rate: 100000} , admin),
    dex_call('add_trading_pair', {symbol: 'BTC/USDT', base_asset: btc, quote_asset: usdt} , admin),
    dex_call('deposit', {asset_id: btc, amount: "2000000000000000000"}, btc_holder),
    dex_call('deposit', {asset_id: usdt, amount: "30000000000000000000000"}, usdt_holder),
  ])

  // order action
  await dex_call('place_order', {
    nonce: '1', 
    trading_pair_id: 0, 
    order_side: 'Sell', 
    price: '987600000000', 
    amount: '100000000',
    version: 0,
  }, btc_holder);

  await dex_call('place_order', {
    nonce: '1', 
    trading_pair_id: 0, 
    order_side: 'Buy', 
    price: '999900000000', 
    amount: '200000000',
    version: 0,
  }, usdt_holder);

  await dex_call('clear', "", admin);

  await delay(200);

  // check dashboard
  let dashboard = {}; 
  dashboard['trading_pairs'] = await dex_call('get_trading_pairs', "");
  dashboard['usdt_balance'] = await dex_call('get_balance', {user: usdt_holder_addr});
  dashboard['btc_balance'] = await dex_call('get_balance', {user: btc_holder_addr});
  dashboard['admin_balance'] = await dex_call('get_balance', {user: admin_addr});
  dashboard['btc_holder_orders'] = await dex_call('get_pending_orders', {version: 0, trading_pair_id: 0, user: btc_holder_addr});
  dashboard['usdt_holder_orders'] = await dex_call('get_pending_orders', {version: 0, trading_pair_id: 0, user: usdt_holder_addr});
  let orderbook = await dex_call('get_orderbook', {version: 0, trading_pair_id: 0});
  console.log(dashboard);
  console.log(orderbook);
}

main()
  .then(
    () => { console.log('---- exit ----') },
    (err) => { console.log(err) }
  )
  .then(() => { process.exit() })
