## Layout

```sh
.
├── common
│   ├── channel
│   ├── config-parser
│   ├── crypto
│   ├── logger
│   ├── merkle
│   └── metrics
│   └── pubsub
├── core
│   ├── api
│   ├── consensus
│   ├── database
│   ├── executor
│   ├── network
│   ├── storage
│   └── mempool
├── devtools
│   └── ci
├── docs
│   └── menu.md
├── protocol
│   ├── codec
│   ├── traits
│   └── types
├── src
   └── main.rs
```

A brief description:

- `common` Contains utilities for muta-chain.
- `core` Contains implementations of module traits.
- `devtools` Contains scripts and configurations for better use of the this repository.
- `docs` for project documentations.
- `protocol` Contains types, serialization, core traits for muta-chain.
- `src` Contains main packages
