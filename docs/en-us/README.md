<p align="center">
  <a href="https://github.com/nervosnetwork/muta">
    <img src="./static/muta-logo1.png" width="270">
  </a>
  <h3 align="center">Build your own blockchain，today</h3>
  <p align="center">
    <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-green.svg"></a>
    <a href="https://github.com/nervosnetwork/muta"><img src="https://img.shields.io/badge/Code-Github-brightgreen.svg"></a>
    <a href="http://makeapullrequest.com"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg"></a>
     <a href="https://discord.gg/QXkFT88"><img src="https://img.shields.io/discord/674846745607536651?logo=discord"
    alt="chat on Discord"></a>
    <a href="https://github.com/nervosnetwork/muta"><img src="https://img.shields.io/github/stars/nervosnetwork/muta.svg?style=social"></a>
    <a href="https://github.com/nervosnetwork/muta"><img src="https://img.shields.io/github/forks/nervosnetwork/muta.svg?style=social"></a>
  </p>
  <p align="center">
     Developed by Nervos<br>
  </p>
</p>

## What is Muta？

Muta is a highly customizable high-performance blockchain framework. It has a built-in BFT-like consensus algorithm "Overlord" with high throughput and low latency, and it can also support different virtual machines, including CKB-VM, EVM, and WASM. Muta has interoperability across VMs. Different virtual machines can be used in a Muta-based blockchain at the same time. Developed by the Nervos team, Muta is designed to allow anyone in the world to build their own blockchain while enjoying the security and finality brought by Nervos CKB.

Developers can customize PoA, PoS or DPoS chains based on Muta, and use different economic models and governance models. Developers can also develop different application chains (such as DEX chains) based on Muta to implement a specific business logic.

Muta's core design philosophy is to make the development of a blockchain state transition as flexible and simple as possible, which means that while reducing the obstacles to build high-performance blockchains, it still maximizes its flexibility to facilitate developers to customize their business logic. Therefore, as a highly customizable high-performance blockchain framework, Muta provides a basic core component that a blockchain system needs, and developers can customize the functional parts of the chain freely.

## Getting Started!

Quickly build a simple chain and try some simple interaction, please refer to [Quick Start](getting_started.md)。

## The basic core component Muta provided
 
Muta provided all the core components needed to build a blockchain:

* [Transaction Pool](transaction_pool.md)
* [P2P Network](network.md)
* [Consensus](overlord.md)
* [Storage](storage.md)

## Customizable Part

Developers can customize the functional parts of the chain by developing Services.

Service is an abstraction layer for extension in Muta framework. Users can define block management, add VMs, etc. based on Service. Each Service, as a relatively independent logical component, can implement its specific function, and at the same time, different services can directly interact with each other, so that more complex functional logic can be constructed. More flexible is that services from different chains can also be reused, which makes it easier for developers to build their own functional modules.

We provide detailed service development guides and some service examples.

* [Service Development Guide](service_dev.md)
* [Service Examples](service_eg.md)
* [Develop a DEX Chain](dex.md)

## Who is using Muta？

Muta powers some open source projects.

<p align="left">
  <a href="https://github.com/nervosnetwork/muta">
    <img src="./static/user/s_huobichain.jpg" width="150">
  </a>
</p>

Is your project using Muta? Edit this page with a Pull Request to add your logo.:tada:

## Contribute ![PRs](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)

#### Pull Request

- :fork_and_knife:Fork it!
- :twisted_rightwards_arrows:Create your branch: `git checkout -b new-branch`
- :wrench:Make your changes
- :memo:Commit your changes: `git commit -am 'Add some feature'`
- :rocket:Push to the branch: `git push origin new-branch`
- :tada:Submit a pull request

#### Issue

- Submit an [issue](https://github.com/nervosnetwork/muta/issues)。Any helpful suggestions are welcomed.:stuck_out_tongue_winking_eye:

#### Translation

If you are willing to participate in the translation of the document, please go to the top of the untranslated English document page and click edit on GitHub to find the source file, modify the source file directly, and submit the PR. The steps are the same as the PR step above.

## Join us!

As an entrepreneurial team, we are still recruiting excellent developers: heart :, and we will continue to recruit until the end of the world, front-end, back-end, blockchain development engineers, etc. For more details, please refer to [Job Description](https://github.com/zhouyun-zoe/Workflow/issues/3).
