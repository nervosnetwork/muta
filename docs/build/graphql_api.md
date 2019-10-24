# Muta GraphQL API

>[GraphQL](https://graphql.org) is a query language for APIs and a runtime for fulfilling those queries with your existing data.
GraphQL provides a complete and understandable description of the data in your API,
gives clients the power to ask for exactly what they need and nothing more,
makes it easier to evolve APIs over time, and enables powerful developer tools.

<details>
  <summary><strong>Table of Contents</strong></summary>

  * [Query](#query)
  * [Mutation](#mutation)
  * [Objects](#objects)
    * [Epoch](#epoch)
    * [EpochHeader](#epochheader)
  * [Inputs](#inputs)
    * [CarryingAsset](#carryingasset)
    * [InputCallAction](#inputcallaction)
    * [InputDeployAction](#inputdeployaction)
    * [InputRawTransaction](#inputrawtransaction)
    * [InputReadonly](#inputreadonly)
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

get latest epoch

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>Readonly</strong></td>
<td valign="top"><a href="#string">String</a>!</td>
<td>

execute readonly call to contract

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputReadonly</td>
<td valign="top"><a href="#inputreadonly">InputReadonly</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>getBalance</strong></td>
<td valign="top"><a href="#balance">Balance</a>!</td>
<td>

Get account balance

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
<td colspan="2" valign="top"><strong>sendCallTransaction</strong></td>
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
<td valign="top"><a href="#inputcallaction">InputCallAction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputEncryption</td>
<td valign="top"><a href="#inputtransactionencryption">InputTransactionEncryption</a>!</td>
<td></td>
</tr>
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

Send deployment contract transactions to the blockchain.

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

Don't use it! This is just for development testing.

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

Don't use it! This is just for development testing.

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
<tr>
<td colspan="2" valign="top"><strong>sendUnsafeCallTransaction</strong> ⚠️</td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td>
<p>⚠️ <strong>DEPRECATED</strong></p>
<blockquote>

Don't use it! This is just for development testing.

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
<td valign="top"><a href="#inputcallaction">InputCallAction</a>!</td>
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

Epoch

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
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>orderedTxHashes</strong></td>
<td valign="top">[<a href="#hash">Hash</a>!]!</td>
<td></td>
</tr>
</tbody>
</table>

### EpochHeader

Epoch header

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
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>epochId</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>preHash</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>timestamp</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>orderRoot</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>confirmRoot</strong></td>
<td valign="top">[<a href="#hash">Hash</a>!]!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>stateRoot</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>receiptRoot</strong></td>
<td valign="top">[<a href="#hash">Hash</a>!]!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>cyclesUsed</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>proposer</strong></td>
<td valign="top"><a href="#address">Address</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>validatorVersion</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td></td>
</tr>
</tbody>
</table>

## Inputs

### CarryingAsset

carrying asset

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
<td colspan="2" valign="top"><strong>amount</strong></td>
<td valign="top"><a href="#balance">Balance</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>assetId</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### InputCallAction

input call action.

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
<td colspan="2" valign="top"><strong>contract</strong></td>
<td valign="top"><a href="#address">Address</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>method</strong></td>
<td valign="top"><a href="#string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>args</strong></td>
<td valign="top">[<a href="#string">String</a>!]!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>carryingAsset</strong></td>
<td valign="top"><a href="#carryingasset">CarryingAsset</a></td>
<td></td>
</tr>
</tbody>
</table>

### InputDeployAction

input deploy action.

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
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>contractType</strong></td>
<td valign="top"><a href="#contracttype">ContractType</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### InputRawTransaction

input raw transaction.

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
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>feeCycle</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>feeAssetId</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>nonce</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>timeout</strong></td>
<td valign="top"><a href="#uint64">Uint64</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### InputReadonly

input readonly params.

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
<td colspan="2" valign="top"><strong>epochId</strong></td>
<td valign="top"><a href="#uint64">Uint64</a></td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>contract</strong></td>
<td valign="top"><a href="#address">Address</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>method</strong></td>
<td valign="top"><a href="#string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>args</strong></td>
<td valign="top">[<a href="#string">String</a>!]!</td>
<td></td>
</tr>
</tbody>
</table>

### InputTransactionEncryption

input signature, hash, pubkey

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
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>pubkey</strong></td>
<td valign="top"><a href="#bytes">Bytes</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>signature</strong></td>
<td valign="top"><a href="#bytes">Bytes</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### InputTransferAction

input transfer action.

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
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>carryingAssetId</strong></td>
<td valign="top"><a href="#hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>receiver</strong></td>
<td valign="top"><a href="#address">Address</a>!</td>
<td></td>
</tr>
</tbody>
</table>

## Enums

### ContractType

<table>
<thead>
<th align="left">Value</th>
<th align="left">Description</th>
</thead>
<tbody>
<tr>
<td valign="top"><strong>ASSET</strong></td>
<td></td>
</tr>
<tr>
<td valign="top"><strong>APP</strong></td>
<td></td>
</tr>
<tr>
<td valign="top"><strong>LIBRARY</strong></td>
<td></td>
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

Keccak hash of hex string

### String

### Uint64

Uint64

