local pb = require('pb')
local protoc = require('protoc')
local hex = require('hex')
local json = require('json')
local http = require('socket.http')
local serpent = require('serpent')
local socket = require('socket')

local consts = require("consts")

-- load schema from text
assert(
    protoc:load [[
  syntax = "proto3";
  enum Crypto {
      DEFAULT = 0;
      RESERVED = 1;
  }
  message Transaction {
      string to = 1;
      string nonce = 2;
      uint64 quota = 3;
      uint64 valid_until_block = 4;
      bytes data = 5;
      bytes value = 6;
      uint32 chain_id = 7;
      uint32 version = 8;
      bytes to_v1 = 9;
      bytes chain_id_v1 = 10;
  }
  message UnverifiedTransaction {
      Transaction transaction = 1;
      bytes signature = 2;
      Crypto crypto = 3;
  }
]]
)


local to_addr = '0000000000000000000000000000000000000001'
local from_addr = '0x7899EE7319601cbC2684709e0eC3A4807bb0Fd74'
local privkey = '0x028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa80'


-- {{method1, params1}, {method2, params2}}
jsonrpc_call = function(params_list)
    local request_body = {}
    for k, v in pairs(params_list) do
        request_body[k] = {
            id = k,
            jsonrpc = '2.0',
            method = v[1],
            params = v[2],
        }
    end
    local request_body = json.encode(request_body)
    -- print('<--', request_body)
    local response_body = {}
    http.request {
        url = string.format('%s://%s:%s/', wrk.schema, wrk.host, wrk.port),
        method = 'POST',
        headers = {
            ['Content-Type'] = 'application/json',
            ['Content-Length'] = #request_body
        },
        source = ltn12.source.string(request_body),
        sink = ltn12.sink.table(response_body)
    }
    local result = response_body[1]
    -- print('-->', result)
    local result_table = json.decode(result)
    local res = {}
    for k, v in pairs(result_table) do
        res[k] = v['result']
    end
    return res
end

get_block_number_and_balance = function()
    -- get erc20 balance
    local res = jsonrpc_call({
        {
            'blockNumber',
            {},
        },
        {
            'getBalance',
            { to_addr },
        },
    })
    -- print('balance: ', serpent.block(res))
    return {
        block_number=res[1],
        balance=res[2],
    }
end

wrk.method = 'POST'
wrk.headers['Content-Type'] = 'application/json'


local counter = 1

function setup(thread)
    thread:set("id", counter)
    counter = counter + 1
    if init_state == nil then
        latest_block = tonumber(jsonrpc_call({{"blockNumber", {}}})[1])
        valid_until_block = latest_block + 9999
        print('valid_until_block:', valid_until_block)
        init_state = get_block_number_and_balance()
        print('init_state', serpent.block(init_state))
    end
    thread:set('contract_addr', contract_addr)
    thread:set('init_state', init_state)
    thread:set('valid_until_block', valid_until_block)
 end

 function init(args)
    balance = tonumber(init_state['balance'])
 end

request = function()
    local nonce = string.format('%s-%d-%s', id, balance, contract_addr)
    balance = balance + 1
    -- print(string.format("thread: %d, nonce: %s", id, nonce))
    local tx = {
        to = to_addr,
        nonce = nonce,
        quota = 30000,
        valid_until_block = valid_until_block,
        value = hex.decode('0000000000000000000000000000000000000000000000000000000000000001'),
    }
    -- print(serpent.block(tx))

    local bytes = pb.encode('Transaction', tx)

    -- send unsafe tx
    local txdata = hex.encode(bytes)
    local body = {
        id = 1,
        jsonrpc = '2.0',
        method = 'sendUnsafeTransaction',
        params = {txdata, privkey}
    }

    body_str = json.encode(body)
    -- print(body_str)
    return wrk.format('POST', '/', nil, body_str)
end


response = function(status, header, body)
    -- print(body)
end


done = function(summary, latency, requests)
    -- print('------------------------------')
    -- print(serpent.block(summary))
    -- print(string.format("init state: %s", serpent.block(init_state)))
    local final_state = get_block_number_and_balance(contract_addr)
    -- print(string.format("final state: %s", serpent.block(final_state)))
    -- duration as s
    local duration = summary['duration'] / 1000000
    print(
        string.format(
            [[
------------------------------------------------------
| state |              balance |         block height |
| init  | %20s | %20s |
| final | %20s | %20s |
------------------------------------------------------
duration                    : %.3fs
average tx num in a block   : %.0f
average block time          : %.3fs
average tx num in a second  : %.0f
   ]],
            tonumber(init_state['balance']),
            init_state['block_number'],
            tonumber(final_state['balance']),
            final_state['block_number'],
            duration,
            (tonumber(final_state['balance']) - tonumber(init_state['balance'])) /
                (tonumber(final_state['block_number']) - tonumber(init_state['block_number'])),
            duration / (tonumber(final_state['block_number']) - tonumber(init_state['block_number'])),
            (tonumber(final_state['balance']) - tonumber(init_state['balance'])) / duration
        )
    )
end
