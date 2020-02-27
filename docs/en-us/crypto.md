# 密码学

## Muta 中使用到的密码学算法

Muta 使用 secp256k1 参数以实现椭圆曲线数字签名算法(ECDSA) 的功能

##  如何生成生成地址

记私钥 p*r*, 对应地址函数 A(p*r*)，则有：

A(p*r*) = Bit<sub>0..160</sub>(Keccak(ECDSAPUBKEY(p*r*)))

Muta 的地址对应成一个 160bit 的值，生成的过程详细描述为：

1. 私钥 -> 公钥：私钥通过 ECDSA 公钥生成算法转换为公钥
2. 公钥 -> 哈希：公钥通过 Keccak 函数转换哈希值
3. 哈希 -> 地址：截取前 160bit 哈希值

## 如何签名交易

记私钥 p*r*，待签名消息 x，对应有签名函数 S(x)，则有：

S(x) = ECDSASIGN(Keccak(RLP(x)), p*r*)

由于使用 spec256k1，因此签名对应成一个 512bit 的值，签名过程消息描述为：

1. 消息 -> 序列化消息：消息通过 RLP 序列化，消息序列为
   1. chainID
   2. cyclesLimit
   3. cyclesPrice
   4. nonce
   5. method
   6. service
   7. payload
   8. timeout
2. 序列化消息 -> 哈希：序列化消息通过 Keccak 函数转换哈希值
3. 哈希 -> 签名：哈希值通过 ECDSA 签名算法生成签名

下面的例子，将使用伪代码描述签名的过程

```typescript
import { encode as RLPEncode } from 'rlp';
import createKeccakHash from 'keccak'
import { sign as ECDSASign } from 'scep256k1';

const tx = [
  chainId, // '0xb6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036'
  cyclesLimit, // '0xffff'
  cyclesPrice, // '0xffff'
  nonce, // '0x0000000000000000000000000000000000000000000000000000000000000000'
  method, // 'a_service_method' 
  service, // 'a_service_name'
  payload, // 'a_method_payload'
  timeout, // '0xffff' // => current_block_height + timeout_gap - 1
];

const encodedMessage = RLPEncode(tx);
const hash = createKeccakHash('keccak256')
		.update(encodedMessage)
		.digest();
const signature = ECDSASign(hash, privateKey);
```