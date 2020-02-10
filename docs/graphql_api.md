# Muta GraphQL API


>[GraphQL](https://graphql.org) is a query language for APIs and a runtime for fulfilling those queries with your existing data.
GraphQL provides a complete and understandable description of the data in your API,
gives clients the power to ask for exactly what they need and nothing more,
makes it easier to evolve APIs over time, and enables powerful developer tools.

Muta has embeded a [Graph**i**QL](https://github.com/graphql/graphiql) for checking and calling API. Started a the Muta
node, and then try open http://127.0.0.1:8000/graphiql in the browser.


<details>
  <summary><strong>Table of Contents</strong></summary>

  * [Query](#query)
  * [Mutation](#mutation)
  * [Objects](#objects)
    * [Block](#block)
    * [BlockHeader](#blockheader)
    * [Event](#event)
    * [ExecResp](#execresp)
    * [Proof](#proof)
    * [Receipt](#receipt)
    * [ReceiptResponse](#receiptresponse)
    * [SignedTransaction](#signedtransaction)
    * [Validator](#validator)
  * [Inputs](#inputs)
    * [InputRawTransaction](#inputrawtransaction)
    * [InputTransactionEncryption](#inputtransactionencryption)
  * [Scalars](#scalars)
    * [Address](#address)
    * [Boolean](#boolean)
    * [Bytes](#bytes)
    * [Hash](#hash)
    * [Int](#int)
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
<td colspan="2" valign="top"><strong>getBlock</strong></td>
<td valign="top"><a href="#/graphql_api?id=block">Block</a>!</td>
<td>

Get the block

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">height</td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a></td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>getTransaction</strong></td>
<td valign="top"><a href="#/graphql_api?id=signedtransaction">SignedTransaction</a>!</td>
<td>

Get the transaction by hash

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">txHash</td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>getReceipt</strong></td>
<td valign="top"><a href="#/graphql_api?id=receipt">Receipt</a>!</td>
<td>

Get the receipt by transaction hash

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">txHash</td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>queryService</strong></td>
<td valign="top"><a href="#/graphql_api?id=execresp">ExecResp</a>!</td>
<td>

query service

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">height</td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a></td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">cyclesLimit</td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a></td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">cyclesPrice</td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a></td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">caller</td>
<td valign="top"><a href="#/graphql_api?id=address">Address</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">serviceName</td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">method</td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">payload</td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
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
<td colspan="2" valign="top"><strong>sendTransaction</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td>

send transaction

</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputRaw</td>
<td valign="top"><a href="#/graphql_api?id=inputrawtransaction">InputRawTransaction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputEncryption</td>
<td valign="top"><a href="#/graphql_api?id=inputtransactionencryption">InputTransactionEncryption</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>unsafeSendTransaction</strong> ⚠️</td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td>
<p>⚠️ <strong>DEPRECATED</strong></p>
<blockquote>

DON'T use it in production! This is just for development.

</blockquote>
</td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputRaw</td>
<td valign="top"><a href="#/graphql_api?id=inputrawtransaction">InputRawTransaction</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" align="right" valign="top">inputPrivkey</td>
<td valign="top"><a href="#/graphql_api?id=bytes">Bytes</a>!</td>
<td></td>
</tr>
</tbody>
</table>

## Objects

### Block

Block is a single digital record created within a blockchain. Each block contains a record of the previous Block, and when linked together these become the “chain”.A block is always composed of header and body.

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
<td valign="top"><a href="#/graphql_api?id=blockheader">BlockHeader</a>!</td>
<td>

The header section of a block

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>orderedTxHashes</strong></td>
<td valign="top">[<a href="#/graphql_api?id=hash">Hash</a>!]!</td>
<td>

The body section of a block

</td>
</tr>
</tbody>
</table>

### BlockHeader

A block header is like the metadata of a block.

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
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td>

Identifier of a chain in order to prevent replay attacks across channels 

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>height</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td>

block height

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>execHeight</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td>

The height to which the block has been executed

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>preHash</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td>

The hash of the serialized previous block

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>timestamp</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td>

A timestamp that records when the block was created

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>orderRoot</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td>

The merkle root of ordered transactions

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>confirmRoot</strong></td>
<td valign="top">[<a href="#/graphql_api?id=hash">Hash</a>!]!</td>
<td>

The merkle roots of all the confirms

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>stateRoot</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td>

The merkle root of state root

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>receiptRoot</strong></td>
<td valign="top">[<a href="#/graphql_api?id=hash">Hash</a>!]!</td>
<td>

The merkle roots of receipts

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>cyclesUsed</strong></td>
<td valign="top">[<a href="#/graphql_api?id=uint64">Uint64</a>!]!</td>
<td>

The sum of all transactions costs

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>proposer</strong></td>
<td valign="top"><a href="#/graphql_api?id=address">Address</a>!</td>
<td>

The address descirbed who packed the block

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>proof</strong></td>
<td valign="top"><a href="#/graphql_api?id=proof">Proof</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>validatorVersion</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td>

The version of validator is designed for cross chain

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>validators</strong></td>
<td valign="top">[<a href="#/graphql_api?id=validator">Validator</a>!]!</td>
<td></td>
</tr>
</tbody>
</table>

### Event

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
<td colspan="2" valign="top"><strong>service</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>data</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### ExecResp

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
<td colspan="2" valign="top"><strong>ret</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>isError</strong></td>
<td valign="top"><a href="#/graphql_api?id=boolean">Boolean</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### Proof

The verifier of the block header proved

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
<td colspan="2" valign="top"><strong>height</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>round</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>blockHash</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>signature</strong></td>
<td valign="top"><a href="#/graphql_api?id=bytes">Bytes</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>bitmap</strong></td>
<td valign="top"><a href="#/graphql_api?id=bytes">Bytes</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### Receipt

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
<td colspan="2" valign="top"><strong>stateRoot</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>height</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>txHash</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>cyclesUsed</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>events</strong></td>
<td valign="top">[<a href="#/graphql_api?id=event">Event</a>!]!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>response</strong></td>
<td valign="top"><a href="#/graphql_api?id=receiptresponse">ReceiptResponse</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### ReceiptResponse

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
<td colspan="2" valign="top"><strong>serviceName</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>method</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>ret</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>isError</strong></td>
<td valign="top"><a href="#/graphql_api?id=boolean">Boolean</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### SignedTransaction

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
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>cyclesLimit</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>cyclesPrice</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>nonce</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>timeout</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>serviceName</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>method</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>payload</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>txHash</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>pubkey</strong></td>
<td valign="top"><a href="#/graphql_api?id=bytes">Bytes</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>signature</strong></td>
<td valign="top"><a href="#/graphql_api?id=bytes">Bytes</a>!</td>
<td></td>
</tr>
</tbody>
</table>

### Validator

Validator address set

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
<td colspan="2" valign="top"><strong>address</strong></td>
<td valign="top"><a href="#/graphql_api?id=address">Address</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>proposeWeight</strong></td>
<td valign="top"><a href="#/graphql_api?id=int">Int</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>voteWeight</strong></td>
<td valign="top"><a href="#/graphql_api?id=int">Int</a>!</td>
<td></td>
</tr>
</tbody>
</table>

## Inputs

### InputRawTransaction

There was many types of transaction in Muta, A transaction often require computing resources or write data to chain,these resources are valuable so we need to pay some token for them.InputRawTransaction describes information above

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
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td>

Identifier of the chain.

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>cyclesLimit</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td>

Mostly like the gas limit in Ethereum, describes the fee that you are willing to pay the highest price for the transaction

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>cyclesPrice</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>nonce</strong></td>
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td>

Every transaction has its own id, unlike Ethereum's nonce,the nonce in Muta is an hash

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>timeout</strong></td>
<td valign="top"><a href="#/graphql_api?id=uint64">Uint64</a>!</td>
<td>

For security and performance reasons, Muta will only deal with trade request over a period of time,the `timeout` should be `timeout > current_block_height` and `timeout < current_block_height + timeout_gap`,the `timeout_gap` generally equal to 20.

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>serviceName</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>method</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>payload</strong></td>
<td valign="top"><a href="#/graphql_api?id=string">String</a>!</td>
<td></td>
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
<td valign="top"><a href="#/graphql_api?id=hash">Hash</a>!</td>
<td>

The digest of the transaction

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>pubkey</strong></td>
<td valign="top"><a href="#/graphql_api?id=bytes">Bytes</a>!</td>
<td>

The public key of transfer

</td>
</tr>
<tr>
<td colspan="2" valign="top"><strong>signature</strong></td>
<td valign="top"><a href="#/graphql_api?id=bytes">Bytes</a>!</td>
<td>

The signature of the transaction

</td>
</tr>
</tbody>
</table>

## Scalars

### Address

20 bytes of account address

### Boolean

### Bytes

Bytes corresponding hex string.

### Hash

The output digest of Keccak hash function

### Int

### String

### Uint64

Uint64

