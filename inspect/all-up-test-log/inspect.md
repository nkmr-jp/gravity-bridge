```plantuml
title 全体の流れ 210916173948 
box Peggy(Gravity)
    participant Validator1 as "Validator1\n\n address: cosmos1hsyx...\n rpc: tcp://0.0.0.0:26657\n grpc: 0.0.0.0:9090\n listen: tcp://7.7.7.1:26655\n p2p: tcp://7.7.7.1:26656"
    participant Validator2 as "Validator2\n\n address: cosmos14wns...\n rpc: tcp://7.7.7.2:26658\n grpc: 7.7.7.2:9091\n listen: tcp://7.7.7.2:26655\n p2p: tcp://7.7.7.2:26656"
    participant Validator3 as "Validator3\n\n address: cosmos16kus...\n rpc: tcp://7.7.7.3:26658\n grpc: 7.7.7.3:9091\n listen: tcp://7.7.7.3:26655\n p2p: tcp://7.7.7.3:26656"
end box

participant ETH as "ETH"

```