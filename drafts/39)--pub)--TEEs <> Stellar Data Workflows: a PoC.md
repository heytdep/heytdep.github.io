`04/09/2025`

As promised in the last [writeup](https://heytdep.github.io/post/38/post.html), here are some updates on leveraging TEEs for complex blockchain data workflows, specifically on the Stellar network.

In [awesome-tee-stellar](https://github.com/xycloo/awesome-tee-stellar) I sketched an app that stitches together the three moving parts discussed in the other post: a Stellar client (`client`, `clients/stellar`) that runs stellar core under the hood through rs-ingest (same lib we've been using as core wrapper for Mercury) that observes ledger activity, a lightweight collector (`collector`) that aggregates and validates chain data, and an executor (`executors/retroshade`) that runs the actual data workflow inside a trusted context. 

![](/images/pocteesstellar.png)

# What this PoC tries to achieve

Blockchains excel at commitment, not necessarily computation. The kinds of jobs many apps require like indexing and aggregating contract events, tracking and aggregating state across ledgers, simulating outcomes, etc don’t fit neatly on a decentralized setting as they are downstream processes. 

When an application needs to compute and retrieve such data, we centralize them (and lose trust properties by introducing at least one trusted actor). The design in the original writeup proposed to move the workflow into a TEE, pin the trust of its outputs to hardware attestation, and feed the workflow chain data that is committed by a set of trusted peers on the executor's view. 

This PoC is that idea implemented in a fairly modular way that makes it simple to read and potentially pull apart and experiment with (something I'll be doing as well).

# The shape of the system

Concretely, the **Stellar client** watches the network and emits compact messages whenever something relevant happens (i.e the ledger closes and emits a new ledger close). The collector receives those notices from multiple parties, does simple checks, and only when a threshold of signals is satisfied does it build a final payload for that data and forward it to the executor. 

The executor fetches the exact state it needs, runs the transformation, and produces an attested result that can be consumed downstream. Note that I haven't yet changed the snapshot source we use for retroshades to track requested entries and initiate state requests yet. Once that is implemented we can effectively have the secure hardware compute on actual valid state independently of the events.

## Why not just run a normal indexer?

Because the interesting property isn’t that the job simply runs as if it were running on my/yours/anyone's server, it’s that its environment and code identity are measured. So, you’re not trusting the server operator for safety rather you’re trusting a hardware-anchored measurement. That lets actors like wallets, dapps, bots, users, etc to treat the output like an actual capability.


# No light clients

This PoC is specifically centered on chains that don't have the ability to build a light client similarly to how eth l1 does (basically, if you can build a stateless proof of inclusion of some state). The whole clients/collector end of things target exactly that and similarly to how eth l1 light clients work they require a committee to sign off the chain's root (I go over more details around the light clients theoretical points in the other post). It's curcial that the executor trusts the committee, but it's worth noting that here tee execution alleviates the inefficiencies on trustless state access for chains that don't have the above charateristic.

### Implementing for other chains

At xycloolabs, we only directly work with the Stellar Network but the implementation is modular enough that allows to easily plug in clients from different chains, you just need to implement the `BlockchainClient` trait.

For instance, stellar's client implementation is as simple as

```rust
use std::collections::HashMap;

use client::{BlockchainClient, CommitteeNode};
use common::message::{
    ChainEventKind, ChainStateRequestKind, ChainStateResponseKind, StellarLedgerEntry,
    StellarStateResponse,
};
use stellar_xdr::next::{LedgerEntry, Limits, ReadXdr, WriteXdr};
use tokio::{runtime::Builder, sync::mpsc, task::LocalSet};
use zephyr::snapshot::raw_endpoint::configurable_entry_and_ttl;

mod event;

pub struct StellarClient {
    pub core_endpoint: String,
    pub network: String,
}

impl StellarClient {
    pub fn new(core_endpoint: String, network: String) -> Self {
        Self { core_endpoint, network }
    }

    pub fn client_from_self(self, signing_key: &[u8; 32]) -> anyhow::Result<CommitteeNode<Self>> {
        let node = CommitteeNode::new(signing_key, self)?;
        Ok(node)
    }
}

impl BlockchainClient for StellarClient {
    fn spawn_chain_event_worker(&self) -> anyhow::Result<mpsc::Receiver<ChainEventKind>> {
        let (sender, receiver) = mpsc::channel(20);
        let network = self.network.clone();

        std::thread::spawn(move || {
            tracing::info!("running on own thread");
            let rt = Builder::new_multi_thread()
                .enable_all()
                .worker_threads(4)
                .build()
                .expect("build current_thread runtime");

            let local = LocalSet::new();

            tracing::info!("running on localset");
            rt.block_on(local.run_until(async move {
                if let Err(e) = event::run_stellar_core(sender, network).await {
                    tracing::error!(?e, "run_stellar_core exited");
                }
            }));
        });

        Ok(receiver)
    }

    fn retrieve_chain_state(
        &self,
        requested_state: common::message::ChainStateRequestKind,
    ) -> anyhow::Result<common::message::ChainStateResponseKind> {
        match requested_state {
            ChainStateRequestKind::ChainState(state_request) => {
                let mut response = HashMap::new();

                for key in state_request.inner() {
                    let entry = configurable_entry_and_ttl(
                        key.to_xdr(Limits::none()).unwrap(),
                        self.core_endpoint.clone(),
                    )?;
                    if let Some((entry, ttl)) = entry {
                        response.insert(
                            key,
                            StellarLedgerEntry::new(
                                LedgerEntry::from_xdr(entry, Limits::none())?,
                                ttl,
                            ),
                        );
                    }
                }

                Ok(ChainStateResponseKind::ChainState(
                    StellarStateResponse::new(response),
                ))
            }
        }
    }
}
```

You can then import the `client` lib on top of your chain's:

```rust
fn main() {
    tracing_subscriber::fmt::init();
    let rt = Builder::new_multi_thread().enable_all().worker_threads(4).build().unwrap();
    
    let client = StellarClient::new("someendpoint".into(), "Test SDF Network ; September 2015".into());
    let key: [u8; 32] =
        hex::decode("input")
            .unwrap()
            .try_into()
            .unwrap();

    let mut node = client.client_from_self(&key).unwrap();
    let result = rt.block_on(node.start(vec![]));
    // ...
}

// ...

impl StellarClient {
    pub fn client_from_self(self, signing_key: &[u8; 32]) -> anyhow::Result<CommitteeNode<Self>> {
        let node = CommitteeNode::new(signing_key, self)?;
        Ok(node)
    }
}
```

# Conclusion

While this is not yet near being actually deployed on production, it's a cool example that can easily be turned into a standard or a more formal protocol.

This is our attempt to spark some research and interested around TEEs affinity with the Stellar network, which is especially interesting given the chain's predisposition to on-off ramps 👀.

See you in the next write up! 

In case you missed it from the article, here's the code:

- https://github.com/xycloo/awesome-tee-stellar