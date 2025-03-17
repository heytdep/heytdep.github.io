`30/01/2025`

TEEs are awesome, but they can be compromised and in itself pose a challenge around availability that also greatly impacts security (NB: this last connection to security will be clearer as you read the write-up). This write-up aims to give a solution for a specific set of apps.

<img src="/images/17422425620882.jpg">

Before starting, we need to distinguish between:

1. compromised TEEs: chip is compromised through e.g physical attacks. For simplicity, I'll refer to this situation as *hacked node*.
2. compromised host: chips is not compromised but the host operating it. They can filter packages, dilute the TSC, deny availability, delay connections, etc. For simplicity, I'll refer to this situation as *malicious node*.

This write-up proposes a solution to both problems as long as **at least one party** within a select board of network nodes is **not malicious**.

Expectedly though, the discourse is a bit more nuanced when talking about requirements for system safety. I'll expand more about how a honest minority can outweigh a malicious+hacked majority towards the end of the article.

# Preamble

This system design cannot be blindly applied to any TEE application, and truth is it shouldn't. This system is designed for applications that need to secure financial value where TEEs security acts as an agent with control over the funds. 

TEEs are inherently vulnerable to a certain vector of physical attacks that are however expensive and time-consuming, as a result it makes sense to start protecting against hacked chips when the value of what's secured by the chip's integrity becomes greater than the cost-to-attack.

For what regards the software world, there are a few solutions that are being explored around protecting TEE-based systems from hacked chips including MPC techniques, FHE, or even a mix of the two. There's a lot of effort being carried also on the hardware side but it's out of scope for this write-up.

The approach I will discuss here is a simple MPC based technique coupled with some assumptions that we have around TEEs + the requirement of a committee of trustable parties; **There's an explanation for this committee** and it solely exists to tackle censorship resistance + doesn't need to necessarily have strict gated access. 

Another important consideration to be made is that this system's success lies in its ability to detect hacked chips within the system, not in safeguarding against them. Once that happens the system can be safely halted. Adding constraints around having to keep the system functional in the presence of hacked chips falls into trusted majority assumptions and consensus which we don't want to rely upon. Additionally, given the disincentive to spend lots of resources and money into freezing a system where you can't as the attacker get money out of it, such system is arguably as effective as one that includes the additional liveness constraint.

# How It Works: The Core of the Idea

The core of the idea is that we need to enforce determinism in some critical part of the application. Assuming that our application has a certain state ``S`` that manages the financial assets controlled by the TEE, and that state is computed with a generic transition function ``S_n = t(S_{n-1}, x_n)`` where ``n`` is an incremental checkpoint counter and ``x_n`` is the input set what ``t`` will apply to previous state ``S_{n-1}``, which roughly approximates to the actions that can be taken in the application that impact the financial value withheld.

If: 

1. We can enforce determinism in the computation of ``t``, so given an set ``x_n`` and a given checkpointed state ``S_{n-1}`` we should be able to replicate ``S_n`` across any other TEE.
2. ``x_n`` can be safely gossiped across TEEs with e.g pubkey encyrption with dstack shared secret.

> NB: enforcing determinism is sometimes easier than you might think. If your app relies on non-deterministic functionality as well e.g I/O, you can often isolate the non-deterministic functionality out of ``t`` and use other mechanisms to prove validity.

Then we can enforce this hacked-tee resistant system.

**How?** The catch here is that to steal financial value from our TEE-controlled system, the attacker needs their TEE to produce ``S`` so that the attacker can steal funds. If two TEEs compute different ``S_n`` given the same ``S_{n-1}`` and ``x_n`` inputs then one of the two chips has been hacked. 

More concretely, we assume a setup where we have non-connected vaults and where there is more than one TEE executing the application in CVMs that share a dstack secret. The hacked TEE proposes a withdrawal signed with the shared secret as a result of the computation of ``S_n``. If there is any other non hacked chip that knows ``S_{n-1}`` and ``x_n`` then that chip can invalidate the withdrawal using itself as well the dstack shared key. When this happens we can assume that one of the TEEs that have been onboarded in the application are hacked, so we can freeze the system and allow users to safely withdraw their funds.

This might not be the best resolution, but considering that for a system freeze to happen you'll have to spend a lot of time and money hacking one of the used chips, it's likely not worth it for the attacker to carry out the attack just to DoS. At that point, it's likely easier less expensive to just DoS (or even attack) the underlying chain.

This sounds great! But as you might notice there's a core aspect of this that must be tackled: for ``S_n`` to be computed by other TEEs the hacked chip should gossip ``x_n``. If said chip is hacked it's also safe to assume that it's also malicious, so the host will block all attempts to share ``x_n`` to the other peers. If the malicious + hacked TEE just doesn't share anything, then no peer will be able to call a "fault" in e.g a withdrawal. This moves the discourse onto the more nuanced aspect of availability and censorship resistance (CR). 

# Tackling Availability/CR

As said above, a key aspect for this approach to work is that the TEE proposing the change must share ``x_n`` (we assume that ``S_{n-1}`` can already be retrieved by other peers).

**Spoiler alert** is that there's no good way to do this, and the best we can do is rely on the committee I introduced in the beginning on the write-up + also rely on some inherent charateristics of the system we're working in.

One good way to tackle censorship on the hosts' end is to have a trusted majority within a select set of members of the social layer validate ``S_n``. 

> *How is this any good!? Isn't this what you said you didn't want to use when mentioning the trusted committee?*

Kind of. When introducing the trusted committee, I said that it was bad/non-trivial to rely on a trusted majority to find out whether a state computation was correct or not. In this situation, I'm saying that we can use a trusted majority to make sure that a certain number of TEEs received ``x_n``.

> *Didn't you mention just one non-malicious committee member in the beginning?!*

I did, and I mean it, I'll get there in a minute. The fact that we're relying on trusted majority for availability and not for actual verification of ``S`` means that we can take advantage of a few of things:

1. It's already very likely that the application will have some kind of social layer. Even just for upgrading the application, there will be a selected set of the social layer voting on changing application measurements.
2. The social committee does not actually have any power over user funds! They exist only to grant that a certain amount of TEEs received ``x_n``. committee members themsleves cannot even try to steal funds unless they also hacked the chips. In case the attacker can hack the majority of the chips operated by the trusted layer *and* compromise their hosts, they can try to carry out an attack. In reality however, this will only work if **all** of the members of the committee are malicious. I'll expand more about this in the next paragraph.
3. Since the committee signature alone is not enough and needs to be accompanied by a hardware RoT, it's much easier to add non-hacked chips to the committee than it is to add hacked chips.

In light of this, but especially due to point 3, we can **safely** have our system minimize the requirements to become part of the committee as much as possible; meaning that we can indeed obtain funds safety in systems where only one member of the committee is not malicious! To prove this, I'll sketch up the attack scenario.

### Preventing the attack.

This attack assumes that in a trusted committee of size 3, 2/3 operators plus the TEE operating the malicious withdrawal are hacked and compromised by the same attacker.

In such scenario, an attack vector is indeed viable:

1. hacked tee proposes malicious ``S_n`` that enables a malicious withdrawal to steal funds.
2. the 2/3 hacked committee chips sign off on the withdrawal and won't share with the non compromised chip the ``x_n`` they are using (which is of course signed by the tee proposing ``S_n``) trying to make it look like the good node is for example disconnected from its peers.
3. the only non hacked chip won't as a result receive an ``x_n`` that is valid (i.e signed by ``S_n`` proposer) thus can't compute the proof required (i.e ``S_n`` is malicious) to stop the withdrawal.

However, keeping the assumption that for the withdrawal to be valid the majority of chips operated by the trusted committee need to sign over ``S_n``, and that we have minimzed requirements to be part of the trusted committee, the non malicious operator can onboard new TEEs and/or committee members without requesting approval from other members of the committee (**NB**: actual numbers could change depending on system requirements) and turn the tables on the required majority for the withdrawal to go through. This means that also malicious operators can easily onboard new chips to try and "conquer" the trusted majority, but in reality it's significantly simpler and less expensive to onboard a non hacked node, meaning that the honest operator can easily outweigh the malicious ones on a purely theoretical basis.

<img src="/images/17422425620881.jpg">



## Why the committee Then?

Minimizing requirements to become committee could make you doubt the need to have an actual committee alltogether. In the above description of preventing the attack for instance, I've purposefully not specific what can the honest committe member onboard: is it associated TEEs? new members of the committee?

The only issue with this approach is that it can potentially cause DoS attacks. If a compromised committee member cuts connection to all of its associated TEEs so that these won't receive ``x_n`` they can call a fault on a withdrawal by outweighting majority with non-hacked yet malicious chips. Note that this won't freeze the system since the non-hacked malicious chips cannot prove invalidity of ``S_n`` over a signed ``x_n``. This means that if there was no committee at all, anyone that is willing to spend more than what the honest operators are spending together can dos the system until/unless the honest operators "double down" on the number of chips. This is a scenario that we don't want to create since it means onboarding a lot of TEEs unnecessarily (remember that the TEE-powered app is not a blockchain!).

Additionally, users would not be able to migrate to another non-DoSsed system because without a committee said DoS can be carried out in the new setup aswell. Introducing the committee is crucial to tackle this. Since it formed by reputable organizations and individuals, the committee is much less likely to provoke the above-described DoS attack since it would be public and was it provoked it is a discussion that can be settled on the social layer, for example users could migrate to a new system that does not include the DoSser in the committee. Having a committee also enables to safely fine-tune numbers; mainly those related to the requirements to onboard new nodes or committee operators. For example, a system could choose to only allow the DoS vector be possible if 2/5 of the committe operators are malicious and want to carry said attack. 

Hopefully, the reasoning behing the committee is now clear.

# A note on additional security

This system is actually quite composable. It really only consists on a set of TEEs verifying what a "leader" TEE is proposing. This means that we can add additional layers of security if needed such as for instance shuffled leader rotation or even adding another layer of MCP at "leader-level" if we deem that the system requires it.

# The Very Much Under-Discussed Networking Layer

Note that for all of the above to work a networking layer that deals with encryption, message signing, coupling dstack shared secrets with node-specific secrets, etc is essential. 

In the discussed approach the networking layer would require each onboarded node to broadcast received and produced messages to all other onboarded nodes + all those messages should be signed aswell. 

For example, when the leader gossips ``x_n`` to the chips associated with the committee in order for their computed ``S_n`` to be validated, they must sign ``x_n`` with their leader-specific key since for faults to be called verifier nodes must provide the input used signed by the leader to prove that the leader is indeed hacked. At the same time, all connected peers should gossip signed information they received to minimize the risk of some verifiers falling out of sync with the leader plus having more redundancy for resiting leader censorship (even though it's a very thin layer since malicious leader will also likely contro malicious committee nodes).

In general tho, I think that the importance of the networking layer is overlooked when talking about TEEs and a standardized P2P implementation would be a killer use-case; maybe I'll write more about it in the future. I'm building one, let me know if you would like to collaborate!
