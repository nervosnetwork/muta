# 内置 Service 说明

## 概述

目前 Muta 框架源代码目前内置了两个 build-in Service：Metadata Service 和 Asset Service。Metadata Service 为起链所比如配置的 Service。其他 Service（包括 Asset Service）为可选配置。

## Metadata Service 

用来存储链的元数据信息，以支持链的运营方在起链前对链的相关信息进行配置。这些元数据包括：

```rust
pub struct Metadata {
    pub chain_id:        Hash,
    pub common_ref:      String, // BLS 签名算法的公共参数
    pub timeout_gap:     u64, // (交易有效期 - 当前区块高度)的最大值
    pub cycles_limit:    u64, // 区块全部交易消耗的 cycles 上限
    pub cycles_price:    u64, // 节点设置的交易打包进区块的最小 cycles_price
    pub interval:        u64, // 区块产生间隔
    pub verifier_list:   Vec<ValidatorExtend>, // 共识验证人列表
    pub propose_ratio:   u64, // 共识 propose 阶段的超时时间与 interval 的比值
    pub prevote_ratio:   u64, // 共识 prevote 阶段的超时时间与 interval 的比值
    pub precommit_ratio: u64, // 共识 precommit 阶段的超时时间与 interval 的比值
    pub brake_ratio: u64 // 共识重发 choke 的超时时间与 interval 的比值
}

pub struct ValidatorExtend {
    pub bls_pub_key: String,
    pub address:        Address,
    pub propose_weight: u32, //出块权重
    pub vote_weight:    u32, // 投票权重
}
```
通过 Metadata Service 可以读取这些信息，接口如下： 

## 接口

### 读取链元数据信息
   
```rust
fn get_metadata(&self, ctx: ServiceContext) -> ProtocolResult<Metadata>；
```

GraphiQL 示例：

```graphql
query get_metadata{
  queryService(
  caller: "016cbd9ee47a255a6f68882918dcdd9e14e6bee1"
  serviceName: "metadata"
  method: "get_metadata"
  payload: ""
  ){
    ret,
    isError
  }
}
```

## Asset Service

Asset service 负责管理链原生资产以及第三方发行资产。

- 资产成为一等公民：加密资产作为区块链的核心，理应成为一等公民。Asset 模块利用 Muta 框架提供的 service 能力，为所有资产提供链级别的支持，为面向资产编程提供支持。
  
- 第三方发行资产： 用户可以使用 Asset 模块发行资产，自定义资产属性和总量等

- 资产与合约交互： 未来可以打通虚拟机和资产模块，为资产的广泛使用提供支持

## 接口

Asset 模块采用类似以太坊 ERC-20 的接口设计，主要包含：

1. 发行资产

```rust
// 资产数据结构
pub struct Asset {
    pub id:     Hash,
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
    pub issuer: Address,
}

// 发行资产接口
// 资产 ID 自动生成，确保唯一
fn create_asset(&mut self, ctx: ServiceContext, payload: CreateAssetPayload) -> ProtocolResult<Asset>;

// 发行资产参数
pub struct CreateAssetPayload {
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
}
```

GraphiQL 示例：

```graphql
mutation create_asset{
  unsafeSendTransaction(inputRaw: {
    serviceName:"asset",
    method:"create_asset",
    payload:"{\"name\":\"Test Coin\",\"symbol\":\"TC\",\"supply\":100000000}",
    timeout:"0x172",
    nonce:"0x9db2d7efe2b61a88827e4836e2775d913a442ed2f9096ca1233e479607c27cf7",
    chainId:"b6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036",
    cyclesPrice:"0x9999",
    cyclesLimit:"0x9999"
  }, inputPrivkey: "0x30269d47fcf602b889243722b666881bf953f1213228363d34cf04ddcd51dfd2"
  )
}
```

2. 查询资产信息

```rust
// 查询接口
fn get_asset(&self, ctx: ServiceContext, payload: GetAssetPayload) -> ProtocolResult<Asset>；

// 查询参数
pub struct GetAssetPayload {
    pub id: Hash, // 资产 ID
}
```

GraphiQL 示例：

```graphql 
query get_asset{
  queryService(
  caller: "016cbd9ee47a255a6f68882918dcdd9e14e6bee1"
  serviceName: "asset"
  method: "get_asset"
  payload: "{\"id\": \"5f1364a8e6230f68ccc18bc9d1000cedd522d6d63cef06d0062f832bdbe1a78a\"}"
  ){
    ret,
    isError
  }
}
```

3. 转账

```rust
// 转账接口
fn transfer(&mut self, ctx: ServiceContext, payload: TransferPayload) -> ProtocolResult<()>；

// 转账参数
pub struct TransferPayload {
    pub asset_id: Hash,
    pub to:       Address,
    pub value:    u64,
}
```

GraphiQL 示例：

```graphql
mutation transfer{
  unsafeSendTransaction(inputRaw: {
    serviceName:"asset",
    method:"transfer",
    payload:"{\"asset_id\":\"5f1364a8e6230f68ccc18bc9d1000cedd522d6d63cef06d0062f832bdbe1a78a\",\"to\":\"f8389d774afdad8755ef8e629e5a154fddc6325a\", \"value\":10000}",
    timeout:"0x289",
    nonce:"0x9db2d7efe2b61a28827e4836e2775d913a442ed2f9096ca1233e479607c27cf7",
    chainId:"b6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036",
    cyclesPrice:"0x9999",
    cyclesLimit:"0x9999",
    }, inputPrivkey: "0x30269d47fcf602b889243722b666881bf953f1213228363d34cf04ddcd51dfd2"
  )
}
```

4. 查询余额

```rust
// 查询接口
fn get_balance(&self, ctx: ServiceContext, payload: GetBalancePayload) -> ProtocolResult<GetBalanceResponse> 

// 查询参数
pub struct GetBalancePayload {
    pub asset_id: Hash,
    pub user:     Address,
}

// 返回值
pub struct GetBalanceResponse {
    pub asset_id: Hash,
    pub user:     Address,
    pub balance:  u64,
}
```

GraphiQL 示例： 

```graphql
query get_balance{
  queryService(
  caller: "016cbd9ee47a255a6f68882918dcdd9e14e6bee1"
  serviceName: "asset"
  method: "get_balance"
  payload: "{\"asset_id\": \"5f1364a8e6230f68ccc18bc9d1000cedd522d6d63cef06d0062f832bdbe1a78a\", \"user\": \"016cbd9ee47a255a6f68882918dcdd9e14e6bee1\"}"
  ){
    ret,
    isError
  }
}
```

5. 批准额度

```rust
// 批准接口
fn approve(&mut self, ctx: ServiceContext, payload: ApprovePayload) -> ProtocolResult<()>;

// 批准参数
pub struct ApprovePayload {
    pub asset_id: Hash,
    pub to:       Address,
    pub value:    u64,
}
```

GraphiQL 示例： 

```graphql
  unsafeSendTransaction(inputRaw: {
    serviceName:"asset",
    method:"approve",
    payload:"{\"asset_id\":\"5f1364a8e6230f68ccc18bc9d1000cedd522d6d63cef06d0062f832bdbe1a78a\",\"to\":\"f8389d774afdad8755ef8e629e5a154fddc6325a\", \"value\":10000}",
    timeout:"0x378",
    nonce:"0x9db2d7efe2b61a28827e4836e2775d913a442ed2f9096ca1233e479607c27cf7",
    chainId:"b6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036",
    cyclesPrice:"0x9999",
    cyclesLimit:"0x9999",
    }, inputPrivkey: "0x30269d47fcf602b889243722b666881bf953f1213228363d34cf04ddcd51dfd2"
  )
}
```

6. 授权转账

```rust
// 接口
fn transfer_from(&mut self, ctx: ServiceContext, payload: TransferFromPayload) -> ProtocolResult<()>；

// 参数
pub struct TransferFromPayload {
    pub asset_id:  Hash,
    pub sender:    Address,
    pub recipient: Address,
    pub value:     u64,
}
```

GraphiQL 示例：

```graphql
mutation transfer_from{
  unsafeSendTransaction(inputRaw: {
    serviceName:"asset",
    method:"transfer_from",
    payload:"{\"asset_id\":\"5f1364a8e6230f68ccc18bc9d1000cedd522d6d63cef06d0062f832bdbe1a78a\",\"sender\":\"016cbd9ee47a255a6f68882918dcdd9e14e6bee1\", \"recipient\":\"fffffd774afdad8755ef8e629e5a154fddc6325a\", \"value\":5000}",
    timeout:"0x12c",
    nonce:"0x9db2d7efe2b61a28827e4836e2775d913a442ed2f9096ca1233e479607c27cf7",
    chainId:"b6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036",
    cyclesPrice:"0x9999",
    cyclesLimit:"0x9999",
    }, inputPrivkey: "0x45c56be699dca666191ad3446897e0f480da234da896270202514a0e1a587c3f"
  )
}
```

7. 查询限额

```rust
// 查询接口
fn get_allowance(&self, ctx: ServiceContext, payload: GetAllowancePayload) -> ProtocolResult<GetAllowanceResponse>；

// 查询参数
pub struct GetAllowancePayload {
    pub asset_id: Hash,
    pub grantor:  Address,
    pub grantee:  Address,
}

// 返回值
pub struct GetAllowanceResponse {
    pub asset_id: Hash,
    pub grantor:  Address,
    pub grantee:  Address,
    pub value:    u64,
}
```

GraphiQL 示例：

```graphql
query get_allowance{
  queryService(
  caller: "016cbd9ee47a255a6f68882918dcdd9e14e6bee1"
  serviceName: "asset"
  method: "get_allowance"
  payload: "{\"asset_id\": \"5f1364a8e6230f68ccc18bc9d1000cedd522d6d63cef06d0062f832bdbe1a78a\", \"grantor\": \"016cbd9ee47a255a6f68882918dcdd9e14e6bee1\", \"grantee\": \"f8389d774afdad8755ef8e629e5a154fddc6325a\"}"
  ){
    ret,
    isError
  }
}
```