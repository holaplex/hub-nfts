# Hub NFTs

Hub-NFTs is a comprehensive service designed to facilitate the creation, minting, updating, transferring, and retrying of drops in the Non-Fungible Token (NFT) ecosystem across various blockchains. The service is blockchain-agnostic, meaning it is designed to support multiple blockchains. Initially implemented with Solana, it can be extended to other blockchains like Ethereum, Binance Smart Chain, or Polygon with relative ease, thanks to its modular and extensible design. The core of this service is built around the Edition trait, which abstracts the blockchain-specific actions for NFTs, and is designed to be implemented for each supported blockchain. This ensures that the main API handlers remain consistent and independent of the underlying blockchain, thereby providing a unified interface for NFT operations.

# Getting Started

Requirements:
- Docker
- Rust
- SeaORM cli

```
docker compose up -d # starts postgres and redpanda
sea migrate up --database-url postgres://postgres:holaplex@localhost:5439/hub_nfts # setup the database by running migrations

cargo run --bin holaplex-hub-nfts # start the NFTs graphql API
```

A GraphQL playground is available at [http://localhost:3004/playground](http://localhost:3004/playground).
