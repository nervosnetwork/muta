# Muta GraphQL API

>[GraphQL](https://graphql.org) is a query language for APIs and a runtime for fulfilling those queries with your existing data.
GraphQL provides a complete and understandable description of the data in your API,
gives clients the power to ask for exactly what they need and nothing more,
makes it easier to evolve APIs over time, and enables powerful developer tools.

Muta has embeded a [Graph*i*QL](https://github.com/graphql/graphiql) for checking and calling API. Started a muta
node, and then try open http://127.0.0.1:8000/graphiql in the browser.


<details>
  <summary><strong>Table of Contents</strong></summary>

  * [Query](#query)
  * [Mutation](#mutation)
  * [Objects](#objects)
    * [Epoch](#epoch)
    * [EpochHeader](#epochheader)
  * [Inputs](#inputs)
    * [InputDeployAction](#inputdeployaction)
    * [InputRawTransaction](#inputrawtransaction)
    * [InputTransactionEncryption](#inputtransactionencryption)
    * [InputTransferAction](#inputtransferaction)
  * [Enums](#enums)
    * [ContractType](#contracttype)
  * [Scalars](#scalars)
    * [Address](#address)
    * [Balance](#balance)
    * [Boolean](#boolean)
    * [Bytes](#bytes)
    * [Hash](#hash)
    * [String](#string)
    * [Uint64](#uint64)

</details>

## Query
<table>
<thead>
<tr>
<th align="left">Field</th>
<th align="right">Argument</th>
<th align="left">Type</th>
<th align="left">Description</th>
</tr>
</thead>
<tbody>
<tr>
<td colspan="2" valign="top"><strong>getLatestEpoch</strong></td>
<td valign="top"><a href="#epoch">Epoch</a>!</td>
<td>

Get the latest epoch

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">epochId</td>
<td valign="top"><a href="#uint64">Uint64</a></td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>getBalance</strong></td>
<td valign="top"><a href="#balance">Balance</a>!</td>
<td>

Get the asset balance of an account

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">address</td>
<td valign="top"><a href="#address">Address</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">id</td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

The asset id. Asset is the first-class in muta, this means that your assets can be more than one in muta, and the UDT(User Defined Token) will be supported in the future

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">epochId</td>
<td valign="top"><a href="#uint64">Uint64</a></td>
<td></td>
</tr>
</tbody>
</table>

## Mutation
<table>
<thead>
<tr>
<th align="left">Field</th>
<th align="right">Argument</th>
<th align="left">Type</th>
<th align="left">Description</th>
</tr>
</thead>
<tbody>
<tr>
<td colspan="2" valign="top"><strong>sendTransferTransaction</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

Send a transfer transaction to the blockchain.

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputRaw</td>
<td valign="top"><a href="#inputrawtransaction">InputRawTransaction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputAction</td>
<td valign="top"><a href="#inputtransferaction">InputTransferAction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputEncryption</td>
<td valign="top"><a href="#inputtransactionencryption">InputTransactionEncryption</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>sendDeployTransaction</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

Send deployment contract transaction to the blockchain.

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputRaw</td>
<td valign="top"><a href="#inputrawtransaction">InputRawTransaction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputAction</td>
<td valign="top"><a href="#inputdeployaction">InputDeployAction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputEncryption</td>
<td valign="top"><a href="#inputtransactionencryption">InputTransactionEncryption</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>sendUnsafeTransferTransaction</strong> ⚠️</td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>
<p>⚠️ <strong>DEPRECATED</strong></p>
<blockquote>

DON'T use it in production! This is just for development.

</blockquote>
</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputRaw</td>
<td valign="top"><a href="#inputrawtransaction">InputRawTransaction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputAction</td>
<td valign="top"><a href="#inputtransferaction">InputTransferAction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputPrivkey</td>
<td valign="top"><a href="#bytes">Bytes</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>sendUnsafeDeployTransaction</strong> ⚠️</td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>
<p>⚠️ <strong>DEPRECATED</strong></p>
<blockquote>

DON'T use it in production! This is just for development.

</blockquote>
</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputRaw</td>
<td valign="top"><a href="#inputrawtransaction">InputRawTransaction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputAction</td>
<td valign="top"><a href="#inputdeployaction">InputDeployAction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputPrivkey</td>
<td valign="top"><a href="#bytes">Bytes</a>!</td>
<td></td>
</tr>
</tbody>
</table>

## Objects

### Epoch

Epoch is a single digital record created within a blockchain. Each epoch contains a record of the previous Epoch, and when linked together these become the “chain”.An epoch is always composed of header and body.

<table>
<thead>
<tr>
<th align="left">Field</th>
<th align="right">Argument</th>
<th align="left">Type</th>
<th align="left">Description</th>
</tr>
</thead>
<tbody>
<tr>
<td colspan="2" valign="top"><strong>header</strong></td>
<td valign="top"><a href="#epochheader">EpochHeader</a>!</td>
<td>

The header section of an epoch

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>orderedTxHashes</strong></td>
<td valign="top">[<a href="#hash">Hash</a>!]!</td>
<td>

The body section of an epoch

</td>
</tr>
</tbody>
</table>

### EpochHeader

An epoch header is like the metadata of an epoch.

<table>
<thead>
<tr>
<th align="left">Field</th>
<th align="right">Argument</th>
<th align="left">Type</th>
<th align="left">Description</th>
</tr>
</thead>
<tbody>
<tr>
<td colspan="2" valign="top"><strong>chainId</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

Identifier of a chain in order to prevent replay attacks across channels 

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>epochId</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td>

Known as the block height like other blockchain

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>preHash</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

The hash of the serialized previous epoch

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>timestamp</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td>

A timestamp that records when the epoch was created

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>orderRoot</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

The merkle root of ordered transactions

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>confirmRoot</strong></td>
<td valign="top">[<a href="#hash">Hash</a>!]!</td>
<td>

The merkle roots of all the confirms

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>stateRoot</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

The merkle root of state root

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>receiptRoot</strong></td>
<td valign="top">[<a href="#hash">Hash</a>!]!</td>
<td>

The merkle roots of receipts

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>cyclesUsed</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td>

The sum of all transactions costs

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>proposer</strong></td>
<td valign="top"><a href="#address">Address</a>!</td>
<td>

The address descirbed who packed the epoch

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>validatorVersion</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td>

The version of validator is designed for cross chain

</td>
</tr>
</tbody>
</table>

## Inputs

### InputDeployAction

The deploy transfer transaction

<table>
<thead>
<tr>
<th colspan="2" align="left">Field</th>
<th align="left">Type</th>
<th align="left">Description</th>
</tr>
</thead>
<tbody>
<tr>
<td colspan="2" valign="top"><strong>code</strong></td>
<td valign="top"><a href="#bytes">Bytes</a>!</td>
<td>

Encoded contract code

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>contractType</strong></td>
<td valign="top"><a href="#contracttype">ContractType</a>!</td>
<td>

The type of contract

</td>
</tr>
</tbody>
</table>

### InputRawTransaction

There was many types of transaction in muta, A transaction often require computing resources or write data to chain,these resources are valuable so we need to pay some token for them.InputRawTransaction describes information above

<table>
<thead>
<tr>
<th colspan="2" align="left">Field</th>
<th align="left">Type</th>
<th align="left">Description</th>
</tr>
</thead>
<tbody>
<tr>
<td colspan="2" valign="top"><strong>chainId</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

Identifier of the chain.

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>feeCycle</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td>

Mostly like the gas limit in Ethereum, describes the fee that you are willing to pay the highest price for the transaction

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>feeAssetId</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

asset type

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>nonce</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

Every transaction has its own id, unlike Ethereum's nonce,the nonce in muta is an hash

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>timeout</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td>

For security and performance reasons, muta will only deal with trade request over a period of time,the `timeout` should be `timeout > current_epoch_height` and `timeout < current_epoch_height + timeout_gap`,the `timeout_gap` generally equal to 20.

</td>
</tr>
</tbody>
</table>

### InputTransactionEncryption

Signature of the transaction

<table>
<thead>
<tr>
<th colspan="2" align="left">Field</th>
<th align="left">Type</th>
<th align="left">Description</th>
</tr>
</thead>
<tbody>
<tr>
<td colspan="2" valign="top"><strong>txHash</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

The digest of the transaction

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>pubkey</strong></td>
<td valign="top"><a href="#bytes">Bytes</a>!</td>
<td>

The public key of transfer

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>signature</strong></td>
<td valign="top"><a href="#bytes">Bytes</a>!</td>
<td>

The signature of the transaction

</td>
</tr>
</tbody>
</table>

### InputTransferAction

The action of transfer transaction

<table>
<thead>
<tr>
<th colspan="2" align="left">Field</th>
<th align="left">Type</th>
<th align="left">Description</th>
</tr>
</thead>
<tbody>
<tr>
<td colspan="2" valign="top"><strong>carryingAmount</strong></td>
<td valign="top"><a href="#balance">Balance</a>!</td>
<td>

The amount of the transfer

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>carryingAssetId</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>

The asset of of the transfer

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>receiver</strong></td>
<td valign="top"><a href="#address">Address</a>!</td>
<td>

The receiver of the transfer

</td>
</tr>
</tbody>
</table>

## Enums

### ContractType

According to different purposes, muta has many contract type

<table>
<thead>
<th align="left">Value</th>
<th align="left">Description</th>
</thead>
<tbody>
<tr>
<td valign="top"><strong>ASSET</strong></td>
<td>

Asset contract often use for creating User Defined Asset(also known as UDT(User Defined Token))

</td>
</tr>
<tr>
<td valign="top"><strong>APP</strong></td>
<td>

App contract often use for creating DAPP(Decentralized APPlication) 

</td>
</tr>
<tr>
<td valign="top"><strong>LIBRARY</strong></td>
<td>

Library contract often providing reusable and immutable function

</td>
</tr>
</tbody>
</table>

## Scalars

### Address

21 bytes of account address, the first bytes of which is the identifier.

### Balance

uint256

### Boolean

### Bytes

Bytes corresponding hex string.

### Hash

The output digest of Keccak hash function

### String

### Uint64

Uint64

