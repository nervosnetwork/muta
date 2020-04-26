<p align="center">
  <a href="https://github.com/nervosnetwork/muta">
    <img src="https://github.com/nervosnetwork/muta-docs/blob/master/static/muta-logo1.png" width="270">
  </a>
  <h3 align="center">让世界上任何一个人都可以搭建属于他们自己的区块链</h3>
  <p align="center">
    <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-green.svg"></a>
    <a href="https://github.com/nervosnetwork/muta/blob/master/rust-toolchain"><img src="https://img.shields.io/badge/rustc-nightly-informational.svg"></a>
    <a href="https://travis-ci.com/nervosnetwork/muta"><img src="https://travis-ci.com/nervosnetwork/muta.svg?branch=master"></a>
     <a href="https://discord.gg/QXkFT88"><img src="https://img.shields.io/discord/674846745607536651?logo=discord"
    alt="chat on Discord"></a>
    <a href="https://github.com/nervosnetwork/muta"><img src="https://img.shields.io/github/stars/nervosnetwork/muta.svg?style=social"></a>
    <a href="https://github.com/nervosnetwork/muta"><img src="https://img.shields.io/github/forks/nervosnetwork/muta.svg?style=social"></a>
  </p>
  <p align="center">
     由 Nervos 团队开发<br>
  </p>
</p>

[English](./README.md) | 简体中文

## 什么是 Muta？

Muta 是一个高度可定制的高性能区块链框架。它内置了具有高吞吐量和低延迟特性的类 BFT 共识算法「Overlord」，并且可以支持不同的虚拟机，包括 CKB-VM、EVM 和 WASM。Muta 具有跨 VM 的互操作性，不同的虚拟机可以同时在一条基于 Muta 搭建的区块链中使用。Muta 由 Nervos 团队开发，旨在让世界上任何一个人都可以搭建属于他们自己的区块链，同时享受 Nervos CKB 所带来的安全性和最终性。

开发者可以基于 Muta 定制开发 PoA、PoS 或者 DPoS 链，并且可以使用不同的经济模型和治理模型进行部署。开发者也可以基于 Muta 来开发不同的应用链（例如 DEX 链），以实现某种特定的业务逻辑。

Muta 的核心理念是使一个区块链状态转换的开发尽可能的灵活和简便，也就是说在降低开发者搭建高性能区块链障碍的同时，仍然最大限度地保证其灵活性以方便开发者可以自由定制他们的协议。因此，作为一个高度可定制的高性能区块链框架，Muta 提供了一个区块链系统需要有的基础核心组件，开发者可以自由定制链的功能部分。

## 快速开始！

[Muta 文档网站](https://nervosnetwork.github.io/muta-docs/)

快速搭建一条简单的链并尝试简单的交互，请参考[快速开始](https://nervosnetwork.github.io/muta-docs/#/getting_started.md)。

## Muta 提供哪些基础核心组件？

Muta 框架提供了搭建一个分布式区块链网络所需的全部核心组件：

* [交易池](https://nervosnetwork.github.io/muta-docs/#/transaction_pool.md)
* [P2P 网络](https://nervosnetwork.github.io/muta-docs/#/network.md)
* [共识](https://nervosnetwork.github.io/muta-docs/#/overlord.md)
* [存储](https://nervosnetwork.github.io/muta-docs/#/storage.md)

## 开发者需要自己实现哪些部分？

开发者可以通过开发 Service 来定制链的功能部分。

Service 是 Muta 框架中用于扩展的抽象层，用户可以基于 Service 定义区块治理、添加 VM 等等。每一个 Service 作为一个相对独立的逻辑化组件，可以实现其特定的功能，同时，不同的 Service 之间又可以直接进行交互，从而可以构建更为复杂的功能逻辑。更为灵活的是，不同链的 Service 还可以复用，这使得开发者们可以更为轻松的搭建自己的功能模块。

我们提供了详细的 Service 开发指南，以及一些 Service 示例。

* [Service 开发指南](https://nervosnetwork.github.io/muta-docs/#service_dev.md)
* [Service 示例](https://nervosnetwork.github.io/muta-docs/#service_eg.md)

## 谁在使用 Muta？

<p align="left">
  <a href="https://github.com/HuobiGroup/huobi-chain">
    <img src="https://github.com/nervosnetwork/muta-docs/blob/master/static/user/s_huobichain.jpg" width="150">
  </a>
</p>

您的项目使用的是 Muta 吗？欢迎在这里添加您项目的 logo 和链接，请点击顶部的 `Edit Document` ，修改本文档的相关内容，并提交 Pull Request 即可:tada:

## 贡献 ![PRs](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)

如何贡献请参考 [CONTRIBUTING.md](CONTRIBUTING.md)，Security Policy 请参考 [SECURITY.md](SECURITY.md)。
