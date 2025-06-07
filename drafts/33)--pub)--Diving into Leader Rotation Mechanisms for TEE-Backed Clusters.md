`06/06/2025`

![](/images/wannarotate.png)

One common approach for applications that are based on TEEs is to gain performance by committing state through a single peer (leader peer), but relying on a fallback path in case the leader is unavailable and on leader rotation to mainly increase system security against physical attacks, but also to distribute the role across distinct points in the network topology.

> **TL;DR** Running peers inside an Intel TDX guest means we *already* get strong integrity and confidentiality guarantees.
> Instead of dragging in cryptographic functions (e.g VRFs, PVSS, BFT ballots, etc), we can select a leader path within one enclave and choose between different approaches to implementing the new path.
> The design aligns with the trust we inherit from TDs and does not *generally* add any setback in the threat model, this of course depends on the overall design of the application



# Some Possible Approaches

There's already extensive research for leader rotation mechanisms that we can borrow from existing decentralized systems, here I'm just describing some of the highlights with distinct charateristics.

## Leader-Based Protocols

Single leader protocols are inherently more adjacent to the purpose of TEEs: one leader peer proposes state commit and the peers verify the validity of the commit. When leader is unavailable or acts maliciously the peers can trigger a view change in the leader. 

Generally we have two types of leader-based mechanisms: stable and rotating.

### On Stable Leaders for TEE based Apps

This approach is usually the best path if we can assume the host operator of the leader to be honest because it guarantees better stability. A malicious leader is however much more dangerous on a stable vs rotating mechanisms because it can easily cause malicious behavior that is difficult to detect (e.g censorship). 

Placing trust in the host operator is something that modern TEE-based apps that intersect with web3 are trying to circumnvent, that said there's a few interesting points that relate to TEEs on the note of stable leaders:

1. Assuming the leader exists because they are privileged in that timeframe, e.g only leader allowed to commit state to a smart contract, some degree of leader rotation is needed to deter physical attacks. For this purpose stable mechanisms that only trigger view changes when they detect a problem don't work as well. 
2. Sophisticated censorship is fairly complex to perform. Because traffic is fully confidential (assuming the implementation grants it) and there are integrity guarantees around the code being executed it's tricky for a malicious operator to perform sophisicated censorship, i.e targeting specific packets or even users. This gets into the more complex discourse of CR in TEE apps (which we'll tackle shortly in other posts), but it's safe to assume that sophisticated systems will make it almost impossible for a malicious operator to carry censorship undetected unless they can physically compromise the chip. Either way, most protocols will likely resort to allowing social consensus to force-trigger view changes when needed, e.g if a leader is giving delayed availability (potentially on purpose).
3. TEE based systems by nature will have smaller quorum sets because economic security around integrity is already very high even with just a small threshold/MPC/SSS/other nodes. Given this (smaller quorums are generally more vulnerable) stable rotation mechanisms are actually a fairly good fit as long as the quorum that can trigger the view change is composed by parties that are not anonymous and are part of a privileged social layer. This means that networking overhead, especially for view changes, is generally not an issue.

#### PBFT & SBFT and Networking Overheads

PBFT and SBFT are both stable leader consensus mechanisms (imo two of the best), with the main difference being that PBFT has the "standard" quadratic O(n^2) communication in normal state while SBFT achieves O(n) (for context, like e.g HotStuff) using aggregated signatures. For both protocols newtorking overhead for view changes is O(n^2), even in SBFT where the critical path is O(n), mainly because the view to base off to needs to be targeted first.

Rotating leaders optimize for view changes as well because they aren't the exception but the rule, so they have linear comms. Though as noted in the above paragraph, I expect most TEE-based systems to have small quorums so O(n^2) still remains attractive, potentially even better due to less rtt for aggregations.

### On rotating leaders

In rotating leader protocols the view change is the rule of every consensus step (e.g block). As a result, this operation must be able to scale as part of the critical path.

However, this often sacrifices 1 roundtrip and O(n) might not be worth it when n is small. 

#### HotStuff

HotStuff is interesting in that similarly to SBFT it uses aggregated signatures for the critical path **also** for view changes, basically each peer uses their already aggregated threshold sigs as view certificate. So peer sends to leader fixed signature, leader combines the partial signatures into an aggregate threshold one and then forwards it back to the peers (O(1) <-> O(1)). tldr; linear comms but extra roundtrip.

---

## Randomness in Leader Rotation

Above we explore deterministic leader election mechanisms. However, in a TEE setting, the newly elected leader should be selected randomly because one of the primary concerns is physical compromise. 

If an attacker doesn't know which leader will be next they won't know which one to physically compromise, to be sure they'd have to compromise all of them (and in case one of them doesn't get compromised depending on the application guarantees and CR properties the attack might be stopped, but this is out of scope for this post).

Compared to determinstic rotation, random rotation is *usually* enforced for models where fault assumptions for sybil attacks revolves around weighing stake in randomness vs (possibly semi-)permissioned networks.

TEE systems are in my view a mix of the two, randomness grants better guarantees around the overall economic security of the system, while the network topology is *usually* similar to one of a BFT (semi)-permissioned network.

There's various ways to achieve this and again, we can borrow from the extensive existing research. Some highlights I personally like are SSLE and VRF sortitions.

### Leader Secrecy and TEEs

Both SSLE and VRF based solutions add overhead and use cryptographic functions to maintain secrecy around the newly elected leader and both introduce in their own way randomness.

However, the idea is that thanks to the confidentiality guarantees TEEs offer the schemes crypto-based approaches employ are not needed, so we can pick a leaner path. A valid concern is that mechanisms fully based on cryptographic primitives are arguably safer than what hardware can guarantee, but considering that in this scenario TEEs are not a dependency rather a requirement using TEE's confidentiality for this specific part of the system doesn't add any setbacks to the threat model. Using strong cryptographic based approaches for leader rotation would actually often result in an imbalanced system, i.e one where leader rotation is safer than other parts of the system that are more important (e.g confidentiality on application execution layer).


## Leader-less Protocols: ballots and tie-breaks

This is not really applicable to most TEE applications (interesting area to dive into tho) but ideally this is where I see many decentralized products pivot towards, i.e no single proposer rather a multi proposer mechanism where the canonical state depends on protocol-specific rules and tie breaks. Again, this approach has some drawbacks in a TEE based system, mainly it brings in overhead while it doesn't take advantage of the TEEs guarantees around integrity.

---

## Closing Thoughts

When talking about leader rotation mechanisms for TEEs, the best approach is probably to mix stable and rotation based approaches relying on the concept of epochs: stable leader with ability to trigger-change during epoch and for each epoch introduce randomness in electing a new leader. Some protocols might choose to rely on deterministic rotation as well depending on the requirements. One caveat of relying on TEE's integrity guarantees, is that for randomized leader election we won't need the extra roundtrips that other schemes generally introduce.

---

# Sketch: Simple Random Leader Rotation and Fallback

With the assumption that TEEs are confidential, we can design a very simplistic and effective approach to leader rotation keeping in mind that many applications will likely have strong availability guarantees by e.g having state replicated in peer nodes that can be elected as fallback leaders in case the leader is malicious or unavailable.

Assuming that we want a setup where:

1. leaders are stable within an epoch.
2. before the start of each epoch a leader is randomically derived.

Before the start of each epoch the cluster performs a lightweight leader-election routine within the enclave (e.g a trusted domain):

0. All peers in the quorum are mutually attested.
1. Every peer deterministically derives its own candidate rotation vector from a local entropy source.
2. Each peer gossips their candidate rotation path to its peers.
3. Peers perform a tie-break on all candidate values depending on internal protocol rules (e.g xor comparing lexicographically or any other comparison method). Winning rotation path is deterministic.
4. Depending on the application, there might be the need to post the rotation path to a higher authority (e.g a smart contract on Eth l1). Any peer, likely it would need to be the leader, can post the new rotation path posting the various messages signed by the peers. NB: there should be guards around replaying messages.

Another caveat is that this election process can be reused to fallback on another peer as primary in case social consensus or in-protocol network health checks (NB: imo the latter is tricky to achieve without it being a footgun) raise the need to do so. Rotate in similar way as stable leader mechanisms is especially useful in scenarios where there's a higher authority like a smart contract that orchestrates the state commits and ensures that only the leader can commit new state.

This procedure allows every node to pre-compute the upcoming leader and fallback order easily, and the implementation can be modified slightly to achieve different types of targeted networking overheads.

