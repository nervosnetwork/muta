# 配置说明

默认的创世块和配置样例在 `config` 文件夹中，此处对其中的一些字段进行说明。

## 创世块

`genesis.toml`:

```toml
timestamp = 0
prevhash = "44915be5b6c20b0678cf05fcddbbaa832e25d7e6ac538784cd5c24de00d47472"

[[services]]
name = "asset"
payload = '''
{
    "id": "f56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c",
    "name": "MutaToken",
    "symbol": "MT",
    "supply": 320000011,
    "issuer": "f8389d774afdad8755ef8e629e5a154fddc6325a"
}
'''

[[services]]
name = "metadata"
payload = '''
{
    "chain_id": "b6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036",
    "common_ref": "7446645045376b553041",
    "timeout_gap": 20,
    "cycles_limit": 999999999999,
    "cycles_price": 1,
    "interval": 3000,
    "verifier_list": [
        {
            "bls_pub_key": "040386a8ac1cce6fd90c31effa628bc8513cbd625c752ca76ade6ff37b97edbdfb97d94caeddd261d9e2fd6b5456aecc100ea730ddee3c94f040a54152ded330a4e409f39bfbc34b286536790fef8bbaf734431679ba6a8d5d6994e557e82306df",
            "address": "12d8baf8c4efb32a7983efac2d8535fe57deb756",
            "propose_weight": 1,
            "vote_weight": 1
        },
        {
            "bls_pub_key": "040e7b00b59d37d4d735041ea1b69a55cd7fd80e920b5d70d85d051af6b847c3aec5b412b128f85ad8b4c6bac0561105a80fa8dd5f60cd42c3a2da0fd0b946fa3d761b1d21c569e0958b847da22dec14a132121027006df8c5d4ccf7caf8535f70",
            "address": "a55e1261a73116c755291140e427caa0cbb5309e",
            "propose_weight": 1,
            "vote_weight": 1
        },
        {
            "bls_pub_key": "0413584a15f1dec552bb12233bf73a886ed49a3f56c68eda080743577005417635c9ac72a528a961a0e14a2df3a50a5c660641f446f629788486d7935d4ad4918035ce884a98bbaaa4c96307a2428729cba694329a693ce60c02e13b039c6a8978",
            "address": "78ef0eff2fb9f569d86d75d22b69ea8407f6f092",
            "propose_weight": 1,
            "vote_weight": 1
        },
        {
            "bls_pub_key": "041611b7da94a7fb7a8ff1c802bbf61da689f8d6f974d99466adeb1f47bcaff70470b6f279763abeb0cec5565abcfcb4ce13e79b8c310f0d1b26605b61ac2c04e0efcedbae18e763a86adb7a0e8ed0fcb1dc11fded12583972403815a7aa3dc300",
            "address": "103252cad4e0380fe57a0c73f549f1ee2c9ea8e8",
            "propose_weight": 1,
            "vote_weight": 1
        }
    ],
    "propose_ratio": 15,
    "prevote_ratio": 10,
    "precommit_ratio": 10,
    "brake_ratio": 7,
    "tx_num_limit": 20000,
    "max_tx_size": 1024
}
'''
```

创世块的初始化参数：

- `timestamp`: 创世块的时间戳，可以随意设置，配置成 0，或者当天 0 点的时间都可以。
- `prevhash`: 可以随意设置，只会影响查询创世块时的字段显示。

`services` 为各个 service 的初始化参数。各 service 的初始化参数说明：

- `asset`: 如果链需要发行原生资产，可以参考上面的例子填写，否则可以去掉
  - `id`: 资产的唯一 id，建议设置成 hash ，以免在之后和链上其他资产重复
  - `name`: 资产名字
  - `symbol`: 资产简称
  - `supply`: 资产发行总量
  - `issuer`: 发行方地址
- `metadata`: 链的元数据，必须填写
  - `chain_id`: 链唯一 id，建议设置为任意 hash
  - `common_ref`: BLS 签名需要
  - `timeout_gap`: 交易池能接受的最大超时块范围。用户在发送交易的时候，需要填写 `timeout` 字段，表示块高度超过这个值后，如果该交易还没有被打包，则以后都不会被打包，这样可以确保之前的某笔交易超时后一定会失败，避免用户的交易很长时间未被打包后换 `nonce` 重发交易，结果两笔交易都上链的情况。当用户填写的 `timeout` > `chain_current_height` + `timeout_gap` 时，交易池会拒绝这笔交易。考虑到一些特殊情况（比如一些冷钱包对交易签名后较长时间才发出），该值可以适当调大
  - `cycles_limit`: 10进制，链级别对单个交易可以消耗的最大 `cycle` 的限制
  - `cycles_price`: 最小 cycle 价格，目前没有使用
  - `interval`: 出块间隔，单位为 ms。当设置为 3s 的时候，出块间隔并不是严格的 3s，而是在 3s 附近波动，这是因为 Overlord 共识在响应性上的优化。当网络状况较好的时候，会小于 3s，网络情况较差，则会略大于 3s。
  - `verifier_list`: 共识列表
    - `bls_pub_key`: 节点的 BLS 公钥
    - `address`: 节点的地址
    - `propose_weight`: 节点的出块权重。如果有四个共识节点，出块权重分别为 `1, 2, 3, 4`，则第一个节点的出块概率为 `1 / (1 + 2 + 3 + 4)`。投票权重的逻辑类似。
    - `vote_weight`: 节点的投票权重
  - `propose_ratio`: propose 阶段的超时时间与出块时间的比例。例如 `propose_ratio` 为 5, `interval` 为 3000，则 propose 阶段的超时时间为 `15 / 10 * 3000 = 4500`，单位均为毫秒。
  - `prevote_ratio`: prevote 阶段的超时时间与出块时间的比例
  - `precommit_ratio`: precommit 阶段的超时时间与出块时间的比例
  - `brake_ratio`: brake 阶段的超时时间与出块时间的比例
  - `tx_num_limit`: 每一个块里最多可以打包的交易数
  - `max_tx_size`: 单个交易最大的字节数

## 链的运行配置

`chain.toml`:

```toml
data_path = "./devtools/chain/data/1"
privkey = "592d6f62cd5c3464d4956ea585ec7007bcf5217eb89cc50bf14eea95f3b09706"

[network]
listening_address = "0.0.0.0:1337"
rpc_timeout = 10

[graphql]
graphiql_uri = "/graphiql"
listening_address = "0.0.0.0:8000"
graphql_uri = "/graphql"
workers = 0 # if 0, uses number of available logical cpu as threads count.
maxconn = 25000
max_payload_size = 1048576

[executor]
light = false

[mempool]
broadcast_txs_size = 200
broadcast_txs_interval = 200
pool_size = 200000

[logger]
metrics = false
log_path = "./devtools/chain/logs/1"
log_to_console = true
filter = "info"
log_to_file = true
console_show_file_and_line = false
```

- `privkey`: 节点私钥，节点的唯一标识，在作为 bootstraps 节点时，需要给出地址和该私钥对应的公钥让其他节点连接；如果是出块节点，该私钥对应的地址需要在 consensus verifier_list 中
- `data_path`: 链数据所在目录
- `graphql`:
  - `listening_address`: GraphQL 监听地址
  - `graphql_uri`: GraphQL 服务访问路径
  - `graphiql_uri`: GraphiQL 访问路径
  - `workers`: 处理 http 的线程数量，填 0 的话，会默认按 CPU 的核数
  - `maxconn`: 最大连接数
- `network`:
  - `listening_address`: 链 p2p 网络监听地址
  - `rpc_timeout`: RPC 调用（例如从其它节点拉交易）超时时间，单位为秒
- `network.bootstraps`: 起链时连接的初始节点信息
  - `pubkey`: 公钥
  - `address`: 网络地址
- `mempool`: 交易池相关配置
  - `pool_size`: 交易池大小
  - `broadcast_txs_size`: 一次批量广播的交易数量
  - `broadcast_txs_interval`: 每次广播交易的时间间隔，单位 ms
- `executor`:
  - `light`: 设为 true 时，节点将只保存最新高度的 state
- `logger`: 日志相关配置
  - `filter`: 全局日志级别
  - `log_to_console`: 是否输出日志到 console，生产环境建议设为 false
  - `console_show_file_and_line`: 当 `log_to_console` 和本配置都置为 true 时，console 输出的日志里会包含日志打印处的文件和行数。本地通过日志调试时有用，一般可以设为 false。
  - `log_to_file`: 是否输出日志到文件
  - `metrics`: 是否输出 metrics。logger 模块中有专门的 metrics 输出函数，如有需要，可以用来输出 metrics 日志，不受全局日志级别的影响，且对应的日志会输出到专门的文件。
  - `log_path`: 会在该路径生成两个日志文件：`muta.log` 和 `metrics.log`。`metrics.log`中包含了专门的 metrics 日志，`muta.log` 中包含了其它所有 log 输出。

## 日志示例

文件中的日志均为 json 格式，方便用程序处理。其中 message 一般为一个嵌套的 json 结构，用来表达结构化信息。

```bash
$ tail logs/muta.log -n 1
{"time":"2020-02-12T17:11:04.187149+08:00","message":"update_after_exec cache: height 2, exec height 0, prev_hash 039d2f399864dba72c5b0f26ec989cba9bdcb9fca23ce48c8bc8c4398cb2ad0b,latest_state_root de37f62c1121e283ad52fe5b3e260c899f03d42da29fdfe08e82655185d9b772 state root [de37f62c1121e283ad52fe5b3e260c899f03d42da29fdfe08e82655185d9b772], receipt root [], confirm root [], cycle used []","module_path":"core_consensus::status","file":"/Users/huwenchao/.cargo/git/checkouts/muta-cad92efdb84944c1/34d052a/core/consensus/src/status.rs","line":114,"level":"INFO","target":"core_consensus::status","thread":"main","thread_id":4576796096,"mdc":{}}

$ tail logs/metrics.log -n 1
{"time":"2020-02-12T17:11:04.187240+08:00","message":"{\"timestamp\":1581498664187,\"event_name\":\"update_exec_info\",\"event_type\":\"custom\",\"tag\":{\"confirm_root\":\"56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421\",\"exec_height\":1,\"receipt_root\":\"56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421\",\"state_root\":\"de37f62c1121e283ad52fe5b3e260c899f03d42da29fdfe08e82655185d9b772\"},\"metadata\":{\"address\":\"f8389d774afdad8755ef8e629e5a154fddc6325a\",\"v\":\"0.3.0\"}}","module_path":"core_consensus::trace","file":"/Users/huwenchao/.cargo/git/checkouts/muta-cad92efdb84944c1/34d052a/core/consensus/src/trace.rs","line":24,"level":"TRACE","target":"metrics","thread":"main","thread_id":4576796096,"mdc":{}}
```