/**
 * run this demo with below steps:
 *
 * ```
 * $ cargo run --example muta-chain
 *
 * # in another terminal
 * $ cd built-in-services/riscv/examples/dex
 * $ yarn install
 * $ node riscv_demo.js
 * ```
 */

const muta_sdk = require('muta-sdk');
const Muta = muta_sdk.Muta;
const fs = require('fs');

const muta = Muta.createDefaultMutaInstance();
const client = muta.client();
client.options.defaultCyclesLimit = '0xffffff';
const account = Muta.accountFromPrivateKey("0x10000000000000000000000000000000000000000000000000000000000000000");

const erc20 = fs.readFileSync("./erc20.bin");
const dex = fs.readFileSync("./dex.bin");
// const erc20 = fs.readFileSync("./erc20.js");
// const dex = fs.readFileSync("./dex.js");

async function deploy(code, init_args) {
    const tx = await client.composeTransaction({
        method: 'deploy',
        payload: {
            // intp_type: 'Duktape',
            intp_type: 'Binary',
            init_args,
            code: code.toString('hex'),
        },
        serviceName: 'riscv'
    });
    console.log(tx);
    const tx_hash = await client.sendTransaction(account.signTransaction(tx));
    console.log(tx_hash);

    const receipt = await client.getReceipt(tx_hash);
    console.log('deploy:', {tx_hash, receipt});

    const addr = JSON.parse(receipt.response.ret).address;
    return addr;
}

async function query(address, args) {
    const res = await client.queryService({
        serviceName: 'riscv',
        method: 'call',
        payload: JSON.stringify({
            address: address,
            args: JSON.stringify(args),
        }),
    });
    console.log('query:', {address, args, res});
    res.ret = JSON.parse(res.ret);
    return res;
}

async function send_tx(address, args) {
    const payload = {
        address,
        args: JSON.stringify(args),
    }
    const exec_tx = await client.composeTransaction({
        payload,
        serviceName: 'riscv',
        method: 'exec',
    });
    console.log('send_tx:', {address, args, exec_tx});
    const tx_hash = await client.sendTransaction(account.signTransaction(exec_tx));
    console.log('tx_hash:', tx_hash);

    const exec_receipt = await client.getReceipt(tx_hash);
    console.log('send_tx:', {exec_receipt, address, args});
    return exec_receipt;
}

function assert(condition, msg = 'assert failed!') {
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
    assert(!res.response.ret.includes('[ProtocolError]'));
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
    assert(!res.response.ret.includes('[ProtocolError]'));
    res = await query(erc20_addr, {method: 'allowances', owner: from_addr, spender: dex_addr});
    assert(!res.isError);
    assert(res.ret == '300');
    res = await send_tx(dex_addr, {method: 'deposit', asset: erc20_addr, amount: 200});
    assert(!res.response.ret.includes('[ProtocolError]'));
    res = await query(erc20_addr, {method: 'allowances', owner: from_addr, spender: dex_addr});
    assert(!res.isError);
    assert(res.ret == '100');
    res = await query(dex_addr, {method: 'balance_of', account: from_addr, asset: erc20_addr});
    assert(!res.isError);
    assert(res.ret == '200');
    res = await query(erc20_addr, {method: 'balance_of', account: from_addr});
    assert(!res.isError);
    assert(res.ret == '999999700');
    res = await send_tx(dex_addr, {method: 'withdraw', asset: erc20_addr, amount: 1});
    assert(!res.response.ret.includes('[ProtocolError]'));
    res = await query(dex_addr, {method: 'balance_of', asset: erc20_addr, account: from_addr});
    assert(!res.isError);
    assert(res.ret == '199');
    res = await query(erc20_addr, {method: 'balance_of', account: from_addr});
    assert(!res.isError);
    assert(res.ret == '999999701');
    res = await send_tx(dex_addr, {method: 'withdraw', asset: erc20_addr, amount: 200});
    assert(res.response.ret.includes('[ProtocolError]'));
    res = await query(dex_addr, {method: 'balance_of', asset: erc20_addr, account: from_addr});
    assert(!res.isError);
    assert(res.ret == '199');
}

main();
