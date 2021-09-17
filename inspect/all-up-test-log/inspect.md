

```plantuml

title 全体の流れ 
' actor
participant Operator

box Orchestrator
    participant EthSigner
    participant Oracle
    participant Relayer
end box

box Valset(Validator Set)
    participant Validator1 as "Validator1\n\n address: cosmos1hsyx...\n rpc: tcp://0.0.0.0:26657\n grpc: 0.0.0.0:9090\n listen: tcp://7.7.7.1:26655\n p2p: tcp://7.7.7.1:26656"
    participant Validator2 as "Validator2\n\n address: cosmos14wns...\n rpc: tcp://7.7.7.2:26658\n grpc: 7.7.7.2:9091\n listen: tcp://7.7.7.2:26655\n p2p: tcp://7.7.7.2:26656"
    participant Validator3 as "Validator3\n\n address: cosmos16kus...\n rpc: tcp://7.7.7.3:26658\n grpc: 7.7.7.3:9091\n listen: tcp://7.7.7.3:26655\n p2p: tcp://7.7.7.3:26656"
end box

participant ETH as "ETH"

```


## Operator
[Definitions gravity-bridge/overview.md](https://github.com/cosmos/gravity-bridge/blob/5fabdc4/docs/design/overview.md#definitions)
> Operator - Cosmos SDK validatorノードをコントロールする人（複数可）です。
> これは、Cosmos SDKのステーキングセクションでは、valoperまたは'Validator Operator'とも呼ばれています。

## Orchestrator
[Definitions gravity-bridge/overview.md](https://github.com/cosmos/gravity-bridge/blob/5fabdc4/docs/design/overview.md#definitions)
> Orchestrator - Operatorが使いやすいようにEth Signer、Oracle、Relayerを組み合わせた単一のバイナリ。

### Eth Signer
> Eth Signer (name WIP) - Operatorが管理する別のバイナリで、2つのチェーン間でトークンを移動させるためのトランザクションに署名するためのEthereum秘密鍵を保有しています。

### Oracle
> Oracle (name WIP) - Operatoが管理する独立したバイナリで、EthereumチェーンからCosmosチェーンにデータを移すために使用されるCosmos SDKの秘密鍵を保持しています。

### Relayer
> Relayer - これは、EthereumのGravityコントラクトにアップデートを提出するノードの一種です。一括して取引を行うことで手数料を得ることができます。


## Validator Set( Valset )
[Definitions gravity-bridge/overview.md](https://github.com/cosmos/gravity-bridge/blob/5fabdc4/docs/design/overview.md#definitions)
> Validator Set - Cosmos SDKチェーン上のバリデータのセットと、それぞれの投票権です。tendermintブロックの署名に使われるed25519の公開鍵です。

[test_valset_update()](https://github.com/nkmr-jp/gravity-bridge/blob/mylog/orchestrator/test_runner/src/happy_path.rs#L242)
valsetはコード上では頻出するワードだが、ドキュメントには説明が無い。コード上のコメントから察するに Validator Setのことと思われる。

### Validator
> Validator - これはCosmos SDK Validating Node（署名ブロック）です。

### REST Server
> RESTサーバー - これはポート1317で動作するCosmos SDKの「RESTサーバー」で、バリデータノードまたはオペレーターが管理する別のCosmos SDKノードで動作します。


## Full Node
> Full Node - これはオペレーターが運営するEthereum Full Nodeです。
