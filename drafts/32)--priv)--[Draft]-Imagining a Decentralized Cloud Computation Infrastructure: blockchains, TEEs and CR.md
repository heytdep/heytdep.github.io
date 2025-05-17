`17/05/2025`

> Disclaimer: this post aims to start an open discussion by bringing in some sparse concepts. 

Won't introduct decentralized systems and why they're crucial, but right now there's one main issue with existing **decentralized** infrastructure which is the need for nodes to agree on all committed state. This is obviously crucial for decentralized systems because we cannot assume any guarantees regarding execution integrity, meaning that any kind of change needs to be verified within the consensus quorum. 

One consequence of this is that execution needs to be deterministic in all of its aspects which limits what can actually happen in a decentralized setting impacting not only functionality but also performance due to flow of execution, i.e we need to organize transactions in blocks, the set of rules must be well defined, every instruciton executing must abide to such definition and interface (which has its advantages like smart contract composability), etc.

This is great but it's not really how the internet we know works. In blockchains, applications can talk to each other because they are literally executing the exact same thing only with a different set of parameters (e.g different smart contract binaries). If I want two contracts to communicate by making a cross contract call, what generally happens in smart contract VMs is:

1. vm for A is instantiated and executes code for A until it reaches host instruction to call B.
2. vm for B is instantiated + there's (generally) a context switch (e.g new frame) for B's execution; but it remains on the same host.
3. after execution B's context is popped and if the call failed the host rolls back   to the state previous to B's execution (generally kept at the beginning of point 2).

This works because A and B follow the very same limited and safe instructions set producing effects that affect the chain's state as a whole without caring if it's an effect of A or B. This is the effect of generalizing over the execution; this step is mandatory because we cannot have a quorum set for every tiny application or use case, so we generalize over what can be done and pull everything together so that any node in the network can execute any instruciton.

This is great and works well for many financial applications, but it's not how the internet works. 

When we use google's login to access excalidraw, google oauth doesn't really care that excalidraw can verify the effects of the login and neither does excalidraw since there's inherent trust in google's service (e.g if google's authentication is hacked (having worked on security research as well, this is obviously a simplification but bear with me), excalidraw is as well).

We unfortunately cannot fit in a generalized VM execution any potential behavior that's executed outside the client + require determinism in the execution, thus we cannot build a decentralized infrastructure that works for behavior that is more complex than simple modifications to local data.

More importantly even for what regards scaling, if we were to be able to do the above it wouldn't be feasible to scale. 

So for using cloud infra, is the only solution to trust centralized services?

Not necessarily. 

> NB: This post does make some assumptions that are not feasible in the short term with the biggest one being the assumption that corporations want to collaborate on not extracting value from its users (e.g monetizing on user data, hiding malicious behavior, keeping code closed source etc).

# The Main Idea

Verifying every ounce of cloud behavior like we do in blockchains is not a good idea for complex systems:

1. It obviously doesn't scale.
2. It's unfeasible execution wise to denominate a common ground for effects.

What *I think* we really want to achieve is a decentralized system where:

1. [prerequisite] Code running on it must be open source. If not, it should at least go through an execution layer that can attest to the effects of such code but this is complex and no different than trying to generalize on execution.
2. There's no notion of global state. Every application should have its own implementations and effects.
3. Nodes in the network don't need to re-execute every input coming from their peers because there's no global state.
4. The network topology is defined by programmable trust relations among peers, it shouldn't be limited to proof of stake/work protocols but should factor in other charateristics (trust in the underlying entity, monetary/strategic agreements, uptime, etc). 
5. The network is actually just a big chunk of social consensus that:
    1. allows users to make informed decisions about which applications to use.
    2. allows to counter censorship.
    3. does not guarantee any integrity on the execution.


Hopefully this sounds like a good way to tackle the ever increasing risk of cloud infra monopoly and centralization, but there's two main issues:

1. What does the "network" actually do if it cannot guarantee integrity?!
2. How is this any different than trusting centralized servers since we don't have global verifiable state?

## The current system

Right now we have blockchains that roughly look like the following:

![](/assets/17471611983529.jpg)

This means that every node that wants to participate in consensus needs to receive, execute and locally apply the effects of each transaction that is sent over the network which is as I mentioned above an issue for both scaling and functionality. 

Additionally, to participate in a consensus where nodes can easily be byzantine to exploit the underlying protocol, nodes are incentivized to behave correctly given existing incentives and potentially the risk of being slashed (e.g of either monetary stake or reputation). Incentive to behave correctly means that nodes need some level of requirements to participate making in some scenarios the bar to participate in consensus high (expensive hardware, lots of stake, both or more).

In my opinion, one of the best things about crypto is that we can enforce a fair social consensus guaranteed through determinism, which at the end of the day depends on the fact that inputs to transition functions can be cryptographically verified, i.e txs are signed. When we send a transaction we can have cryptographic proof that such action has been authorized by the rules of the system. The reason why we need consensus is to decide the transactions to be applied and the state on which to apply them. However, there's an important distinction to be made regarding the nature of these two values that need to be agreed upon: while (valid) transactions are signed by all the parties involved in the transaction so a 3rd party (i.e node) cannot maliciously craft a valid transaction, the new proposed state assumes trust in the execution layer of the node that roughly takes as input the txs and the initial state producing a new state that can or cannot be exact according to the protocol's rules. Hence, agreeing upon which transactions to apply really is to enable nodes to agree on their effects because we already know the inputs to be valid. 

This yields another really neat property: an honest external observer can detect which state is correct as a result of the transition function assuming they know which transactions where applied and what the initial state is, i.e the rules are open and openly verifiable and observable. This is why, **if we assume ideal networking conditions**, in a cluster where all nodes are honest consensus would generally resolve without any disputes or mismatching results. 

Additionally, since every piece of data on the blockchain has value because the market assigns such value blockchains have the advantage of being able to rely on what some call "social consensus". When nodes in a protocol have diverging rules (either because of an attempted attack or because of some fork) they ultimately either converge into a single canonical view or we see the network "splitting" (NB: here by splitting I effectively mean that it's two completely different networks). 

# What to Do?

Now that I've reharsed on how most existing decentralized systems work at a very high level, we can actually go about seeing what we can do to achieve the infra I described in the [main idea](#the-main-idea). 

We've looked at how we don't want a global network state that needs to be agreed upon, but we haven't really talked about how we can make this happen. Again, we need some level of guarantees around execution integrity and not having global state but application specific one means that each application should have the same level of decentralization while relying on a diverse set of nodes to validate integrity. This is obviously not feasible as well. 

What might be feasible on the other hand is to have such applications with a baseline of execution integrity without requiring any consensus on the execution. 

This is where hardware-based solutions (mostly TEEs) come in. TEEs allow us to guarantee integrity on the exeuction without generally relying on consensus. [TEEs aren't perfect](https://forum.teekettle.org/t/ttee-update-on-technical-work/18) so there are other things to factor in before assuming that integrity can be fully granted but I'll get there later on.

When talking about TEEs there's three main factors to consider:

1. Code complexity is *generally* much higher (also involves I/O) == it's much simpler for code running in a TEE to have vulnerabilities.
2. TEEs themselves can have vulnerabilities!!
3. TEEs offer great guarantees around integrity and confidentiality but none about availability. Here we enter the CR territory.

# Code Complexity

This is a fairly complex topic that I think hasn't been really well-expored yet. When we talk about TEEs in the web3 world we usually mean a server where the OS is measured and locked down to prevent tampering with the expected behavior. For example, when using Intel TDX we launch a trusted domain where:

1. the TD VF code and its configurations are measured.
2. OS loader is measured.
3. OS kernel, initial memory, and app itself.
4. Some designs also measure extra stuff, e.g a vm settings hash.

In such a setting, TDs execute complex code in a fully locked down system. Anyone that has worked on complex cloud systems before knows that this is indeed a difficult problem to solve. On one hand you have to manage dependencies upgrades, bugfixes, configuration issues, etc in an agile manner, on the other hand you must ensure that this process is decentralized and keeps the integrity+confidentiality (and safety) guarantees expected from the app.

Every action that you take in one of the two directions seems to put you farther away from the other. Generally you shouldn't want an operator to upgrade the code or snoop into the application logs, but you also don't want the application to be a black box that no one knows how to debug when something fails. If you choose to open up some logs, you start getting into the risk of breaking confidentiality so you need to be careful about what you do. Sometimes you can split the application under different components where some can be upgraded more easily than others and that's good, but this is still an ongoing problem that doesn't really have a solution. 

We should probably kickstart a more structured discussion about this on the TEE chats.

# TEEs have vulns!!

This is probably the biggest problem that the general public is worried about for TEEs. Honestly there's a broad range of topics to pick here: attacks on the supply chain, key generation that is truly trustless, non-invasive/invasive physical attacks, software attacks, etc.

While there's ongoing effort on the hardware side to harden the tech, there's also some protections that can be used by the applications to provide more safety around the state they commit. Mainly, the most common approaches that are being currently researched are FHE and MPC based techniques. Basically you split critical parts of the application in a way that more than one node must be compromised by an attacker in order to commit invalid state, greatly impacting the cost to attack and its complexity.

We've internally been researching some of these approaches and personally one of my favorite is simply introducing a layer of verifier peers that run within a reputable layer of social consensus. How does this work?

### Small Excursus on a Possible Solution

The core of the idea is that we need to enforce determinism in some critical part of the application. Assuming that our application has a certain state ``S`` that manages the financial assets controlled by the TEE, and that state is computed with a generic transition function ``S_n = t(S_{n-1}, x_n)`` where ``n`` is an incremental checkpoint counter and ``x_n`` is the input set what ``t`` will apply to previous state ``S_{n-1}``, which roughly approximates to the actions that can be taken in the application that impact the financial value withheld.

If: 

1. We can enforce determinism in the computation of ``t``, so given an set ``x_n`` and a given checkpointed state ``S_{n-1}`` we should be able to replicate ``S_n`` across any other TEE.
2. ``x_n`` can be safely gossiped across TEEs with e.g pubkey encyrption with dstack shared secret.

> NB: enforcing determinism is sometimes easier than you might think. If your app relies on non-deterministic functionality as well e.g I/O, you can often isolate the non-deterministic functionality out of ``t`` and use other mechanisms to prove validity.

Then we can enforce this hacked-tee resistant system.

**How?** The catch here is that for an attacker to e.g to steal financial value from our TEE-controlled system, the attacker needs their TEE to produce ``S`` so that the attacker can steal funds. If two TEEs compute different ``S_n`` given the same ``S_{n-1}`` and ``x_n`` inputs then one of the two chips has been hacked. 

More concretely as an example, we assume a setup where we have non-connected vaults and where there is more than one TEE executing the application in CVMs that share a dstack secret. The hacked TEE proposes a withdrawal signed with the shared secret as a result of the computation of ``S_n``. If there is any other non hacked chip that knows ``S_{n-1}`` and ``x_n`` then that chip can invalidate the withrawal using itself as well the dstack shared key. When this happens we can assume that one of the TEEs that have been onboarded in the application are hacked, so we can freeze the system and allow users to safely withdraw their funds (there's various ways one can achieve it, not in the scope of this post tho).

This might not be the best resolution, but considering that for a system freeze to happen you'll have to spend a lot of time and money hacking one of the used chips, it's likely not worth it for the attacker to carry out the attack just to DoS. At that point, it's likely easier less expensive to just DoS (or even attack) the underlying chain.

This sounds great! But as you might notice there's a core aspect of this that must be tackled: for ``S_n`` to be computed by other TEEs the hacked chip should gossip ``x_n``. If said chip is hacked it's also safe to assume that it's also malicious, so the host will block all attempts to share ``x_n`` to the other peers. If the malicious + hacked TEE just doesn't share anything, then no peer will be able to call a "fault" in e.g a withdrawal. 

This moves the discourse onto the more nuanced aspect of availability and censorship resistance (CR). 

# We Don't Have Availability Guarantees!!

Applications that are designed to run on TEEs generally take the approach of a single server committing state. This state as shown above can also be potentially verified by a small set of peers in a way that even if only one of them has been hacked we can freeze the system. However, since there's no notion of consensus going on, while we can inherit the integrity guarantees from the TEE, we loose another important charateristic of what consensus achieves (or at least should): censorship resistance. How would we achieve this when it's a single node committing the state? We can't!

To be honest, the more I think of it, there's no good way of tackling CR without relying on having many nodes. That said, with TEEs we have one fairly important charateristic around the messages that are being exchanged between peers, which is the message's inherent confidentiality. If the host cannot decrypt the content of the messages, the censorship they can carry out is fairly limited if the application employs effective networking techniques. In fact a malicious host can only try to understand the messaging patterns and filter by package size or source. All this can be masked if we require the networking layer to obfuscate the traffic by e.g keeping fixed packet sizes, constant traffic, etc. This unfortunately comes at the expense of unneded traffic on the nodes. I'm not familiar with what's the best way to achieve this, but for instance Flashbots [is working on this](https://collective.flashbots.net/t/network-level-censorship-understanding-the-problem/4769) as well.

Anyways, in my opinion the best way to tackle CR for TEE applications is very similar to how we tackle it in blockchain P2P systems, including:

1. (randomized) quadratic messaging and message relays.
2. some nodes weight more than others in the CR discourse (based on consensus algorithm).

We can try to squeeze these two charateristics in TEE applications as well. 

### Small Excursus on a Possible Solution

In fact, one good way to tackle censorship on the hosts’ end is to have a trusted majority within a select set of members of the social layer validate ``S_n``.

Note that in this situation, I’m not saying that we can use a trusted majority to give integrity to the application state, but only make sure that a certain number of TEEs received ``x_n``, or in general, that a message has been broadcasted by a node or not.The fact that we’re relying on trusted majority for availability and not for actual verification of S means that we can take advantage of a few of things:

1. It’s already very likely that the application will have some kind of social layer. Even just for upgrading the application, there will be a selected set of the social layer voting on changing application measurements.
2. The social committee does not actually have any power over user funds/internal application state! They exist only to grant that a certain amount of TEEs received a message. Committee members themsleves cannot even try to steal funds unless they also hacked the chips. In case the attacker can hack the majority of the chips operated by the trusted layer and compromise their hosts, they can try to carry out an attack. In reality however, this will only work if all of the members of the committee are malicious. I’ll expand more about this in the next paragraph.
3. Since the committee signature alone is not enough and needs to be accompanied by a hardware RoT, it’s much easier to add non-hacked chips to the committee than it is to add hacked chips.

In light of this, but especially due to point 3, we can safely have our system minimize the requirements to become part of the committee as much as possible; meaning that we can indeed obtain funds safety in systems where only one member of the committee is not malicious! To prove this, I’ll sketch up the attack scenario.

#### Attack Description and Prevention

This attack assumes that in a trusted committee of size 3, 2/3 operators plus the TEE operating the malicious withdrawal are hacked and compromised by the same attacker.

In such scenario, an attack vector is indeed viable:

1. hacked tee proposes malicious S_n that enables a malicious withdrawal to steal funds.
2. the 2/3 hacked committee chips sign off on the withdrawal and won’t share with the non compromised chip the x_n they are using (which is of course signed by the tee proposing S_n) trying to make it look like the good node is for example disconnected from its peers.
3. the only non hacked chip won’t as a result receive an x_n that is valid (i.e signed by S_n proposer) thus can’t compute the proof required (i.e S_n is malicious) to stop the withdrawal.

However, keeping the assumption that for the withdrawal to be valid the majority of chips operated by the trusted committee need to sign over S_n, and that we have minimzed requirements to be part of the trusted committee, the non malicious operator can onboard new TEEs and/or committee members without requesting approval from other members of the committee (NB: actual numbers could change depending on system requirements) and turn the tables on the required majority for the withdrawal to go through. This means that also malicious operators can easily onboard new chips to try and “conquer” the trusted majority, but in reality it’s significantly simpler and less expensive to onboard a non hacked node, meaning that the honest operator can easily outweigh the malicious ones on a purely theoretical basis.

![](/assets/17473364079880.jpg)

### Why the committee Then?

Minimizing requirements to become committee could make you doubt the need to have an actual committee alltogether. In the above description of preventing the attack for instance, I’ve purposefully not specific what can the honest committe member onboard: is it associated TEEs? new members of the committee?

The only issue with this approach is that it can potentially cause DoS attacks. If a compromised committee member cuts connection to all of its associated TEEs so that these won’t receive x_n they can call a fault on a withdrawal by outweighting majority with non-hacked yet malicious chips. Note that this won’t freeze the system since the non-hacked malicious chips cannot prove invalidity of S_n over a signed x_n. This means that if there was no committee at all, anyone that is willing to spend more than what the honest operators are spending together can dos the system until/unless the honest operators “double down” on the number of chips. This is a scenario that we don’t want to create since it means onboarding a lot of TEEs unnecessarily (remember that the TEE-powered app is not a blockchain!).

Additionally, users would not be able to migrate to another non-DoSsed system because without a committee said DoS can be carried out in the new setup aswell. Introducing the committee is crucial to tackle this. Since it formed by reputable organizations and individuals, the committee is much less likely to provoke the above-described DoS attack since it would be public and was it provoked it is a discussion that can be settled on the social layer, for example users could migrate to a new system that does not include the DoSser in the committee. Having a committee also enables to safely fine-tune numbers; mainly those related to the requirements to onboard new nodes or committee operators. For example, a system could choose to only allow the DoS vector be possible if 2/5 of the committe operators are malicious and want to carry said attack.

Hopefully, the reasoning behing the committee is now clear.

### A note on additional security

This system is actually quite composable. It really only consists on a set of TEEs verifying what a “leader” TEE is proposing. This means that we can add additional layers of security if needed such as for instance shuffled leader rotation or even adding another layer of MCP at “leader-level” if we deem that the system requires it.


<hr/>

Either way, the two above solutions are just a way to show that we can indeed do something about these limitations of TEEs. 

**Now back to the decentralized cloud system**.

# The Decentralized TEE-Based Infrastrcuture.

Let's wrap a couple of concepts from the previous section first:

1. the more entities we have that weight in social consensus, the more difficult it is to carry out censorship.
2. we can leverage the CR layer to enforce stronger integrity guarantees also in the presence of hacked chips.

Now, imagine that we want the various TEE applications that are being developed to interact with each other in the same way that smart contracts interact with each other within a single chain, i.e with the same security guarantees.

Such system would be highly interoperable simply by forwarding web requests, exactly like the internet we all know today. But this approach goes beyond simply serving web requests. This approach works but it inherits the same issues that web2 today has around availability: one server can just shut down and choose to stop serving another, which specifically in the case of locked down OS running on a TEE would be catastrophic because every single dependency could infer on the dependant's availability.

How do we solve this? There's two things here:

1. Trustless replication.
2. Censorship resistance.

## Trustless Replication

CVM replication has been a hot topic in the TEE world for a while and it's arguably  one of those designs that are simple and effective that everyone wants to use. Bascially, servers that are running the same code in a TEE can attest to that, allowing other nodes to share state through an encrypted channel safely directly to another TEE. This maintains the confidentiality of the data and allows anyone to onboard as long as there's no censroship happening.

## Censorship Resistance Layer

If you recall, in the beginning of the post I mentioned that what I imagined was a decentralized infrastructure, not just standalone cloud applications that run with confidentiality/integrity guarantees. In fact, what if we had a higher level consensus that tackles availability shared by TEE applications that are part of a quorum? In a scenario where availability is guaranteed we can easily have interoperation across TEE applications.

For example, imagine an automated trading strategy that uses [t+](https://tplus.cx)   as exchange. The trading strategy needs assurances that when they forward a trade to t+ on behalf of their users that trade is actually received and committed to t+ state. If we assume censorship resistance, the simplest way to interop is to just base off standard requests.

In general, I'd imagine three scenarios that form when standalone applications need to interact with eachother:

1. just base off requests (maybe threshold send to multiple nodes).
2. app state is merklized, app MPC commits root state and root state is shared to peers. Then user provides inclusion proof. 
3. no interop because it's not supported at app layer => apps choose if and how they can interop with others.

A similar approach can be taken also for execution layers like smart contract platforms that just want to scale by employing some of the charateristics of the system I described in the beginning of the article, mainly:

1. Lack global state. Every node has its own state and archival properties.
2. Nodes in the network don't need to re-execute every input coming from their peers (because there's no global state).

Imagine the following approach:

![](/assets/17474136065682.jpg)

Here node 1 and 2 don't need to share the same state, just an endorsed root of each other's state. When node 2 wants to use state from node 1, if 1 is in 2's quorum then 2 has already pulled in the latest state committed by 1. Practically however, this bring in the good old problem of dealing with potentially outdated state. One could guarantee atomicity in the execution but if the state is outdated then you're vulnerable to a broad range of attacks. From some very superficial thinking, there's ways one can mitigate this but it doesn't seem possible to fully solve the problem without relying on some level of sequencing around the operations (even if minimal). It might be worth it to explore the latter in the future. For example, we could have a node type that is able to bundle states from multiple nodes which awaits for the involved nodes to process the bundled requests.

Anyways, having a broad and diverse set of nodes that operate different applications as censorship resistance layer allows for both network obfuscation techniques and identifying censorship to be more effective. If anyone is interested in discussing how to best leverage said layer to actually implement censorship resistance let me know!

# The Social Layer

What we discussed until now is basically that we get integrity (and if needed confidentiality) guarantees from the hardware which allows us to have much less to worry about when committing state (which also == no need to have a VM, less to worry about determinism), and availability is tackled by a consensus layer.

I briefly described some of the ways such layer can handle censorship and there can easily be others that apply, but in general the important concept in my opinion is that having a diverse set of operators that stake their reputation in **only** tackling availability (and not integrity) makes up for a very solid CR layer:

1. incentive to misbehave is extremely low because it will never directly affect integrity, an example is the attack vector I described here.
2. since there's nothing to execute the gadget that applications can add to participate is very low overhead and programmable on a per-message basis. E.g we can easily embed functionality that attests to having received and forwarded a message.

But how does this social layer look like?

Consensus layers where you stake reputation are sometimes stronger than their PoS/PoW counterparts depending on the topology of the quorum in relation to what it secures. In this case, any form of proof of agreement is superior. Granted that TEE applications will generally at least have a set of operators that coincides with the internal team or comp which have a reputation to maintain, taking over consensus does not directly imply ability to steal funds. This means that:

1. the protocol can already count on participation from other applications because it would allow those apps themselves to have a CR layer that blends in with operators that can have a different nature (completely different apps).
2. to join you need to be trusted by someone that is trusted among the quorum. Acting byzantine would cause massive reputation loss and yield 0 profits.

Is this a permisisoned network? Not necessarily. Federated Byzantine Agreement (FBA) is an example of how reputation based consensus can be permissionless. 

The main idea of a FBA system is that each node defines its own quorum slices == set of nodes that they trust to reach consensus. Effectively, the quorum slice is a subset of the quorum that convinces a particular node (the trustee) of agreement. A node being able to select their own quorum slice set allows quorums to naturally emerge from local choices, meaning that each node can choose who they trust based on reputation or financial arrangements and the intersection will form the quorum, being the set of nodes sufficient to reach agreement. For those who are interested, I wrote some more about FBAS [here](https://forum.teekettle.org/t/trustless-tees-today-ztee-ledger-masking-and-quorum-rotation/40#p-58-fbas-14). 

Implementing a FBA system effectively allows for a subset of TEE operators to emerge as a reputable layer to tackle censorship while also allowing other operators that might not be initially trusted to participate with own view as well until they are (if ever) included in the trust intersection.
