/**
 * run this demo with below steps:
 *
 * ```
 * $ cargo run --example muta-chain
 *
 * # in another terminal
 * $ cd built-in-services/riscv/examples/dex
 * $ yarn install
 * $ yarn run ts-node riscv_demo.ts
 * ```
 */
// run this demo with below steps:
// 1.
// yarn run ts-node riscv_demo.ts
import { Muta } from "muta-sdk";
import { readFileSync } from "fs";

const muta = new Muta({
    endpoint: "http://127.0.0.1:8000/graphql",
    chainId: "0xb6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036"
});

const client = muta.client;
const account = muta.accountFromPrivateKey("0x10000000000000000000000000000000000000000000000000000000000000000");

const erc20 = readFileSync("./erc20.js");
const dex = readFileSync("./dex.js");

async function deploy(code, init_args) {
    const tx = await client.prepareTransaction({
        method: 'deploy',
        payload: {
            intp_type: 'Duktape',
            init_args,
            code: code.toString('hex'),
        },
        serviceName: 'riscv'
    });
    // console.log(tx);
    tx.cyclesLimit = '0x99999999';
    tx.cyclesPrice = '0x1';
    const tx_hash = await client.sendTransaction(account.signTransaction(tx));
    // console.log(tx_hash);

    const receipt = await client.getReceipt(tx_hash);
    console.log('deploy:', {tx_hash, receipt});

    const addr = JSON.parse(receipt).address;
    return addr;
}

async function query(address, args) {
    const res = await client.queryService({
        caller: account.address,
        method: 'call',
        payload: {
            address: address,
            args: JSON.stringify(args),
        },
        serviceName: 'riscv',
        cyclesLimit: '0x99999999',
        cyclesPrice: '0x1',
    });
    console.log('query:', {address, args, res});
    return res;
}

async function send_tx(address, args) {
    const payload = {
        address,
        args: JSON.stringify(args),
    }
    const exec_tx = await client.prepareTransaction({
        payload,
        serviceName: 'riscv',
        method: 'exec',
    });
    exec_tx.cyclesLimit = '0x99999999';
    exec_tx.cyclesPrice = '0x1';
    const tx_hash = await client.sendTransaction(account.signTransaction(exec_tx));
    // console.log(tx_hash);

    const exec_receipt = await client.getReceipt(tx_hash);
    console.log('send_tx:', {exec_receipt, address, args});
    return exec_receipt;
}

async function assert(condition, msg = 'assert failed!') {
    if (!condition) {
        throw new Error(msg);
    }
}

async function main() {
    const erc20_addr = await deploy(erc20, JSON.stringify({
        method: 'init',
        name: 'bitcoin',
        symbol: 'BTC',
        supply: 1000000000,
    }));
    const dex_addr = await deploy(dex, '');

    let res;
    let from_addr = account.address.slice(2);
    let to_addr = '0000000000000000000000000000000000000000';
    res = await query(erc20_addr, {method: 'total_supply'});
    assert(!res.isError);
    assert(res.ret == '1000000000');
    res = await query(erc20_addr, {method: 'balance_of', account: from_addr});
    assert(!res.isError);
    assert(res.ret == '1000000000');
    res = await send_tx(erc20_addr, {method: 'transfer', recipient: to_addr, amount: 100});
    assert(!res.includes('[ProtocolError]'));
    res = await query(erc20_addr, {method: 'balance_of', account: from_addr});
    assert(!res.isError);
    assert(res.ret == '999999900');
    res = await query(erc20_addr, {method: 'balance_of', account: to_addr});
    assert(!res.isError);
    assert(res.ret == '100');

    res = await query(erc20_addr, {method: 'allowances', owner: from_addr, spender: dex_addr});
    assert(!res.isError);
    assert(res.ret == '0');
    res = await send_tx(erc20_addr, {method: 'approve', spender: dex_addr, amount: 300});
    assert(!res.includes('[ProtocolError]'));
    res = await query(erc20_addr, {method: 'allowances', owner: from_addr, spender: dex_addr});
    assert(!res.isError);
    assert(res.ret == '300');
    res = await send_tx(dex_addr, {method: 'deposit', asset: erc20_addr, amount: 200});
    assert(!res.includes('[ProtocolError]'));
    res = await query(erc20_addr, {method: 'allowances', owner: from_addr, spender: dex_addr});
    assert(!res.isError);
    assert(res.ret == '100');
    res = await query(dex_addr, {method: 'balance_of', account: from_addr, asset: erc20_addr});
    assert(!res.isError);
    assert(res.ret == '200');
    res = await query(erc20_addr, {method: 'balance_of'});
    assert(!res.isError);
    assert(res.ret == '999999700');
    res = await send_tx(dex_addr, {method: 'withdraw', asset: erc20_addr, amount: 1});
    assert(!res.includes('[ProtocolError]'));
    res = await query(dex_addr, {method: 'balance_of', asset: erc20_addr});
    assert(!res.isError);
    assert(res.ret == '199');
    res = await query(erc20_addr, {method: 'balance_of'});
    assert(!res.isError);
    assert(res.ret == '999999701');
    res = await send_tx(dex_addr, {method: 'withdraw', asset: erc20_addr, amount: 200});
    assert(res.includes('[ProtocolError]'));
    res = await query(dex_addr, {method: 'balance_of', asset: erc20_addr});
    assert(!res.isError);
    assert(res.ret == '199');
}

main();
