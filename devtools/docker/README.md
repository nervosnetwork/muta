# use docker for development

## build `muta:build` and `muta:run` docker image

```
docker build -t huwenchao/muta:build -f devtools/docker/dockerfiles/Dockerfile.muta_build .

docker build -t huwenchao/muta:run -f devtools/docker/dockerfiles/Dockerfile.muta_run .
```

Use docker to build muta binary:
```
docker run -it --init --rm -v `pwd`:/code -v `pwd`/target/docker_target:/code/target -v `pwd`/target/cargo_cache:/usr/local/cargo/registry huwenchao/muta:build bash -c 'cd /code && cargo build'
```

We mount `target` and `cargo/registry` to dirs under `target` to make it faster when recompile the binary.

Use the default config to run a single node muta chain:
```
docker run -it -v `pwd`:/app -v `pwd`/target/docker_target/debug/muta-chain:/app/muta-chain huwenchao/muta:run /bin/bash -c 'cd /app; ./muta-chain'
```

## use `docker-compose` to run multiple nodes rapidly

### single node

```
# get single node up
$ docker-compose -f devtools/docker/dockercompose/docker-compose-single.yaml up

# use graphql to interact with the node: <http://localhost:8000/graphiql>

# go inside the node
$ docker-compose -f devtools/docker/dockercompose/docker-compose-single.yaml exec node0 bash
```

The chain data is in `target/data/single`.

### multiple nodes

#### master and follower

Start 2 nodes, `node1` is in the validator list, and `node2` is follower, sync blocks from node1.
Check the config if you want.

```
$ docker-compose -f devtools/docker/dockercompose/docker-compose-mul.yaml up

# go inside node1
$ docker-compose -f devtools/docker/dockercompose/docker-compose-mul.yaml exec node1 bash
```

The chain data is in `target/data/mul1` and `target/data/mul2`.


#### four nodes bft

Start 4 nodes bft.

```
$ docker-compose -f devtools/docker/dockercompose/docker-compose-bft.yaml up
```

The nodes names are `bft_node1` ~ `bft_node4` and chain data is in `target/data/bft1` ~ `target/data/bft4`.


## private keys used in multi node config

```
[mul-1] and [bft-1]
user_address=10ecc2746d8ad8ca82872bf0af59ebefbe003b2d0c privkey=136dbd5402e4002d9efb3f67eb8250b719e54813a4369cf8ad26ef916d938850 pubkey=03d158fb85df6201708f2cbde51e0b7c2d8835a28aaea7ed243e5db1aa442b18b1

[mul-2] and [bft-2]
user_address=10c198d22b505a51f11f0358353f341abff4dfee61 privkey=61b315af6f2c21a4716689e6b5fb65e0acdfda86f2e167d8ea1000b75372b90c pubkey=036bde6d0d178b0b7f225191e6a2584439c9d908714168d0a077736ea8d5452b38

[bft-3]
user_address=10bbd58fe01f00aeb469175585bd2858b5b21c5092 privkey=037ddeb069fdfeede6993eb587b394bc1984a3ba21b8ea64d5b8679b0d673d77 pubkey=024ce78450e49c99fc95eeb6ddc1963e4fec1bd815d3a23d0d48653dd9d6ca112f

[bft-3]
user_address=104c52183d10fe7f65f3b22b45528d97212507bd6c privkey=fa57417b549b0e303dbe6a0653b25810f818442fdf0c0e264d7cb546e9c9f310 pubkey=0385cab5d471616c91ecc5562275ff339899a306c55e2fc9ff860a1d5899152dfc
```