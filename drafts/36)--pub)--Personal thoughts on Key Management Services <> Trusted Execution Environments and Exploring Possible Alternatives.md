`14/08/2025`

During SBC I had the plasure of seeing the Phala team's talk about Phala Cloud's current dstack implementation and what's in for the future; exciting stuff.

Part of the current (and future) implementation is the key management service (KMS). 

Overall during the stay I've been asked a few times about my idea of not explicitly relying on the KMS for tplus' architecture, so finally writing down some thoughts here as well as the alternative I came up with to see what other folks think.

# KMS

First things first, definitions. Maybe I'm going at this completely wrong because I haven't really gotten the full definition of the KMS; from my understanding the KMS serves the sole purpose of derivating keys. The KMS is stateless as far as other apps' keys go, the only state it persists is mainly the kms's certs, its signing key and the configuration. When an **attested** application requests the secret, the KMS computes it deterministically based on the application's virtual machine config (this is what phala uses to provision starting input to the TD so that such data doesn't need to be embedded in the os, which would indeed be tragic audit-wise). This means that any attested application that runs on dstack can persist their secret as long as the kms is alive, solving a bunch of availability issues that we encounter with TEEs:

1. Easy replication. Kind of implicit here but not having to manage the key in-app is indeed great.
2. Application shutdown (e.g due to bug, temporary fault in the servers, etc)
3. App updates.

Both 2/3 are eased by the KMS since the access control can be programmed through on-chain governance which is ultimately what I think most TEE-based systems will end up using (having 0 safety guards on that end is quite unsafe if the code is complex and deals with I/O like most TEE use-cases unless you write perfect code == always unsafe :p ).

# Some Thoughts

All of this is really cool, but there's a few arguments if you approach the problem from a different perspective.

## Concentration of secrets

In a purely game-theoretic scenario (though I know it's not directly applicable to this setup, working with TEEs inherently includes trust in the equation), having a single secret on a TD that generates n secrets where n is the amount of apps that rely on such TD to derive their key destabilizes the game's equilibrium. The incentives to attack a single KMS grow (non-linearly) with the amount of applications that rely on such KMS. 

That said, **there's two great things here tho that kind of invalidate this argument**:

1. Each application can specify its own KMS or opt out from it, dstack doesn't force you to use a certain KMS (thanks Phala team!).
2. We can try to re-establish equilibrium by increasing security over the KMS itself thorugh the many available secret masking strategies that exist out there and split the derivation secret across multiple TDs; key derivation is a one-time-per-vm thing anyways so doesn't need to be fast. This way, incentive grows but so does attack complexity.

## Higher safety at KMS-level != Higher Safety for App

Just mentioning this point here because the KMS forwards the full secret to the application, thus the safety conditions for your app's infra against physical attacks remain the same. Remember that safety != liveness, KMS as is tackels liveness making the dev experience better in general.

## About Crash-Tolerant Systems

When you choose to rely on a KMS for liveness you basically relay the liveness constraints over to the KMS's infrastructure. At that point the KMS becomes part of your application's system. 

For applications that secure large financial values having a liveness + integrity guarantee around such financial data (e.g signers tied to on-chain vaults) it's crucial that this data can be retrieved even when the system **fully** shuts down. 
The KMS might have better liveness that the application in itself, but does it grant the same liveness guarantees as the chain to which the signers are tied to? Likely not, and that's fine.

The point I'm getting to is that many of such TEE-based apps will want to have a non-TEE dependant way of bootstrapping their protocols in case of a full system shutdown (including the KMS). Even tying the keys to specific hardware by leveraging sgx's sealing capabilities and build the KMS on that is too risk when you think that only that specific `MRENCLAVE` can start recovery.

That's why we talk about distributed recovery protocols like Paxos or Raft. In certain scenarios however, we can explore different solutions that are simpler and yield promising results.

### Possible no-kms solution.

Think of the following (pretty common) generic TEE-based application setup: 

Let $\Pi$ be our protocol:

- $\Pi$ runs trusted domain(s) that maintain some internal encrypted state that mutates through user inputs and which maintains a signing key that allows to perform authorized actions on an ethereum smart contract.
- $\Pi$ implements measures so that the trusted domains can attest as to which version of the internal application's state they are maintaining and among a set of such versions the smart contract is able to determine which one is the latest.
- $\Pi$ implements functionality that allows for state replication aross trusted domains.

We generally have two ways of controlling our ethereum smart contract:

1. Keep the signing key within the trusted domains and gossip (e.g DH key exchange) it across attested peers.
2. Have trusted domains attest on-chain to our contract (+potential additional requirements, but that takes us out of the generic context I'm setting up), once attested the specific trusted domain's signing key becomes (one of) the signer in the contract.

For 2 to work however, we should be guaranteed that the trusted domain that is now operating as a privileged signer on the contract is running the latest version of state. This is crucial in many cases to preserve integrity of the app's users state (e.g think of user balances). This is within the assumptions that we're making for $\Pi$.

If we go with 1, we can rely on the KMS to keep the signing key. This grants good liveness guarantees but due to the above-mentioned reasons it's not enough.

We can still tackle 2 through the KMS: peers within $\Pi$ dump state that's encrypted with the KMS' provisioned key to persistent storage and then if the app crashes, anyone with access to persistent storage can ask for the secret trough the KMS and decrypt state, the new trusted domain can now attest to the smart contract that they're running the latest version of the app's state. 

Not good enough tho, we're still relying on the KMS to be available. 

What about re-using this last approach but with the state dumps encrypted to multiple secret shards that are provisioned to members of a council when dumping such data (really, they just need to be gossiped with e.g DH)?

But a council is no good? Well, not really. If we're talking about liveness **(and safety!!)** in TEEs, almost every complex $\Pi$ will have some concept of council/governance/quorum/trust relation/federation whatever you want to call it. Why? Even just updating the trusted domain measurements would require to introduce such concept, and guess what's one of the reasons why we're scared that trusted domains will crash? Bugs that need to be patched and whose patches need to be shipped to production. That's something the KMS relies on as well!

Anyways, the council is especially helpful here for applications whose encrypted state is not overly sensitive. 
In the case that the encrypted state was indeed highly sensitive, the attack complexity doesn't really change; only the attack surface over time:

1. With KMS: adversary would need to takeover governance, update measurements and trick the KMS into granting the singing key to a malicious trusted domain.
2. With shared secret: adversary takes over shared secret threshold and decrypts the data directly.

Note that since we're strictly talking about confidentiality of encrypted data dumps that may already be in the hands of the adversary (in fact, it should be in the hands of everyone!), the KMS can't employ many counter measures. 

The only thing that changes here is that implementing secret sharing so that it mimics complex governance is more difficult and that attacking on shared secrets whose shards are intelligently handled to mimic complex governance systems can be retroactive:

- t1: $\Pi$'s state dumps are published and encrypted to 3 shards with a threshold of 2.
- t2: $\Pi$ improves its council's safety so from now on the state dumps are encrypted to 10 shards with a threshold of 7.

In the above scenario the adversary can attach state between t1 and t2 by just compromising 2 "old shards". If this was handled by governance, adversary would only be able to attack t1 < t < t2 in between that timeframe with the same ease.

# Conclusion

There's no perfect solution, but for some $\Pi$s the KMS might or might not be the best solution depending on the preconditions the specific application grants us. Curious to see what others think in relation to what they're building, I'm sure there's many other different perspectives that I am missing. For example, if $\Pi$ must guaratnee confidentiality at all costs, a redundant SGX-based kms setup might be the way to go. 

In the case of Tplus however, the worst that can happen if the adversary takes over the secret shards is plaintext user inventories. This ability to have some slack on the confidentiality end of things led us to the thought process behind the above-described behavior.

I'd expect the KMS to still be the best solution for most applications and a very good way to offload availability concerns elsewhere and props to the Phala team for having it well-integrated [**and audited**](https://x.com/PhalaNetwork/status/1955805066985939139) within dstack.
