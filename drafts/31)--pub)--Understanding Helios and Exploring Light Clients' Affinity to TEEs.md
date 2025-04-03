`03/04/2025`

Decentralized TEE-based applications that interface with blockchains need to plug in a trusted source of data for said blockchains. One approach is to run full nodes within a TEE, but an even better one is to use light clients when applicable. At a high level, light clients guarantee to the TEE that the data used by the execution layer existed in the blockchain at the specified block.

Helios proposes an interesting approach where the light client not only provides a consensus client but also integrates with external execution layers (execution RPCs) that support handing out inclusion MPT proofs (e.g Alchemy or Infura). In this write-up I briefly explore how Helios works at a lower level to understand the actual guarantees that it provides and later make some observations regarding the affinity of such light clients with TEEs and what are some precautions we need to make when utilizing light clients within a TEE. 

# MPT Proofs in Light Clients

With helios we can plug in any consensus rpc we want and bootstrap our node given our weak subjective root of trust (more on this in the next section).

Looking at the code, helios basically allows to execute on verifiable state by either verifying inclusion proof on the returned data or by loading said state first for more complex actions. E.g if we run `call` what happens is that helios first builds state:

```rust
let mut db = ProofDB::new(self.tag, self.execution.clone());
_ = db.state.prefetch_state(tx, validate_tx).await;

// ...

pub async fn prefetch_state(
        &mut self,
        tx: &N::TransactionRequest,
        validate_tx: bool,
    ) -> Result<()> {
        let block_id = Some(self.block.into());
        let account_map = self
            .execution
            .create_extended_access_list(tx, validate_tx, block_id)
// ...
```

where creating the access list is streamlined to a generic, which behind a couple of calls is implemented by the execution client API. When setting up helios, **if a consensus rpc is provided the builder defaults to verifiable execution** which enables the `verifiable_api` where extending the access list works as follows:

```rust
async fn create_extended_access_list(
    &self,
    tx: &N::TransactionRequest,
    validate_tx: bool,
    block_id: Option<BlockId>,
) -> Result<HashMap<Address, Account>> {
    let block_id = block_id.unwrap_or_default();
    let tag = BlockTag::try_from(block_id)?;
    let block = self
        .state
        .get_block(tag)
        .await
        .ok_or(ExecutionError::BlockNotFound(tag))?;
    let block_id = BlockId::Number(block.header().number().into());

    let ExtendedAccessListResponse { accounts } = self
        .api
        .create_extended_access_list(tx.clone(), validate_tx, Some(block_id))
        .await?;

    for (address, account) in &accounts {
        self.verify_account(*address, account, &block)?;
    }

    Ok(accounts)
}
  
fn verify_account(
    &self,
    address: Address,
    account: &AccountResponse,
    block: &N::BlockResponse,
) -> Result<()> {
    let proof = EIP1186AccountProofResponse {
        address,
        balance: account.account.balance,
        code_hash: account.account.code_hash,
        nonce: account.account.nonce,
        storage_hash: account.account.storage_root,
        account_proof: account.account_proof.clone(),
        storage_proof: account.storage_proof.clone(),
    };
    // Verify the account proof
    verify_account_proof(&proof, block.header().state_root())?;
    // Verify the storage proofs
    verify_storage_proof(&proof)?;
    // Verify the code hash (if code is included in the response)
    if let Some(code) = &account.code {
        verify_code_hash_proof(&proof, code)?;
    }

    Ok(())
}
```

The important part here is `verify_some_proof` which ultimately resolves to `verify_mpt_proof` which `/// Verifies a MPT proof for a given key-value pair against the provided root hash`. This means that the execution layer returns a merkle patricia trie inclusion proof where which gets matched against the current block header (assuming the client is synced!)

This all sounds good, but how do we obtain our root?

# Consensus Client

Again, Helios is cool because it wraps both execution and consensus. Execution uses state obtained through RPC inclusion proofs and consensus does the most important part, syncing the root state. To understand what are the guarantees around helios light clients, the question is about how this actually happens.

When bootstrapping our light client, we request a “bootstrap” from the RPC consensus we specified when building:

```rust
let bootstrap = self
	.rpc
	.get_bootstrap(checkpoint)
	.await
```

The `checkpoint` is the famous [**weak subjectivity checkpoint**](https://ethereum.org/en/developers/docs/consensus-mechanisms/pos/weak-subjectivity/#ws-checkpoints). This is the root of trust we are going to use to sync our node. It’s a block hash that was included in the chain. As you might guess, the validity of such root of trust derives the security of the light client; a checkpoint that was not actually included can lead a light client that relies on a malicious consensus RPC to believe a certain state is correct when it indeed isn’t.

We request the bootstrap for a given block root (the checkpoint) and what we get back is effectively all the information we need to bootstrap:

```rust
pub struct Bootstrap<S: ConsensusSpec> {
    pub header: LightClientHeader,
    pub current_sync_committee: SyncCommittee<S>,
    pub current_sync_committee_branch: FixedVector<B256, typenum::U5>,
    pub current_sync_committee_branch: FixedVector<B256, typenum::U6>,
}
```

We cannot take bootstrap for granted tho as it’s the result of an RPC call at the end of the day and we cannot blindly trust the response. As such we verify the committee that was provided against the bootstrap header (NB: we’re not directly verifying against our checkpoint here, just verifying that the committee is indeed a leaf of the beacon header), and then we make sure that the header on which we verified is indeed our checkpoint:

```rust
let committee_valid = is_current_committee_proof_valid(...);
let header_hash = bootstrap.header().beacon().tree_hash_root();
let header_valid = header_hash == checkpoint;
```

Once the bootstrap is verified it is appended to the light client’s in memory store and the syncing process can begin: based on the sync diff we request updates to the consensus client and finally verify and apply all of the updates:

```rust
let updates: Vec<Update<S>> = self.get_updates().await?;
for update in updates {
    self.verify_update(&update)?;
    self.apply_update(&update);
}
```

## Consensus <> Execution Communication

To fully understand the implementation and evaluate considerations for using helios in a TEE, we need to check how the updates are applied and how it impacts the execution layer.

If you look at the code, helios maintains the two clients separately. As the consensus syncs, it produces blocks that are appended to consensus state. However, when execution tries to get a hold of the root hash to verify MPT proofs, it doesn’t request them directly to the consensus client, it rather looks at the local execution client state and depending on the provided block tag it gets the block. 

This implies that there is some level of communication between the consensus and execution clients. In fact, when helios instantiates the execution client it provides it with a receiver for an mpsc channel:

```rust
impl<N: NetworkSpec, C: Consensus<N::BlockResponse>> Node<N, C> {
  pub fn new(
      execution_mode: ExecutionMode,
      mut consensus: C,
      fork_schedule: ForkSchedule,
  ) -> Result<Self, ClientError> {
      let block_recv = consensus.block_recv().unwrap();
      let finalized_block_recv = consensus.finalized_block_recv().unwrap();

			// ...
      start_state_updater(state, execution.clone(), block_recv, finalized_block_recv);

      Ok(Node {
          consensus,
          execution,
          fork_schedule,
      })
  }
}
  
pub fn start_state_updater<N: NetworkSpec>(
  state: State<N>,
  client: Arc<dyn ExecutionSpec<N>>,
  mut block_recv: Receiver<N::BlockResponse>,
  mut finalized_block_recv: watch::Receiver<Option<N::BlockResponse>>,
) {
  run(async move {
      loop {
          select! {
              block = block_recv.recv() => {
                  if let Some(block) = block {
                      state.inner.write().await.push_block(block, client.clone()).await;
                  }
              },
              _ = finalized_block_recv.changed() => {
                  let block = finalized_block_recv.borrow_and_update().clone();
                  if let Some(block) = block {
                      state.inner.write().await.push_finalized_block(block);
                  }
              },
          }
      }
  });
}
```

As we might expect, the consensus client on the other hand sends out the verified blocks while synchronizing:

```rust
impl<S: ConsensusSpec, R: ConsensusRpc<S>, DB: Database> ConsensusClient<S, R, DB> {
    pub fn new(rpc: &str, config: Arc<Config>) -> Result<ConsensusClient<S, R, DB>> {
        let (block_send, block_recv) = channel(256);
        let (finalized_block_send, finalized_block_recv) = watch::channel(None);
				run(async move {
            let mut inner = Inner::<S, R>::new(
                &rpc,
                block_send,
                finalized_block_send,
								// ..
            );

						// ... sync client to the current checkpoint
            _ = inner.send_blocks().await;
            loop {
              // advance and send out blocks to the execution layer receiver!
              let res = inner.advance().await;
              let res = inner.send_blocks().await;
              // ..
            }
        });

        Ok(ConsensusClient {
            block_recv: Some(block_recv),
            finalized_block_recv: Some(finalized_block_recv),
						// ..
        })
    }
}

```

Cool, so now we know exactly what gets verified and what gets send to the execution layer. Before making TEE specific considerations however, we need to check out how light clients deal with re-orgs.

# Dealing with Reorgs on Ethereum L1

Reorgs occur when there are two blocks being proposed for the same head. In PoS specifically, this should not happen much because validators are not racing each other to the block rather there is an elected validator creating the block; a validator could still produce two blocks for the same head but it would get their stake slashed. Reorgs however seem to occur mainly due to delays in the networking level that cause a block to not be received in time by most of the network and not have enough attestations to back up the block. 

For example, say that we have block 10 that was selected as head of the chain, then it’s validator v_1’s turn to create the block 11 but the next elected validator v_2 has received block 11 late (e.g after 18s vs within 12s) so it built block 12 off block 10. Assuming that block 12 gets received by the majority view of the network in time, it will then have more attestation and will be selected as new head (from 10 → 12). 

In fact,  due to network latency/delayed block propagation, a (non necessarily small) subset of the validators might not see the latest block in time and are led to attest for an older block. Later, when the delayed block is received, so we effectively have two competing blocks, the fork choice rule will switch to the branch with higher attestation weight.

### Light clients and reorgs

Light clients rely on the beacon API to process updates to their local view; more specifically after synchronizing with committee data they sync on finalized state. The latter happens when advancing the local view as well:

```rust
pub async fn sync(&mut self, checkpoint: B256) -> Result<()> {
    // sync committee data ..
    let finality_update = self.rpc.get_finality_update().await?;
    self.verify_finality_update(&finality_update)?;
    self.apply_finality_update(&finality_update);
    // ..
}

pub async fn advance(&mut self) -> Result<()> {
	let finality_update = self.rpc.get_finality_update().await?;
	self.verify_finality_update(&finality_update)?;
	self.apply_finality_update(&finality_update);
	// ..
}
```

Behind some calls, applying the finality update on the light client’s local view looks like this

```rust
pub fn apply_generic_update<S: ConsensusSpec>(
    store: &mut LightClientStore<S>,
    update: &GenericUpdate<S>,
) -> Option<B256> {
		// ..
    if should_update_optimistic {
        store.optimistic_header = update.attested_header.clone();
    }

    // ..
    if should_apply_update {
        let checkpoint = apply_update_no_quorum_check(store, update);
        // ..
    }
}

fn apply_update_no_quorum_check<S: ConsensusSpec>(
    store: &mut LightClientStore<S>,
    update: &GenericUpdate<S>,
) -> Option<B256> {
		// ..
    if update_finalized_slot > store.finalized_header.beacon().slot {
        store.finalized_header = update.finalized_header.clone().unwrap();

        if store.finalized_header.beacon().slot > store.optimistic_header.beacon().slot {
            store.optimistic_header = store.finalized_header.clone();
        }

        if store.finalized_header.beacon().slot % S::slots_per_epoch() == 0 {
            let checkpoint = store.finalized_header.beacon().tree_hash_root();
            return Some(checkpoint);
        }
    }

    None
}
```

As the code hints, the light client’s call to `/eth/v1/beacon/light_client/finality_update` on the consensus RPC returns both data to update the optimistic and finalized headers. Optimistic headers can be reorg’d while finalized can give more assurances. 

That said, for a view to be finalized we must wait for the current epoch to complete and the subsequent epoch to complete as well; this means that the light client is operating on a version of the chain that is between 6-12 minutes behind the “real-time” view of the chain. 

---

We’re now ready to make some considerations that are TEE specific.

> NB: there might be more!
> 

# TEE considerations

## **On the weak subjective checkpoint**

The bootstrapping checkpoint should be signed and published by the TEE. This allows anyone to verify that the light client was bootstrapped at a certain point in time using a checkpoint from another point in time. Depending on whether the checkpoint is valid and not vulnerable to not necessarily theoretical mathematical attacks other nodes can decide to join the P2P network of light clients. Kettles that are replicating and not bootstrapping should request the checkpoint from other kettles, there should be at least one kettle (the bootstrapping one) ready to hand out the checkpoint.

## **On RPCs**

Since the light client can verify RPC data for both consensus and execution, the RPCs used should obviously be configurable by the operator; potentially also at runtime. We already rely on the operator for availability of said light client, so we also trust the operator to use RPCs that are available; runtime switching can come really in handy if the RPC providers turn out to be unavailable or not reliant. This implies that the host has some level of authentication within the TD that enables them to swap rpc providers mid-execution with a handle to switch providers. This doesn’t decrease security. Additionally, helios should have some built-in redundancy to provide higher guarantees without having to re-sync. The very responsive helios team is already working on this! [https://github.com/a16z/helios/issues/574](https://github.com/a16z/helios/issues/574)

## **On timestamps**

Helios requires a handle over the current system timestamp in a handful of situations. The first one to get out of the way is the bootstrapping. Helios holds logic to force the checkpoint to be resistant to worst-case scenario theoretical attacks. This logic would be enforced within the measurement. However, this logic relies on the timestamp. Timestamps are generally untrusted territory within TEEs like TDX. Assuming we’re using `kvmclock` a malicious host could bootstrap with an old checkpoint opening up doors for practical attacks to infer malicious chain state in the light client and producing potentially malicious downstream data. Since on 1 we said we want to rely on social layer to choose which network to join, this is not an issue. A visibly old checkpoint will be considered as an attempt to bootstrap to a potentially invalid state and such node will not be joined; but some designs may not rely on this logic and could require e.g the bootstrapping to be done on a hardcoded rpc which was known to work correctly at bootstrap time taking that as a root of trust as well. Other attack vectors, e.g in `get_updates` are mainly harming availability by e.g keeping the node at an older state by fiddling with the `expected_current_slot` . This could be potentially harming in an application as well, but can be solved by introducing redundancy in the light clients and allowing any light client to check the messages produced by each other, and if a malicious host tries to submit old state as valid the other nodes can prove that the state was old. These can be pretty easily solved by disabling `kvmclock` and using the virtual TSC provided by TDX; safeguarded by an NTP setup on bootstrap. More on this here.


## **On reorgs**

As explained perviously, handling re-orgs is not something that in inherent behavior of the light client itself. The light client relies on a beacon node to infer the canonical fork, but as shown in the above section, the beacon node shares both a finalized view and an optimistic one. The light client might choose to rely on the optimistic view, but in case of a reorg of said optimistic view, the light client will update its view to follow the one inferred by the new finalized view derived from the beacon node. This means that the light client’s internal view was wrong at a certain point in time. Reorgs are handled automatically by deriving view from the beacon node, but if basing the application’s view on one that can be rolled back is not an option for the application then having the verification happen on a finalized view can be safer. Note that transactions in reorg’d block are rolled back and included in a later block it doesn’t mean they are malicious so it depends on what the application rules require. The application might also choose to combine both optimistic and finalize views for operations of different validity.

### Reducing Reorg Impact Probability

One approach to reducing the probability of having internal application state impacted by the reorg is to require at least *n* optimistic block before we deem block `CURRENT - n` valid. If `n > 1` then we already greatly reduce the probability of a reorg impacting our internal application logic at the expense of lagging *n* slots behind. This approach works for tasks that need an optimistic view of the network with higher probability of being valid but where such view doesn't perform critical actions.

### Caching optimistic views

This approach if for application state that is more critical but does not necessarily have a chained effect, i.e can be updated given an optimistic view but in case a reorg happens it's not a vulnerability to re-update such state to account for the reorg. 

If the reorg happens then we need to check whether it impacts the block we used to modify internal app state to act appropriately on that state. This implies that the internal app state can detect reorgs. In order to pick up reorgs one approach is to cache the optimistic views and if one mismatches with a view we picked up the settlement from then it’s an indicator that the reorg happened.

## Dstack Light clients

The dstack replication approach has started to gain popularity within TEEs because it allows nodes to replicate with the same encrypted state without ever decrypting it in an insecure way. Light clients don't necessarily need to share encrypted state, however having to onboard and verify the quote for each light client before being able to verify the signature is some extra lifting and adds some complexity on the consumer (which could be another TEE as well). 

To tackle this we can use the dstack replication approach so that there's a single public key we need to verify the signature of: the dstack shared key. We now know that any signature for the dstack pubkey is an attested light client, so there's no need to verify multiple quotes, we can just verify the bootstrap quote one time!

The light clients will then all produce data under the same signature. 

### Tplus dstack light clients on TDX

I've [experimented with this](https://github.com/tpluslabs/tplus-examples/tree/main/dstack-helios-tdx) and you can go over to [https://tpluslabs.github.io/tplus-examples/](https://tpluslabs.github.io/tplus-examples/) and follow the instructions.

---

There's probably more to be added here this is just the first iteration! 
