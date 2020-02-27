# Service 开发指南

## 概念

区块链作为一种新的分布式应用，可以简单的理解成一个副本状态机，同时使用密码学做到应用数据的可验证和防篡改。一方面，多个副本的通信和一致性，由 P2P 网络、交易池和共识组件等共同完成，这些组件也是区块链架构中的底层模块，一般很少变动，所以可以固化到框架中直接提供给开发者使用。另一方面，状态机部分往往与链的具体需求和业务相关，需要由开发者进行自定义，框架提供 SDK 来让减轻这部分工作的时间成本和技术复杂度。

Muta 框架将用户自定义部分抽象成一个 Service，同时提供 `ServiceSDK` 让 Service 开发变得简单和高效。每个 Service 完成一个相对独立的功能，单独维护自己的存储和操作接口，类似一个运行在沙盒里的小型状态机。开发者可以使用 Service 开发链的治理模块、业务逻辑，甚至是将虚拟机接入区块链。除了开发自己的 Service，你也可以复用他人已经开发好的 Service，未来 Muta 框架会提供许多常见功能的 Service，如 Asset、Risc-V 虚拟机、DPoS、多签治理等等。Service 之间可以互相调用，这些 Service 共同组成了链的状态机部分，通过框架接口将状态机接入区块链底层组件，一条专属你的全新链就开发完成啦。

换句话说，使用 Muta 框架开发你自己的区块链只需 3 步：

1. 思考自己链的专属需求，确定需要哪些 Service
2. 如果需要的 Service 有现成的，可以直接复用；如果没有，可以自己开发
3. 将这些 Service 接入框架，编译运行！

这篇文章主要介绍 Service 的组成和开发指南。在熟悉 Service 之后，可以阅读 [开发一条 Dex 专有链](dex.md)，学习如何使用 Muta 框架从零开发一条区块链。

## 开发范式

在设计 Service 时，我们希望降低开发者的开发门槛，让更多对区块链不那么熟悉的开发者也可以快速上手，开发自己的区块链。在开发体验上，我们希望向开发合约的体验靠拢，如果你已经学会了如何开发合约，那么恭喜你，你也已经学会了如何开发 Service。在开发范式上，我们把 Service 抽象成一个小型状态机，Service 包含普通状态机所拥有的组件：

- 状态（存储）
- 输入（接口）
- 函数（逻辑）
- 输出（返回值）
- 异常和错误处理

同时也包含区块链特有的一些组件：

- 创世块配置
- 事件
- 资源消耗统计（Cycle）
- 与区块链相关的钩子函数，`before_block` 在一个区块执行前调用 Service 的函数，`after_block` 在一个区块执行后调用 Service 的函数

接下来我们分别介绍每个组件。

## 状态存储

区别于普通程序的存储，区块链的存储需要使用密码学保证数据的可验证和防篡改。`ServiceSDK` 提供了一些数据类型和接口，让开发者无需关心密码学相关的部分，可以像开发普通程序一样完成状态的存储。

`ServiceSDK` 提供了两类存储接口，一类是获得常见数据类型 map、array、uint64、String、Bool 的接口，使用这些数据类型的数据会自动存入区块链的世界状态中。

```rust
pub trait ServiceSDK {
    // Alloc or recover a `Map` by` var_name`
    fn alloc_or_recover_map<Key: 'static + FixedCodec + PartialEq, Val: 'static + FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreMap<Key, Val>>>;

    // Alloc or recover a `Array` by` var_name`
    fn alloc_or_recover_array<Elm: 'static + FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreArray<Elm>>>;

    // Alloc or recover a `Uint64` by` var_name`
    fn alloc_or_recover_uint64(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreUint64>>;

    // Alloc or recover a `String` by` var_name`
    fn alloc_or_recover_string(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreString>>;

    // Alloc or recover a `Bool` by` var_name`
    fn alloc_or_recover_bool(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreBool>>;

... ...
}
```

如果这些数据类型不能满足你的需求，还有一类 key-value 接口：

```rust
pub trait ServiceSDK {
    // Get a value from the service state by key
    fn get_value<Key: FixedCodec, Ret: FixedCodec>(&self, key: &Key)
        -> ProtocolResult<Option<Ret>>;

    // Set a value to the service state by key
    fn set_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        key: Key,
        val: Val,
    ) -> ProtocolResult<()>;

    // Get a value from the specified address by key
    fn get_account_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        address: &Address,
        key: &Key,
    ) -> ProtocolResult<Option<Ret>>;

    // Insert a pair of key / value to the specified address
    fn set_account_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        address: &Address,
        key: Key,
        val: Val,
    ) -> ProtocolResult<()>;

... ...
}
```

使用这类接口的数据也会自动存储在世界状态中。

你需要使用结构体来封装 Service，以 Dex Service 为例：

```rust
// A dex service
pub struct DexService<SDK: ServiceSDK> {
    sdk: SDK,
    trades: Box<dyn StoreMap<Hash, Trade>>,
    buy_orders: Box<dyn StoreMap<Hash, Order>>,
    sell_orders: Box<dyn StoreMap<Hash, Order>>,
    history_orders: Box<dyn StoreMap<Hash, Order>>,
    validity: Box<dyn StoreUint64>,
}
```

此外，Service 的结构体中需要包含实现 `ServiceSDK` trait 的数据类型，通过该类型获得 ServiceSDK 提供的能力。

## 接口方法

Service 通过过程宏标记方法，来提供链外和其他 Service 可以调用的接口，以 Dex Service 为例：

```rust
#[service]
impl<SDK: 'static + ServiceSDK> DexService<SDK> {
    #[cycles(210_00)]
    #[write]
    fn add_trade(&mut self, ctx: ServiceContext, payload: AddTradePayload) -> ProtocolResult<()>;

    #[read]
    fn get_trades(&self, _ctx: ServiceContext) -> ProtocolResult<GetTradesResponse>;

... ...
}
```
给 Service 结构体绑定方法的 `impl` 块中，需要标记 `#[service]` 过程宏，该过程宏会给 Service 自动实现 `Service` trait，框架通过该 trait 和 Service 交互。

Dex Service 中定义了增加交易对和读取交易对两个接口方法，标记了 `#[write]` 的为写方法，该方法可以改变 Service 状态；标记了 `#[read]` 的为读方法，该方法不能改变 Service 状态；方法的第二个参数必须为 `ServiceContext` 类型，该类型负责管理交易执行的上下文；方法的第三个参数是可选的，定义接口的输入参数，同时需要为该类型实现序列化 trait，目前框架使用的是 json 序列化方案：

```rust
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AddTradePayload {
    pub base_asset: Hash,
    pub counter_party: Hash,
}
```

接口方法最多只能有这 3 个参数。

## 返回值和错误处理

接口方法的返回值统一为 `ProtocolResult<T>` 类型：

```rust
pub type ProtocolResult<T> = Result<T, ProtocolError>;

#[derive(Debug, Constructor, Display)]
#[display(fmt = "[ProtocolError] Kind: {:?} Error: {:?}", kind, error)]
pub struct ProtocolError {
    kind:  ProtocolErrorKind,
    error: Box<dyn Error + Send>,
}

impl From<ProtocolError> for Box<dyn Error + Send> {
    fn from(error: ProtocolError) -> Self {
        Box::new(error) as Box<dyn Error + Send>
    }
}

impl Error for ProtocolError {}
```

每个 Service 定义自己的错误类型，并将该类型转换为 `ProtocolError` 供框架统一处理，以 Dex Service 为例

```rust
#[derive(Debug, Display, From)]
pub enum DexError {
    #[display(fmt = "Parsing payload to json failed {:?}", _0)]
    JsonParse(serde_json::Error),

    IllegalTrade,

    TradeExisted,

    TradeNotExisted,

    OrderOverdue,

    OrderNotExisted,
}

impl std::error::Error for DexError {}

impl From<DexError> for ProtocolError {
    fn from(err: DexError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
```

## 创世配置

如果创世块的世界状态需要包含 Service 的初始状态，可以在 Service 中通过过程宏`#[genesis]` 标注的 `fn init_genesis` 方法来完成。框架在创建创世块时，会调用 Service 中标注了 `#[genesis]` 的方法来完成初始化，该函数最多只有一个。

```rust
#[genesis]
fn init_genesis(&mut self, payload: GenesisPayload) -> ProtocolResult<()> {
    self.validity.set(payload.order_validity)
}
```

## 资源消耗统计：cycle

调用 Service 的接口方法，会消耗一定数量的 cycles，使用过程宏 `#[cycles(amount)]` 标记接口方法，框架会自动扣除 `amount ` 数量的 cycles。

## 事件

使用 `ServiceContext` 的 `fn emit_event` 接口，可以向链外抛出事件信息:

```rust
pub fn emit_event(&self, message: String) -> ProtocolResult<()> ;
```

抛出的事件 message 为 json 序列化的字符串，以 Asset Service 为例：

```rust
let event = TransferEvent {
    asset_id: payload.asset_id,
    from: ctx.get_caller(),
    to: payload.to,
    value: payload.value,
};
let event_json = serde_json::to_string(&event).map_err(AssetError::JsonParse)?;
ctx.emit_event(event_json)
```

## ServiceContext 中的其他方法

ServiceContext 维护交易执行的上下文，通过 ServiceContext 可以获取的信息有：

```rust
// 获取交易哈希
pub fn get_tx_hash(&self) -> Option<Hash>;

// 获取 nonce
pub fn get_nonce(&self) -> Option<Hash>;

// 获取 cycle 价格
pub fn get_cycles_price(&self) -> u64；

// 获取  cycle limit
pub fn get_cycles_limit(&self) -> u64；

// 获取已消耗 cycles 数量
pub fn get_cycles_used(&self) -> u64；

// 获取交易发起方地址
pub fn get_caller(&self) -> Address；

// 获取交易所在区块高度
pub fn get_current_height(&self) -> u64；

// 获取额外信息
pub fn get_extra(&self) -> Option<Bytes>；

// 获取当前区块时间戳
pub fn get_timestamp(&self) -> u64；

// 抛出事件信息
pub fn emit_event(&self, message: String) -> ProtocolResult<()>；
```

## Service 调用

通过 ServiceSDK 提供 `fn write` 和 `fn read` 两个方法，可以调用其他 Service。前者可以改变被调用 Service 的状态，后者为只读调用：

```rust
pub trait ServiceSDK {
    fn read(
        &self,
        ctx: &ServiceContext,
        extra: Option<Bytes>,
        service: &str,
        method: &str,
        payload: &str,
    ) -> ProtocolResult<String>;

    fn write(
        &mut self,
        ctx: &ServiceContext,
        extra: Option<Bytes>,
        service: &str,
        method: &str,
        payload: &str,
    ) -> ProtocolResult<String>;

... ...
}
```

第二个参数 `ServiceContext` 直接传入自身的上下文；第三个参数传入调用的任意附加信息；第四个参数为被调 Service 的名称；第五个参数为调用其他 Service 的接口方法的名称；第五个参数为调用参数 json 序列化后的字符串；

## Hook

每个区块执行前后，框架会分别调用 Service 的 hook_before、hook_after 方法, 这两个方法需分别使用 `#[hook_before]`、`#[hook_after]` 过程宏标记。Service 可借助 hook 功能完成特定逻辑，如 DPoS Service 可在 hook_after 方法中统计候选验证人抵押 token 数量，进行验证人变更等操作；Dex Service 可在 hook_after 方法中对订单进行匹配和成交操作：

```rust
// Hook method in dex service
#[hook_after]
    fn deal(&mut self, params: &ExecutorParams) -> ProtocolResult<()>;
```

## 构造方法

构造方法返回 Service 实例，以 Dex Service 为例：

```rust
#[service]
impl<SDK: 'static + ServiceSDK> DexService<SDK> {
    pub fn new(mut sdk: SDK) -> ProtocolResult<Self> {
        let trades: Box<dyn StoreMap<Hash, Trade>> = sdk.alloc_or_recover_map(TRADES_KEY)?;
        let buy_orders: Box<dyn StoreMap<Hash, Order>> =
            sdk.alloc_or_recover_map(BUY_ORDERS_KEY)?;
        let sell_orders: Box<dyn StoreMap<Hash, Order>> =
            sdk.alloc_or_recover_map(SELL_ORDERS_KEY)?;
        let history_orders: Box<dyn StoreMap<Hash, Order>> =
            sdk.alloc_or_recover_map(HISTORY_ORDERS_KEY)?;
        let validity: Box<dyn StoreUint64> = sdk.alloc_or_recover_uint64(VALIDITY_KEY)?;

        Ok(Self {
            sdk,
            trades,
            buy_orders,
            sell_orders,
            history_orders,
            validity,
        })
    }

... ...
```

## Service 示例

这里有一个功能类似 ERC-20 的 [Asset Service 示例](https://github.com/nervosnetwork/muta/tree/master/built-in-services/asset)，读者可以查看一个 Service 的全貌。更多的 Service 示例，请参考 [Service 示例](./service_eg.md)。

## 下一站

现在你已经对 Service 的组件和开发有了一定的认识，下一步通过学习 [开发一条 Dex 专有链](dex.md) ，你将对 Service 有一个更全面的理解并且学会如何使用 Muta 框架开发自己的区块链。

> 注意：由于框架正在持续的开发过程中，所以框架的 api 有可能发生变动