# use docker for development

## use `docker` to run single node

```sh
docker run -it --init -p 8000:8000 nervos/muta

# you can mount the data dir to reserve the chain data after you stop the chain
docker run -it --init -p 8000:8000 -v `pwd`/data:/app/devtools/chain/data nervos/muta
```

If you want to run the chain when developing, you can build the binary and mount it in docker instead of rebuild the image.

```sh
cd /path/to/muta

# build muta
# We mount `target` and `cargo/registry` to dirs under `target` to make it faster when recompile the binary.
docker run -it --init --rm -v `pwd`:/code -v `pwd`/target/docker_target:/code/target -v `pwd`/target/cargo_cache:/usr/local/cargo/registry nervos/muta:build bash -c 'cd /code && cargo build'

# use the new compiled binary to overide that inside docker image
docker run -it --init --rm -p 8000:8000 -v `pwd`/target/docker_target/debug/muta-chain:/app/muta-chain nervos/muta
```

## use `docker-compose` to run multiple nodes rapidly

### single node

```sh
# get single node up
docker-compose -f devtools/docker/dockercompose/docker-compose-single.yaml up

# use graphql to interact with the node: <http://localhost:8000/graphiql>

# go inside the node
docker-compose -f devtools/docker/dockercompose/docker-compose-single.yaml exec node0 bash
```

The chain data is in `target/data/single`.

### multiple nodes

#### master and follower

Start 2 nodes, `node1` is in the validator list, and `node2` is follower, sync blocks from node1.
Check the config if you want.

```sh
docker-compose -f devtools/docker/dockercompose/docker-compose-mul.yaml up

# go inside node1
docker-compose -f devtools/docker/dockercompose/docker-compose-mul.yaml exec node1 bash
```

The chain data is in `target/data/mul1` and `target/data/mul2`.


#### four nodes bft

Start 4 nodes bft.

```sh
docker-compose -f devtools/docker/dockercompose/docker-compose-bft.yaml up
```

The nodes names are `bft_node1` ~ `bft_node4` and chain data is in `target/data/bft1` ~ `target/data/bft4`.


## rebuild docker image

```sh
# rebuild it when we change cargo version or add new build dependencies
docker build -t nervos/muta:build -f devtools/docker/dockerfiles/Dockerfile.muta_build .

# rebuild it when we add new dependencies to the run environment
docker build -t nervos/muta:run -f devtools/docker/dockerfiles/Dockerfile.muta_run .

docker build -t nervos/muta:latest .
```


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