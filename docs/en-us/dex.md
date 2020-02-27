# Tutorial：使用 Muta 框架从零开发一条 Dex 专有链

我们的目标是开发一条链上挂单、链上撮合、链上成交的简易 dex 专有链，旨在通过 step by step 的流程，帮助开发者熟悉 Muta 框架，学会如何使用框架开发自己的区块链。

> 在开始本教程之前，开发者需要先学习 [Service 开发指南](service_dev.md)

我们按照 [Service 开发指南](service_dev.md) 中提到的，使用 Muta 框架开发自己的区块链流程，来开发这条 dex 专有链：

1. 思考自己链的专属需求，确定需要哪些 Service
2. 如果需要的 Service 有现成的，可以直接复用；如果没有，可以自己开发
3. 将这些 Service 接入框架，编译运行！

## 1. 思考需要的 Service

我们一共需要 2 个 Service，除了 Dex Service 外，由于 Dex 链需要有进行交易的资产，所以还需要一个 Asset Service。Asset Service 除了常见的发行、转账、查询功能外，还需要一个锁定资产功能。因为用户发起挂单交易时，需要锁定用户资产，确保成交时有足够的余额来完成交易。Dex 订单成交时，需要修改用户资产余额，所以 Asset Service 需要提供修改余额接口，并且该接口只能由 Dex Service 调用，无法被用户直接调用。我们将 Asset Service 的接口定义如下：

```rust
#[cycles(210_00)]
#[write]
fn create_asset(
    &mut self,
    ctx: ServiceContext,
    payload: CreateAssetPayload,
) -> ProtocolResult<Asset>;

#[cycles(100_00)]
#[read]
fn get_asset(&self, ctx: ServiceContext, payload: GetAssetPayload) -> ProtocolResult<Asset>;

#[cycles(210_00)]
#[write]
fn transfer(&mut self, ctx: ServiceContext, payload: TransferPayload) -> ProtocolResult<()> ;

#[cycles(100_00)]
#[read]
fn get_balance(
    &self,
    ctx: ServiceContext,
    payload: GetBalancePayload,
) -> ProtocolResult<GetBalanceResponse>;

#[cycles(210_00)]
#[write]
fn lock_value(
    &mut self,
    ctx: ServiceContext,
    payload: ModifyBalancePayload,
) -> ProtocolResult<()>;

#[cycles(210_00)]
#[write]
fn unlock_value(
    &mut self,
    ctx: ServiceContext,
    payload: ModifyBalancePayload,
) -> ProtocolResult<()>;

#[cycles(210_00)]
#[write]
fn add_value(
    &mut self,
    ctx: ServiceContext,
    payload: ModifyBalancePayload,
) -> ProtocolResult<()>;

#[cycles(210_00)]
#[write]
fn sub_value(
    &mut self,
    ctx: ServiceContext,
    payload: ModifyBalancePayload,
) -> ProtocolResult<()>;
```

Dex Service 包含的功能有：

1. 增加交易对
2. 查询交易对
3. 发起挂单交易（买或卖）
4. 每个 block 执行结束后，匹配订单并成交
5. 查询订单状态

功能 1、2、3、5 可由用户调用 Servcie 接口触发：

```rust
    #[cycles(210_00)]
    #[write]
    fn add_trade(&mut self, ctx: ServiceContext, payload: AddTradePayload) -> ProtocolResult<()>;

    #[read]
    fn get_trades(&self, _ctx: ServiceContext) -> ProtocolResult<GetTradesResponse>;

    #[cycles(210_00)]
    #[write]
    fn order(&mut self, ctx: ServiceContext, payload: OrderPayload) -> ProtocolResult<()>;

    #[read]
    fn get_order(
        &self,
        ctx: ServiceContext,
        payload: GetOrderPayload,
    ) -> ProtocolResult<GetOrderResponse>;
```

功能 4 由 `#[hook_after]` 自动触发：

```rust
    #[hook_after]
    fn match_and_deal(&mut self, params: &ExecutorParams) -> ProtocolResult<()>;
```

## 2. 开发 Asset Service，Dex Service

### 使用脚手架 muta-drone

Service 设计完成后，我们进入开发阶段。我们需要新建一个 rust 工程，同时在工程中引用 Muta Library，好消息是 Muta 框架提供了脚手架 [muta-drone](https://www.npmjs.com/package/muta-drone) 来帮助开发者一键配置工程目录。

- 安装脚手架

```shell
npm install -g muta-drone
```

- 运行 `drone node` 命令，按提示配置工程目录

```shell
-> drone node
    ? The name of your chain. muta-tutorial-dex     // 工程目录命
    ? The chain id of your chain (32-Hash) (default: random generation)     // 回车键使用默认值
    ? Private key of this node (secp256k1) (default: random generation)     // 回车键使用默认值
    ? Verifier's address set, except you (eg. [0x1..., 0x2..])      // 回车键使用默认值
    ? cycles limit 1099511627776        // 回车键使用默认值
    Downloading template....
    Copying template....
    All right, enjoy!
    Enter the following command to start your chain
    $ cd muta-tutorial-dex && cargo run
    When the rust compilation is complete, access graphiql play your chain.
    $ open http://localhost:8000/graphiql
-> 
```

muta-tutorial-dex 目录结构如下：

```shell
./muta-tutorial-dex
├── Cargo.lock
├── Cargo.toml
├── LICENSE
├── README.md
├── config
│   ├── chain.toml
│   └── genesis.toml
├── rust-toolchain
├── services
│   └── metadata
│       ├── Cargo.toml
│       └── src
│           └── lib.rs
└── src
    └── main.rs
```

可以看到，目录主要包含 config，services 和 src 三个子目录：

- config：链的配置信息
- services：包含链的所有 service
- src：这条链的 bin 目录，在 main.rs 中，我们将 services 接入 muta library，并启动整条链

services 目录中包含了一个 [metadata service](https://github.com/nervosnetwork/muta-template/tree/master/node-template/services/metadata)，该 service 为系统内置 service。我们需要在 services 目录中加上 asset service 和 dex service，脚手架 muta-drone 也有命令帮助我们构建 service 目录。

- 运行 `drone service` 命令，构建 service 工程目录

```shell
-> cd muta-tutorial-dex
-> drone service asset
        Downloading template....
        Copying template....
        Done! asset service path /patht/o/muta-tutorial-dex/services/asset
-> drone service dex
        Downloading template....
        Copying template....
        Done! asset service path /path/to/muta-tutorial-dex/services/dex
```

Service 工程目录如下：

```shell
./asset
├── Cargo.toml
├── rust-toolchain
└── src
    ├── lib.rs
    └── types.rs

./dex
├── Cargo.toml
├── rust-toolchain
└── src
    ├── lib.rs
    └── types.rs
```

可以看到 [lib.rs](https://github.com/nervosnetwork/muta-template/blob/master/service-template/src/lib.rs) 和 [types.rs](https://github.com/nervosnetwork/muta-template/blob/master/service-template/src/types.rs) 默认帮我们实现了一个简单的读写 key-value 的 service。

### 编写 Asset Service

学习完 [Service 开发指南](service_dev.md)，相信读者对如何开发 asset service 已经有了一定的想法，并且能够阅读 [asset service](https://github.com/mkxbl/muta-tutorial-dex/tree/master/services/asset) 源码。这里就不复述相关内容，仅向读者说明一些需要注意的地方：

#### 代码结构

Service 的组件定义在 lib.rs 中，组件需要用到的数据结构，如输入输出参数(`TransferPayload`)、事件类型(`TransferEvent`)、存储类型(`Asset`)定义在 types.rs 中。

#### 序列化

接口方法的输入输出参数、事件类型均使用 json 序列化，存入世界状态的数据类型，如 `Asset` 、`Balance` ，需要实现 `FixedCodec` trait，该 trait 使用 rlp 作为固定序列化方案。

#### 创世配置

Asset Service 通过 `fn init_genesis` 方法，注册了 Muta Tutorial Token，该 token 信息将包含在创世块的世界状态里：

```rust
// lib.rs
#[genesis]
fn init_genesis(&mut self, payload: InitGenesisPayload) -> ProtocolResult<()> {
    let asset = Asset {
        id: payload.id,
        name: payload.name,
        symbol: payload.symbol,
        supply: payload.supply,
        issuer: payload.issuer.clone(),
    };

    self.assets.insert(asset.id.clone(), asset.clone())?;

    let balance = Balance {
        current: payload.supply,
        locked: 0,
    };

    self.sdk.set_account_value(&asset.issuer, asset.id, balance)
}

// types.rs
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct InitGenesisPayload {
    pub id: Hash,
    pub name: String,
    pub symbol: String,
    pub supply: u64,
    pub issuer: Address,
}
```

该方法的输入参数 `InitGenesisPayload`, 定义在 muta-tutorial-dex/config/genesis.toml 文件中，该文件包含所有 service 的创世配置信息：

```toml
# config/genesis.toml
[[services]]
name = "asset"
payload = '''
{
    "id": "f56924db538e77bb5951eb5ff0d02b88983c49c45eea30e8ae3e7234b311436c",
    "name": "Muta Tutorial Token",
    "symbol": "MTT",
    "supply": 1000000000,
    "issuer": "f8389d774afdad8755ef8e629e5a154fddc6325a"
}
'''
```

框架在创建创世块时，会读取该配置并调用 `fn init_genesis` 方法。

#### 接口权限

Asset Service 的 `fn lock`、`fn unlock`、`fn add_value`、`fn sub_value` 接口方法，只能被 Dex Service 调用，无法被用户直接调用。在 Asset Service 中定义了 `ADMISSION_TOKEN`，通过检验在 `ServiceContext` 中的 `extra` 字段是否包含该令牌进行权限控制。

```rust
const ADMISSION_TOKEN: Bytes = Bytes::from_static(b"dex_token");
```

> 注意：由于框架正在持续的开发过程中，所以未来对调用的权限控制机制可能会修改

### 编写 Dex Service

Dex Service 源码可以在 [这里](https://github.com/mkxbl/muta-tutorial-dex/tree/master/services/dex) 找到，注意事项同上。

## 3. 将 Service 接入框架，编译运行！

前面已经提到，这部分工作将在 src 目录的 [main](https://github.com/nervosnetwork/muta-template/blob/master/node-template/src/main.rs) 文件中完成。脚手架下载的 main 文件已经帮我们实现了绝大部分代码，所以这部分工作将变得非常简单。

在模版代码中，定义了一个 `struct DefaultServiceMapping` 结构体，并为该结构体实现了 `trait ServiceMapping`，框架通过 `trait ServiceMapping` 可以获取到所有 service 实例，从而将开发者定义的 service 接入框架底层组件。

```rust
struct DefaultServiceMapping;

impl ServiceMapping for DefaultServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK>(
        &self,
        name: &str,
        sdk: SDK,
    ) -> ProtocolResult<Box<dyn Service>> {
        let service = match name {
            "asset" => Box::new(asset::AssetService::new(sdk)?) as Box<dyn Service>,
            "metadata" => Box::new(metadata::MetadataService::new(sdk)?) as Box<dyn Service>,
            _ => {
                return Err(MappingError::NotFoundService {
                    service: name.to_owned(),
                }
                .into())
            }
        };

        Ok(service)
    }

    fn list_service_name(&self) -> Vec<String> {
        vec!["asset".to_owned(), "metadata".to_owned()]
    }
}
```

`trait ServiceMapping` 包含两个方法，一个 `fn get_service` 用来根据 service 名称获取 service 实例，另一个 `fn list_service_name` 用来获取所有 service 名称。

需要注意的是，框架将使用在 `fn list_service_name` 方法中 service 名称排列的顺序，依次调用 service 中 `#[genesis]` 或 `#[hook_before]` 或 `#[hook_after]` 标记的方法。

我们需要做的，仅仅是把 `fn get_service` 和 `fn list_service_name` 方法中的 service 集合，替换成我们 services 目录中包含的 service 集合：

```rust
struct DefaultServiceMapping;

impl ServiceMapping for DefaultServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK>(
        &self,
        name: &str,
        sdk: SDK,
    ) -> ProtocolResult<Box<dyn Service>> {
        let service = match name {
            "metadata" => Box::new(metadata::MetadataService::new(sdk)?) as Box<dyn Service>,
            "asset" => Box::new(asset::AssetService::new(sdk)?) as Box<dyn Service>,
            "dex" => Box::new(dex::DexService::new(sdk)?) as Box<dyn Service>,
            _ => {
                return Err(MappingError::NotFoundService {
                    service: name.to_owned(),
                }
                .into())
            }
        };

        Ok(service)
    }

    fn list_service_name(&self) -> Vec<String> {
        vec!["metadata".to_owned(), "asset".to_owned(), "dex".to_owned()]
    }
}
```

到这里，所有的开发工作就完成了，运行 `cargo run` 编译并启动 dex 链。

通过浏览器打开 http://localhost:8000/graphiql，即可与 dex 链进行交互，graphiql 的使用方法参见[文档] (graphql_api.md)。

> 注意：由于框架正在持续的开发过程中，所以框架的 api 有可能发生变动