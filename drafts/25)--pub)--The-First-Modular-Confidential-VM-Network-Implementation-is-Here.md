`11/12/2024`

I've recently been introduced to the world of trusted execution environments (TEEs), mainly around the topic of confidential virtual machines (CVM). Dstack is pretty much the only design for clusters of CVMs, and since it lacked a rust implementation that wasn't
enshrined and was as minimal as its [orginal python counterpart](https://github.com/amiller/dstack-vm/), I've decided to create one myself.

<img src="/images/cvmbootstrap.png">

# rs-modular-dstack

[rs-modular-dstack](https://github.com/heytdep/rs-modular-dstack) is not a standard dstack implementation though, it's built from the ground up to be as modular as possible enabling implementors to build fully customizable dstack implementations following the building blocks provided by a core sdk. The reason for having such codebase design is simple: dstack is not a network standard that all nodes need to follow, it's simply a standardized way of sharing secret state between confidential virtual machines, but many parts of the cluster implementation have little to do with the dstack design. Actually, most of the implementation has little to do with the dstack standard and should be up to the individual implementor to adapt the cluster to the app's requirements. 

For example, dstack relies on various paths both on the host machine and on the guest machine to request secret-derived tagged hashes or trigger actions, and `dstack-core` provides highly customizable generics to be used to fill a hardcoded path structure. For instance for the guest these are some of the generics:

```rust
#[async_trait]
pub trait GuestServiceInner: TdxOnlyGuestServiceInner {
    type Pubkey: Send + Sync + DeserializeOwned + Serialize;
    type EncryptedMessage: Send + Sync + Serialize;
    type Quote: Send + Sync + DeserializeOwned;
    type SharedKey;

    fn get_secret(&self) -> anyhow::Result<Self::SharedKey>;

    async fn replicate_thread(&self) -> anyhow::Result<Self::SharedKey>;

    async fn onboard_new_node(
        &self,
        quote: Self::Quote,
        pubkeys: Vec<Self::Pubkey>,
    ) -> anyhow::Result<Self::EncryptedMessage>;
}
```

and this is how they are exported by `dstack-core`:

```rust
pub struct GuestPaths<H: GuestServiceInner> {
    pub inner_guest: Arc<H>,
}

impl<H: GuestServiceInner + Send + Sync> GuestPaths<H> {
    pub fn onboard_new_node(
        &self,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("onboard")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_impl(self.inner_guest.clone()))
            .and_then(
                |request: requests::OnboardArgs<H>, guest_impl: Arc<H>| async move {
                    // do stuff with `guest_impl`
                },
            )
    }

    // Should only be callable within trusted enclaves.
    pub fn get_derived_key(
        &self,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("getkey")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_impl(self.inner_guest.clone()))
            .and_then(
                |request: requests::GetKeyArgs<H>, guest_impl: Arc<H>| async move {
                    // do stuff with `guest_impl`
                },
            )
    }
}
```

This allows the implementor (new-york in our case) to just implement `GuestServiceInner` (and/or use defaults) with the provided bounds and structures without needing to fully understand the big picture.

The codebase design is not the topic of this post though, you can read more about the rationale of modular-dstack in [my original post on flashbots](https://collective.flashbots.net/t/modularizing-dstack-sdks-and-default-patterns-for-creating-p2p-cvm-clusters/4194). 

## Hello world is on Stellar's testnet

The [first app](https://github.com/heytdep/rs-modular-dstack/tree/main/examples/ping-host) ever deployed with the modular dstack implementation is on the Stellar testnet network! There's two main reasons:

1. It's the network I'm the most proficient with.
2. I think that there's a lot of interesting use cases for applications that would be beneficial on Stellar too and having already the helpers to interact with the network from dstack will help future developments on the network I'm more involved with.

The really cool thing about CVMs is that you just need an sgx quoting enclave to participate, or better, in the case of `new-york` RA is completely handled by a dummy quote generator and verifier, so you don't need an actual SGX+TDX hardware to participate in running `ping-host` (the very first hello world app), just a linux machine with qemu.

In fact, if you look at what `new-york` is using to e.g verify a quote from a new node attempting to join the cluster:

```rust
#[async_trait]
impl GuestServiceInner for GuestServices {
    // ..

    async fn onboard_new_node(
        &self,
        quote: Self::Quote,
        pubkeys: Vec<Self::Pubkey>,
    ) -> anyhow::Result<Self::EncryptedMessage> {
        let verify = self.attestation.verify_quote(quote).await?;
    }
}
```

we're actually using the [dummy attestation object](https://github.com/heytdep/rs-modular-dstack/tree/main/crates/dummy-attestation) which as you might notice it's not calling the local sgx quoting enclave.

### Running the TDX emulator VM with Flashbox

We can use the [flashbox](https://github.com/flashbots/flashbox) image along with qemu to launch a TD (in our case it's not actually done with tdx hardware so we are not actually running in a trusted execution environment. This is just for ease of replication, running an actual tee just requires adding a few additional flags to qemu):

```
qemu-system-x86_64 -D /tmp/qemu-guest.log \
    -accel kvm -m 16G -smp 4 \
    -name qemu-vm,process=qemu-vm,debug-threads=on -cpu host -nographic -nodefaults \
    -device virtio-net-pci,netdev=nic0 -netdev user,id=nic0,hostfwd=tcp::10022-:22,hostfwd=tcp::24070-:24070,hostfwd=tcp::24071-:24071 \
    -drive file=../flashbox/flashbox.raw,if=none,id=virtio-disk0 -device virtio-blk-pci,drive=virtio-disk0 \
    -bios /usr/share/ovmf/OVMF.fd \
    -chardev stdio,id=char0,mux=on,signal=off -mon chardev=char0 -serial chardev:char0 \
    -pidfile /tmp/qemu-pid.pid -machine q35 &
```

> Note that `file=../flashbox/flashbox.raw` should point to the flashbox image.

> Note that you need to add your current user to the kvm group first. 

### ./newyork.sh

This is the command that will bootstrap or onboard your node to the provided cluster. It accepts 4 parameters: the cluster contract address, the stellar secret key for signing+submitting transactions, the host interface address (can find it with `ifconfig` and port 8000) and lastly the public key if you're trying to join a cluster that was not yet initialized. 

```
./newyork.sh CLUSTER SECRET 192.168.x.x:8000
```

Under the hood, it deploys the pod newyork pod containing both the guest services and the `ping-host` app to the TD VM while it runs the host environment on the host machine:

<img src="/images/newyorksh.png">

On the qemu vm side, you should be able to check the logs of the newyork container running

```
podman logs new-york-pod-new-york-container
```

which should let you access the internal logs verifying that you've indeed generated the shared secret within the TD and forwared the bootstrap request to the host service (picture above):

<img src="/images/obtainedsecret.jpeg">

# What's Next

This is just the beginning of my work on TEEs, specifically I'm planning on continuing to maintain and improve `rs-modular-dstack` to make it actually safe and maybe work on some lower level primitives for interacting with the quoting enclave. 

Further down the list is built-in support for MPC, partial consensus mechanisms, continuous randomized attestations, and other improvements for extra security.
