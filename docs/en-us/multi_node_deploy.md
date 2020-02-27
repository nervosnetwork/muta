# 多节点链部署指南

## 生成公私钥

```bash
# 安装 muta-keypair 工具
$ cargo install --git https://github.com/nervosnetwork/muta.git --bin muta-keypair

$ muta-keypair -h
muta_keypair 0.1
Muta Dev <muta@nervos.org>
a tool to generate keypairs for muta

USAGE:
    muta-keypair [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --common_ref <common_ref>           common_ref for bls signature, it will be randomly generated if not passed
                                            [default: ]
    -n, --number <number>                   Number of keypairs to generate [default: 4]
    -p, --private_keys <private_keys>...    Generate keypairs from a given private key vector

# 共同商议好一个字符串，经过 hex 编码后作为 BLS 签名的 common_ref 用
# 或者一个人先用 muta-keypair 生成，把随机产生的 common_ref 分发给其它人
$ muta-keypair -n 1
{
  "common_ref": "37537a3658476b334a71",
  "keypairs": [
    {
      "index": 1,
      "private_key": "33e10486adf1b64dbcf9a2c9531ebd78c9c651daf97bd6d2d170b0743f3dd789",
      "public_key": "02bccb67683203d8b2157c6c2b36796094dfe86302250eb823c67e93929a7a8265",
      "address": "7529ec67eccf908a4ec49d119b5a47e45eeaceba",
      "bls_public_key": "0406367be42b1373981cc1e7a65881b7e17931e11a3af54ce7016317f7fb84bd674f9ee2c95ff9833d197c43ca97363322056ed9acee03142c8923a36a39ce339c56dd215d9e262b3df66b4756e476df9cc796d9cc21b59be9e1b2abe9fcbbf6bd"
    }
  ]
}

$ muta-keypair -n 1 -c 37537a3658476b334a71
{
  "common_ref": "37537a3658476b334a71",
  "keypairs": [
    {
      "index": 1,
      "private_key": "9024ebaef92ce541bbfd2cf55a6c1f8ea797393cb6234db550f84b6526d7b9ab",
      "public_key": "02ff12550ee3a923a0c7cc4fc2bea0670ec3057f8a368fb4d375957cdc26e0bce9",
      "address": "c2c456378a72ec16a50264e36232ab9654a81b62",
      "bls_public_key": "04037457b7c7b88683d2affa5b2a0045bb8816d8f67412490d82fd447cc77b96f04090854a38b39f0d2aedb264a01120350df3bb0acd85abfa8a1462af0512ba14032ffc71099a92ed9cb9d2f4046bafb9438ac97d30890dbf7c5213a81ecd3622"
    }
  ]
}
```

## 生成创世块

一个创世块的示例如下。
其中重点需要商议的部分是：
- common_ref: 刚刚大家生成公钥时用的 common_ref
- verifier_list: 初始出块节点，需要填写地址，bls_pub_key 和权重
- admin: 共同认可的超级管理员地址

其它创世块信息可以参考配置说明。

生成创世块后，分发给各个节点。每个节点所使用的创世块必须一致，否则不在同一条链。

```toml
timestamp = 0
prevhash = "44915be5b6c20b0678cf05fcddbbaa832e25d7e6ac538784cd5c24de00d47472"

[[services]]
name = "asset"
payload = '''
{
    "id": "f56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c",
    "name": "Muta Token",
    "symbol": "MT",
    "supply": 1000000000,
    "issuer": "f8389d774afdad8755ef8e629e5a154fddc6325a"
}
'''

[[services]]
name = "metadata"
payload = '''
{
    "chain_id": "b6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036",
    "common_ref": "703873635a6b51513451",
    "timeout_gap": 20,
    "cycles_limit": 999999999999,
    "cycles_price": 1,
    "interval": 3000,
    "verifier_list": [
        {
            "bls_pub_key": "04188ef9488c19458a963cc57b567adde7db8f8b6bec392d5cb7b67b0abc1ed6cd966edc451f6ac2ef38079460eb965e890d1f576e4039a20467820237cda753f07a8b8febae1ec052190973a1bcf00690ea8fc0168b3fbbccd1c4e402eda5ef22",
            "address": "f8389d774afdad8755ef8e629e5a154fddc6325a",
            "propose_weight": 1,
            "vote_weight": 1
        }
    ],
    "propose_ratio": 15,
    "prevote_ratio": 10,
    "precommit_ratio": 10,
    "brake_ratio": 7
}
'''
```

## 各节点生成自己的配置文件

链配置示例如下:

```
# crypto
privkey = "45c56be699dca666191ad3446897e0f480da234da896270202514a0e1a587c3f"

# db config
data_path = "./data"

[graphql]
listening_address = "0.0.0.0:8000"
graphql_uri = "/graphql"
graphiql_uri = "/graphiql"

[network]
listening_address = "0.0.0.0:1337"
rpc_timeout = 10

[[network.bootstraps]]
pubkey = "031288a6788678c25952eba8693b2f278f66e2187004b64ac09416d07f83f96d5b"
address = "0.0.0.0:1888"

[mempool]
pool_size = 20000
broadcast_txs_size = 200
broadcast_txs_interval = 200

[executor]
light = false

[logger]
filter = "info"
log_to_console = true
console_show_file_and_line = false
log_path = "logs/"
log_to_file = true
metrics = true
# you can specify log level for modules with config below
# modules_level = { "overlord::state::process" = "debug", core_consensus = "error" }
```

其中与网络相关的需要特别注意：

```
# 该配置为监听的网络地址
[network]
listening_address = "0.0.0.0:1337"

# 该配置为起链时连接的节点
[[network.bootstraps]]
pubkey = "031288a6788678c25952eba8693b2f278f66e2187004b64ac09416d07f83f96d5b"
address = "44.55.66.77:1337"
```

可以商议 1 个或多个节点作为 bootstraps 节点，bootstraps 节点的 `network.bootstraps` 配置可以不填。
在起链时需要 bootstraps 节点先启动。

其余节点的 `network.bootstraps` 配置填写 bootstraps 节点的链监听地址和公钥。
启动后节点会连接这些 bootstraps 节点，然后通过 P2P 网络协议自动发现，从而连上整个链。

其余配置各个节点可以根据自己的需求自行修改。
具体含义请参考配置说明章节。
