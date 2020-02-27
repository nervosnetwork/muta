# 网络设计

## 当前目标

基于 [tentacle crate](https://github.com/nervosnetwork/p2p) 实现一个简单的可工作的 P2P 网络，主要功能如下：

- 节点身份

  - PeerID: secp256k1 的公钥派生出的 ID，[tentacle-secio](https://crates.io/crates/tentacle-secio)
  - Address: MultiAddress [REF](https://multiformats.io/multiaddr)，只支持 TCP

- 节点发现

  - bootstrap，[tentacle-discovery](https://crates.io/crates/tentacle-discovery)

- 节点质量维护

  - ping，[tentacle-ping](https://crates.io/crates/tentacle-ping)，超时断开

- 节点持久化

  - 基于文件的简易持久化，服务退出时，将保存节点信息，默认关闭

- 消息广播以及单播

  - 基础的广播服务，以及基于 secp256k1 公钥地址的单播

- 消息加密传输

  - 基于 [tentacle-secio](https://crates.io/crates/tentacle-secio)

- 其他

  - 消息优先级: 使用 tentacle 自带的消息发送优先级，目前只有两种，High 和 Normal
  - 消息压缩: 使用 snappy
  - 消息处理: 基于 handler 注册形式，由各个模块自定义接受消息处理逻辑

## 消息收发

### 节点消息端 (Endpoint)

节点通过注册消息端地址对外暴露服务，实现消息接受及处理。目前提供三种类型的地址：

#### Gossip

```text
/gossip/[service_name]/[message_name]
```

消息单向广播以及单播

#### RPC Call

```text
/rpc_call/[service_name]/[message_name]
```

#### RPC Response

```text
/rpc_resp/[service_name]/[message_name]
```

RPC 用于节点之间的消息交互通信，RPC Call 发送请求，RPC Response 返回。

### 消息序列化

序列化采用 protobuf ，消息需要实现 MessageCodec trait 。

```rust
#[async_trait]
pub trait MessageCodec: Sized + Send + Debug + 'static {
    async fn encode(&mut self) -> ProtocolResult<Bytes>;

    async fn decode(bytes: Bytes) -> ProtocolResult<Self>;
}
```

目前针对实现了 serde Serialize 和 Deserialize trait 的消息自动实现了 MessageCodec ，
采用 bincode 作为中间序列化过渡。

### 消息处理

消息处理需要实现 MessageHandler trait

```rust
#[async_trait]
pub trait MessageHandler: Sync + Send + 'static {
    type Message: MessageCodec;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()>;
}
```

### 消息处理逻辑注册

完成上述实现之后，可通过如下接口，完成消息逻辑处理的注册。

```rust
pub fn register_endpoint_handler<M>(
    &mut self,
    end: &str,
    handler: Box<dyn MessageHandler<Message = M>>,
) -> ProtocolResult<()>
where
    M: MessageCodec;

pub fn register_rpc_response<M>(&mut self, end: &str) -> ProtocolResult<()>
where
    M: MessageCodec;
```

`Gossip` 和 `RPC Call` 都需要通过 `register_endpoint_handler` 完成注册，
而 `RPC Response` 需要通过 `register_rpc_response` 完成注册。

未来计划将 `RPC Response` 注册去掉。

`end` 即签名提到的节点消息端 `Endpoint` 缩写。

### 消息的发送

```rust
#[async_trait]
pub trait Gossip: Send + Sync {
    async fn broadcast<M>(&self, cx: Context, end: &str, msg: M, p: Priority) -> ProtocolResult<()>
    where
        M: MessageCodec;

    async fn users_cast<M>(
        &self,
        cx: Context,
        end: &str,
        users: Vec<UserAddress>,
        msg: M,
        p: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec;
}

#[async_trait]
pub trait Rpc: Send + Sync {
    async fn call<M, R>(&self, ctx: Context, end: &str, msg: M, pri: Priority) -> ProtocolResult<R>
    where
        M: MessageCodec,
        R: MessageCodec;

    async fn response<M>(&self, cx: Context, end: &str, msg: M, p: Priority) -> ProtocolResult<()>
    where
        M: MessageCodec;
}
```

如上述定义，网路服务实例化后，可通过调用 `handle()` 获取一个网络服务引用，该
`handle` 实现了上述的接口，同时实现了 `Clone`。各模块可以通过它来完成消息的
发送。

注意：`UserAddress` 目前同 `tentacle-secio` 提供的 secp256k1 公钥绑定。
