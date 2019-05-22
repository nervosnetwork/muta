# use wrk for performance test

Install `wrk` on your system. On macOS, you can use `homebrew` to install, just use command `brew install wrk`. To compile from source code, you can refer to <https://github.com/wg/wrk/blob/master/INSTALL>.

To test simple jsonrpc methods like `peerCount`, you can just use a simple lua script to write your request body like below.

```
$ wrk -c20 -d5s -t8 --script=peer_count.lua --latency http://127.0.0.1:3030
Running 5s test @ http://127.0.0.1:3030
  8 threads and 20 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    56.65ms   29.30ms 244.89ms   80.95%
    Req/Sec    35.61     13.70    70.00     59.34%
  Latency Distribution
     50%   48.34ms
     75%   68.12ms
     90%   94.61ms
     99%  157.63ms
  1448 requests in 5.09s, 209.28KB read
Requests/sec:    284.23
Transfer/sec:     41.08KB
```

You can change the parameters as you need.

To send a transaction(like what we do in `erc20_transfer.lua` and `send_unsafe_transaction.lua`), we need additional packages to get it work.
Install [luajit][luajit] and [luarocks-jit][luarocks-jit], and then use the commands below to install the dependent packages.

```
# install luajit and luarocks-jit using homebrew
brew install luajit
brew tap mesca/luarocks
brew install luarocks51 --with-luajit

# use luarocks to instal packages
luarocks-jit install lua-protobuf
luarocks-jit install lua-resty-http
luarocks-jit install json4lua
luarocks-jit install luasocket
luarocks-jit install serpent
luarocks-jit install hex
```

If you don't have write permissions for `/usr` directory, you may have to add `--local` postfix to the commands.

An `erc20_transter` test is like blow:

```
$ wrk -c2 -d10s -t2 --script=erc20_transfer.lua --latency http://127.0.0.1:3030
valid_until_block:      14286
deploy_erc20 res:       {
  {
    hash = "0xef5f733b2e5d9475c8d161a2635b0db85bd054c019c06c8be6a7d45e2964de92",
    status = "OK"
  } --[[table: 0x0b39efa8]]
} --[[table: 0x0b398988]]
wait until contract deployed
wait until contract deployed
wait until contract deployed
wait until contract deployed
wait until contract deployed
deployed erc20 contract @       0x822ff676b3dd5e2a94bec24427c9365a7baf1e54
contract_addr:  0x822ff676b3dd5e2a94bec24427c9365a7baf1e54      init_state      {
  balance = "0x0000000000000000000000000000000000000000000000000000000000000000",
  block_number = "0x10c0"
} --[[table: 0x0b4aaf38]]
Running 10s test @ http://127.0.0.1:3030
  2 threads and 2 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     3.95ms    2.91ms  56.69ms   95.52%
    Req/Sec   259.28     54.23   360.00     75.50%
  Latency Distribution
     50%    3.56ms
     75%    3.93ms
     90%    4.66ms
     99%   17.91ms
  5178 requests in 10.03s, 1.16MB read
Requests/sec:    516.07
Transfer/sec:    117.93KB
------------------------------------------------------
| state |              balance |         block height |
| init  |                    0 |               0x10c0 |
| final |                 1688 |               0x10c1 |
------------------------------------------------------
duration                    : 10.034s
average tx num in a block   : 1688
average block time          : 10.034s
average tx num in a second  : 168
```

Feel free to write more tests as you need.

**Notice**

As we can not sign a transaction in lua yet(for some protobuf undetermined serializing problem), we use a `sendUnsafeTransaction` method
to do our performance test, it use private key as unsigned transaction as params, the server sign it and do a `sendRawTransaction` method.
This may make your performance test result of transaction related methods a little lower than it really is.
And never use the `sendUnsafeTransaction` method in production to prevent your private key leaking.


## reference

- [GitHub repo][wrk], there are some useful examples to write scripts.
- [wrk script guide](https://github.com/wg/wrk/blob/master/SCRIPTING)
- lua packages
	- [lua protobuf](https://github.com/starwing/lua-protobuf)
	- [lua-hex](https://github.com/mah0x211/lua-hex)


[wrk]: https://github.com/wg/wrk
[luajit]: http://luajit.org/
[luarocks-jit]: https://luarocks.org/
