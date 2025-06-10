# How Do Leader-Based Consensus Mechanisms Look Like on TEEs?

One of the main concerns about TEEs is that hardware gives inherently less guarantees around both integrity and confidentiality than math-based systems such as SNARKs. Proof systems are however not necessarily always the best approach for off-chain computations, specifically they're limited in proving performance.


We assume a setup where there's a single leader that can commit state for a given time relative to the application (e.g epoch or slot) and there's backup peers that need to validate the committed state.

The goal of the system is to make communication linear (O(n))

BFT systems like SBFT decouple commit from execution enabling to parallelize and guarantee building N+1 even if N wasn't executed yet.

Commit needs $3f + 1$ while execution needs $f + 1$ to guarantee acknoweldgement around the execution (i.e at least one honest node acknowledges the new state).

In a TEE based system the guarantee for liveness is slightly different. The system is safe for a certain number of hacked chips $h$ and can be made unavailable for a given number of faulty (i.e malicious) operators $f$.

The interesting part about applying BFT systems to TEE-based applications is that the pre-commit guarantees that hardware gives around the execution integrity is much greater than the one a single leader can offer in a standard leader-based BFT system.

For instance, SBFTs algorithm works roughtly as follows:

1. Client sends request to leader.
2. Leader builds block and sends it to its backup peers.
3. Peers use their (commit) threshold sig to sign over the proposed block and share the signature with the commit collector.
4. Commit collector combines the signature shares and sends the commit proof to all peers.
5. Having received the commit proof the peers can now execute the block proposed and sign over the new state with their (execution) thershold signature and send the certificate to the execution collector.
6. Execution collector combines the signatures into an execution proof. Note that here the threshold is $f+1$ because we just need a honest operator (i.e not $f$) to acknoweldge the new state. The execution collector can now share the new state certificate with peers and clients.

Here the client gets a response for their request in the last step but at step 4 client requests are already committed and are generally expected to be shortly collected by execution collectors unless network conditions changed. 

This means that until our requests get some level of confirmation we still need to await $3f+1$ peers to share their share of the signature. Network slots thus need to match a consensus round and before that there's no guarantees regarding the request the client sends.

In a TEE system however, we don't need the slots to match a consensus round, specifically we don't need the requests to be combined into a block for them to have some level of integrity guarantees. With this in mind, we can move towards a design where slots are much less frequent and are simply to offer an even safer level of confirmation.
