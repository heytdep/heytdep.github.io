`04/09/2025`

We're used to having two options when building complex data infrastructures for blockchain based apps:

1. Run the whole data flow, including blockchain clients, locally and have our users trust it.
2. Rely on third-party services and shift trust upon these services.

While updating Mercury and retroshades to fit within Stellar's protocol 23 upgrade yesterday I had a neat idea to fix this. In this writeup I propose a design where we split data commits from execution and leverage:

1. trusted execution environments (TEEs) to perform trusted computations over chain state.
2. select blockchain nodes and collectors to run a threshold signature aggregation protocol (e.g bls agg). 

Note that rather than the commit+execute scheme that is proposed e.g in SBFT/HotStuff style protocols, we **trust execution** thus only need certificates around the data commits.

### The Win 

We unlock for complex dataflows (indexers, analytics, roll-ups of events, etc.) to become trustless in the view of application operators (and users as well) while allowing the whole setup to live on the cloud; and in the specific retroshades case the workflow is not only on the cloud but extremely scalable. 

Safety around integrity holds because every aggregation step is the result of a function that is executed on secure hardware and where the function's inputs are verifiably derived from committee-validated state.


## Actors and Interfaces

I describe a protocol $\Pi$ composed of the following participants:

* **TEE executors**. The workload lives here; they operate on optimistic unverified state, ask blockchain nodes for that state and are the destination of the threshold signatures protocol since it consumes and verifies the aggregated signature.
* **Blockchain nodes**. Blockchain nodes expose an endpoint that returns state entries that are signed with a TEE-attested public key for keys the executor requests.
* **Collector**. Collectors are a facilitator that degrades liveness but increase networking performance by receiving signatures from nodes and aggregating them until a threshold is met and then forwards the aggregated signature $\sigma_{agg}$ + signers set to the executors.
* **Observers**. Users/application operatorse/etc can query the executor; the executor’s TEE-attested signature lets them detect if data didn’t come from the trusted path.

## "TEE-attested" Signatures

The execution end of the data workflow is based off the assumption that an external observer can verify the validity of the data they're consuming from the executor given this so-called "tee-attested" signature. For those who are not familiar with trusted execution environments, dstack, and related technologies this might seem confusing.

You can think of TEE-attested signatures as signatures that are produced as a result of a special key signing the payload (in this case, the payload is the output of the data workflow). This special key is encumbered in secure hardware, meaning that it's not possible for anyone* that isn't the hardware itself to access this key; only the measured code that lives on the hardware can. 

The result is that if your hardware is running a specific code (e.g a retroshades executor) only the executor itself can use the attested signer to sign over the data, so you know the access to the key producing such signature is respecting the rules expressed in the executor's code.

\* side channel attack vectors against trusted hardware exist, but they're generally very expensive to carry out and require very specialized resources and time. There is also the risk of 0-days around micro-architecture.


# High-Level Workflow

The workflow is mainly split in two parts:

1. the event-driven part.
2. the state-driven one.

Both are used in complex data flows, event-driven usually triggers the workflow while state requests follow from there.

## Event-driven trigger

![](/images/event-driven.png)

1. On ledger close, nodes sign the ledger meta/any event that might need to be forwarded and broadcast it to the collector(s).
2. Collector: Aggregates node signatures until threshold.
3. Delivery:** Collector sends $\sigma_{agg}$ + signers + meta to the subscribed executor(s).
4. Executor: Verifies ledger meta/event by verifying $\sigma_{agg}$.
   → If valid the executor starts the data workflow, in retroshades it would start by requesting the state and performing the smart contract execution (see the next section).

We don't have to trust the collector, and we know that the executor will trigger the workflow only on data that is signed by the committee, thus deemed safe.


## Requesting chain state

![](/images/state-driven.png)


Requesting chain state starts on the executors and not on the nodes, mainly because in the design we assume that demand for state is granular enough to not make sense for nodes to broadcast signatures for all state.

1. (If needed) connect the executor with the collector.
2. Execute on an untrusted snapshot: The executor runs normally assuming a snapshot source as optimistically safe, e.g request state on a local node or one of the nodes in $\Pi$ without going through the collector. Note the executor treats this snapshot as untrusted. Every accessed portion of state is recorded into a state array and cached on the executor.
3. After the call’s reads/writes are known, the executor asks nodes to provide the exact set of entries it touched. The call is routed thorugh the executor to the set of nodes the executor trusts as committee.
4. Nodes fetch the requested keys from local state and return signature $\sigma_{i}$ along with the payload (i.e the state array) to the collector.
5. Similarly to the event-driven approach the collector aggregates signatures into $\sigma_{agg}$ for that state array until threshold is reached.
6. Collector sends $\sigma_{agg}$ + signers + states to the executor.
7. The executor verifies $\sigma_{agg}$:

   * If valid: the executor finalizes and clears the trusted state array in its cache.
   * If invalid the executor flags the state array as unsafe. 

8. Users query with a nonce (to ensure freshness). The executor signs the response (over data+nonce). As a result, external observes can tell if a response didn’t come from the trusted TEE path.

**Note**
We don’t track the executor’s internal state with a global integrity proof. Instead, we only allow the executor to finalize a mutation after the state it relied on has committee-validated signatures. Because execution is inside a TEE, validity of the computation follows from validity of inputs; any mutation with invalid inputs is easily detectable thanks to $\sigma_{agg}$.


## Why collectors

Collectors are not crucial to the functioning of $\Pi$, which could potentially be run only between all executors and all blockchain nodes, rather they act as helpers to concentrate messages and achieve linear communication given that nodes push the signatures and data to a collector which returns one aggregated package to the executor. 

This avoids $O(n^2)$ networking when broadcasting chain events. 

Note that since in the current design chain state is on a per request basis for each collector the nodes don't have to broadcast the state + $\sigma_{i}$ rather simply share them to the executor so networking doesn't really get more efficient for state requests unless we switch over to a non-request based approach (which might make sense in high state contention scenarios).

That said, having the executor in not only to reduce networking impact when broadcasting chain events. Collectors are the core of liveness within $\Pi$ and are completely trustless for safety which means they don't have to run in trusted enclaves and are thus much faster to spin up and mostly upgrade if needed compared to executors. They can also concentrate bandwidth to lower requirements on the other participants in $\Pi$.


## (Partial) synchrony requirements

Note that the system does not aim to serve stale states and doesn't want to introduce such requirement for nodes. As such, we assume the environment is partially synchronous where the time upper bound $\Delta$ for communication fits inside a slot execution window.

Under this assumption, the collector can regularly hit threshold fast enough, and the executor can keep pace with slot/ledger closes. 

If the network falls outside partial synchrony liveness degrades and while safety holds the data can become potentially corrupt as it is missing finalization. We can introduce a mechanism to pick up such degradation which is trivial since we can refer to the underlying chain's slot numbers and from then users can lookup those specific slots to make sure integrity keeps up. Note that a one-time verification is enough thanks to the integrity guarantees on the execution level.

It's also worth noting that when $\Pi$ falls out of sync with the network it is going to be because of networking issues on the collector assuming the threshold within the nodes' committee remains live. Collectors are lightweight and simple to replace and introduce within $\Pi$ so this concern is alleviated. 


## Light Clients for Networks Without Inclusion Proofs?

If you're familiar with how light clients work, you might have found similarities between them and part of $\Pi$: the executor doesn’t reconstruct global state; it simply pulls from other nodes and verifies a subset of data with committee attestations. 
This approach is similar to how light clients operate: an elected committee on the beacon chain first signs over their state root and then execution clients can verify their state's inclusion within the state root that reached consensus threshold among the committee.

However, there's a clear difference between this mechanism and $\Pi$: many networks can’t provide inclusion proofs simply because their storage layout isn't designed for that purpose.

In fact, within $\Pi$ we’re not reaching consensus on the entire state, rather we reach consensus only on the subset of state that the executor asked for. Consensus is therefore done as threshold signatures on those entries rather than on state roots. This is more tedious and less efficient on all fronts, in fact when applicable $\Pi$ should adapt its executors to work as light clients since they can skip a big portion of $\Pi$ while maintaining and likely increasing safety.

For chains that don't support light clients, the mechanisms described until now are potentially viable to emulate the behavior of light clients at a likely lower overall safety and efficiency. Running complex data flows on the executors however is a huge help in bridging the efficiency gap on that end.

## Why?

The cool thing here is that we don't have to track used state within the integrity proof we hand out on the executor, we know the executor will only mutate state when the state has been validated by the committee. 

This allows to build aggregations in a trustless manner since each mutation is attested by the hardware and whenever a mutation's inputs (i.e chain data) are invalid we can verify the invalidity thanks to \sigma_{agg}.

This means that even if we want to introduce further liveness and redundancy on the executors to safeguard the data we don't need any clients to be always live and we can trust any point in state in the executor without assembling execution certificates. So we can have sleepy clients (in relation to the "main" executor) and still fully maintain safety.

## Current Liveness Assumptions and Research Directions

Currently the design is not explicitly tuned for high liveness, in fact the initial design involves only one collector. I did mention however, that $\Pi$ is designed so collectors are lightweight in the actual execution and easy to adapt, upgrade, insert, remove, etc from the protocol. This already trivially allows to increase liveness guarantees almost out of the box.

There is also potential for further research on tweaking safety and liveness thresholds by changing validator and client assumptions, which is a field I've been involved with lately. It is also interesting to think about how such designs intersect with the guarantees we inherit from trusted hardware.

## Practical Optimizations

Some nodes may proactively forward expected entries so the executor can start verifying sooner. This can work in scenarios where some executors are privileged in the nodes' view and such nodes already know what event might trigger the executor asking for state arrays. This works especially well in chains that support footprints/(state) inclusion lists so that the only state the exeutor asks for is, if any, only the extra state required by the data workflow.

Also, it might be worth it to look into whether it's worth it to have nodes sign over specific entries vs request-specific state arrays. It might make sense for if there is high state contention across executors but I doubt it's needed at least in the initial prototype.

## Why Not Fit the Nodes in A TEE?

This is a viable approach, but I prefer designs where the TEE needs to do as less work as possible due to efficiency. I'm also afraid in many chains TEE nodes would be more expensive and fall out of sync easier. Nodes also require continuous maintenance and upgrades, which is not be best setup to run within a TEE.   

# Next Steps

I'm personally interested in carrying out an implementation of $\Pi$ that works on the [Stellar](https://stellar.org) network and runs [retroshades](https://mercurydata.app/) executors.

The implementation will initially be a proof of concept to demonstrate the idea and can potentially evolve into a more structured product.
