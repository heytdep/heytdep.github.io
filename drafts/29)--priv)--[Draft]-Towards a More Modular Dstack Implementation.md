@tdep

# Towards a More Modular Dstack Implementation

A while ago, I started playing around a modular dstack implementation. The core idea is that when deploying to TEEs, each complex enough application will have a lot of requirements on application layer.

For those who are unfamiliar, currently when we talk about dstack there's a division between system layer and application layer. The system layer's goal is to have a secure operating system that enforces assumptions of dstack + manage the application layer by allowing to deploy arbitrary code. Application layer on the other hand is the app's own logic. Splitting the two comes really in handy for replicability, but mainly auditing since xoring the two impacts the measurements too.

With that in mind, in my opinion, having a single system layer for auditability purposes works best only when either the system is as minimalist as possible, or when the system is heavily modularized so that the actual build can differ based off the required functionality. 

## Goal

The goal is to have a dstack implementation that:

1. works on any deployment (bare metal + cloud TDs).
2. allows an app to be deployed on any platform and have the same outcome.
3. allows the implementor to decide which enshrinement to have.
4. (extra) cross platform!? more on this in end of write-up and probably in the future.

2 is actually dependant on 1, and 3 has various ways of being enforced. 

## Vendor Compatibility

This is crucial for any complex application that wants to run as distributed as possible across vendors to minimize the inherent availability issue with TEE applications. Since cloud trusted domains don't allow the implementor to have control over the hypervisor we need to infer the virtual machine metadata at runtime.

Personally, the following workflow seems to be what works best:

1. Build image with app-layer manager to launch on startup.
2. App-layer manager serves an endpoint that accepts anyone to pass data. For more secure communication we can also add functionality to hardcode only an allowed signer/list of singers to post the VM metadata. This requires a different but works good since it can easily be replicable if the implementor posts the signed payload onchain.
3. Once the metadata is received, shut down the endpoint and measure it into rtmr3.


## Same Outcome and Minimal Implementation

A reasonable assumption is that app developers conform to an interface to communicate with the system, and expect that interface to work across all dstack deployments. This maximazes compatibility and development ease.

That said, it's tempting to offload as much work from the app implementor to the system by enshrining the system. I personally think we should not do this. The system should be as minimal as possible to have it easily audited, especially if we switch to using mkosi and only base off already audited implementations.

There's two options here:

1. modularize the system.
2. minimize the sytem.

Both work and each has its own advantage, and the best thing is that they are not mutually exclusive! If we strive for 1, then we can easily tackle 2 and let implementors decide what to use.

Modularization can be achieved within different layers. My inital idea for rs-modular-dstack was on guest backend level. However, another alternative could be to reuse yocto's layer functionality which itself allows for highly modularized and reusable guest images. Similarly to how yocto did with poky, we can have:

- a minimal layer for os implementation (e.g poky) + minimal cvm backend (can be modularized too, but in general, I think that just exposing a get_quote endpoint is what we want here).
- a layer for aTLS.
- a layer for the KMS.
- a layer for the reverse proxy.
- etc

This allows teams to build and deploy across vendors images with different requirements without having to audit the sytem everytime, but also without having to inherit functionality that they don't want to rely upon. This also allows mantienance to be split and more agile since layers don't all depend on eachother. 

### Why minimal system?

The main idea is that we cannot expect each application to follow many standards, some will need to enshrine things at application level and are not going to want to follow standards because those might not work well with their own implementation.

For instance, we're going to deal with replication by establishing a P2P network that can e.g follow Bracha's broadcasting algorithm, and we want to do replication within the P2P network through mutual attestation. In a setup like this, we don't want to have the burden of dealing with another kind of replication. Another example is the KMS. We might want to have a persistent key using SGX's persistent sealing keys, or not want that at all or use a TD as KMS. A lot of this also depends on application layer! There's ways we can backup state without having KMS. 

## Cross platform?

There have been a few stabs at abstracting over the platform specific attestation. If we want to support not only trusted domains, but also other TEE platforms, one approach worth exploring is the vTPM <> TEE combined attestation. Basically, vTPM enclave and app enclave mutually attest, establish secure comms, and the app enclave measurements are extended into the TPM attestation registry. This was downstream only has to check against TPM quotes and we can reuse the vTPM across many VMs. 

I wrote a bit about vTPM [here](https://heytdep.github.io/post/27/post.html) but in general it's something we might want to discuss.

<br/>

# What's next?

Imo, the above would only be the start and what regards system-layer. For the application patterns, there's a lot to talk about starting from trusted time sources all the way over to standard patterns and implementations to grant availability, state recovery, etc. 

I'll be sharing more thoughts in the near future and would love to collaborate in discuss those with other community members. 