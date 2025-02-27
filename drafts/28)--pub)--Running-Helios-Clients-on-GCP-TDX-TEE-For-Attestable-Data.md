`27/02/2025`

Assuming the reader already knows what [Helios](https://github.com/a16z/helios) is, I explore how to run a Helios light client within a Trusted Execution Environment to attest over network data for others to consume.

> "Wait, doesn't this defeat the concept of a light client?"

You're spot on, it kinda does. Light clients are supposed to be so lightweight that the client itself can run it to attest for the data. However, when working with TEEs it might be common to relay this data or even run the light client in a trusted place for the rest of the guest to read data from. TLDR; we want to have a light client running withing the tdx guest stack.

There's a couple of challenges that got in the way:

1. I had to patch, add a new layer, and build the guest os image with yocto.
2. I had to write an attestation client to interact with tsm ABI through configfs. I actually wrote about it [here](https://heytdep.github.io/post/27/post.html).

The two above can be quite challenging especially if you aren't on bare metal tdx capable hardware (i.e don't control the hypervisor) and if you're not very familiar with your cloud provider. For instance, I initially couldn't have my os boot because I hadn't seen I needed [gcp specific](https://cloud.google.com/confidential-computing/confidential-vm/docs/create-custom-confidential-vm-images#intel-tdx) kernel features. There is also almost no (if not zero) material on performing this on gcp, which I guess is also one of the reasons why it turned out to be quite fun.

I have created a repository that contains a layer, patches and init script to patch and build upon flashbots' [yocto-manifests](https://github.com/flashbots/yocto-manifests/blob/main/config_files/tdx-base/setup) repo config. You can find the repository at [tpluslabs/meta-dstack](https://github.com/tpluslabs/meta-dstack).

Without diving too much in the details, this is what we need to get to a working image that boots:

1. add the gcp specific kernel features linked above. Once I learned about those features, a quick search on flashbots/meta-confidential-compute for those features revealed that there's a branch that is a WIP to also enable gcp (v3).
2. patch the config to set target matchine to tdx-gcp. This is thanks to the good organization that the v3 branch places among different kernel features.
3. patch the cvm distro to depend also on our `dstack-sync` recipe.
4. add a layer that places our guest backend to be run after boot.

> I'm not expanding much here, but if you're building something similar and need help or just an input feel free to [reach out](https://x.com/heytdep). The patch itself is quite straightforward though.

# GCP deployment

Once the image is built (follow the instructions in the repo to reproduce the image), we can publish it to gcp and create a tdx capable instance off of it.

You can verify that the deployment by inspecting the serial port output which should look something like:

```
[    3.545903]  __x64_sys_read+0x14/0x1a
[    3.549723]  x64_sys_call+0x142a/0x200c
[    3.553716]  do_syscall_64+0x7c/0xde
[    3.557447]  entry_SYSCALL_64_after_hwframe+0x76/0x7e
[    3.562656] RIP: 0033:0x7f5b5c11f755
[    3.566380] Code: 48 83 c8 ff 44 89 cf 48 89 04 24 e8 f8 94 fa ff 48 8b 04 24 48 83 c4 38 c3 f3 0f 1e fa 80 3d f1 98 0c 00 00 74 1d 31 c0 0f 05 <48> 3d 00 f0 ff ff 76 6c 48 8b 15 9c 16 0c 00 f7 d8 64 89 02 48 83
[    3.585296] RSP: 002b:00007fff62d0a908 EFLAGS: 00000246 ORIG_RAX: 0000000000000000
[    3.593013] RAX: ffffffffffffffda RBX: 0000000000040000 RCX: 00007f5b5c11f755
[    3.600297] RDX: 0000000000040000 RSI: 00007f5b5bb34038 RDI: 0000000000000008
[    3.607588] RBP: 000055653bb4eba0 R08: 00007f5b5c1a8654 R09: 0000000000000000
[    3.614879] R10: 0000000000041000 R11: 0000000000000246 R12: 0000000000000000
[    3.622169] R13: 00007f5b5bb34028 R14: 000055653bb4ebf8 R15: 00007f5b5bb34010
[    3.629455]  </TASK>
[    3.631795] ---[ end trace 0000000000000000 ]---
hwclock: can't open '/dev/misc/rtc': No such file or directory
hwclock: can't open '/dev/misc/rtc': No such file or directory
INIT: Entering runlevel: 5
Configuring network interfaces... [  123.537312] gve 0000:00:03.0 eth0: Device link is up.
dhcpcd-10.0.6 starting
sandbox unavailable: seccomp
sandbox unavailable: seccomp
eth0: waiting for carrier
eth0: carrier acquired
DUID 00:01:00:01:2f:53:76:f7:42:01:0a:8a:00:26
eth0: IAID 0a:8a:00:26
eth0: adding address fe80::ecb8:a32b:a1c0:3c7
eth0: soliciting an IPv6 router
eth0: soliciting a DHCP lease
eth0: offered 10.138.0.38 from 10.138.0.1
eth0: probing address 10.138.0.38/32
eth0: leased 10.138.0.38 for 86400 seconds
eth0: adding host route to 10.138.0.1
eth0: adding default route via 10.138.0.1
ip: SIOCGIFFLAGS: No such device
Starting Dropbear SSH server: Generating 2048 bit rsa key, this may take a while...
Public key portion is:
ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQDE35jR8XgXHa6A2ZzKHU7p46KbZDRo2PTdsxvJpuKqzyw61222IkaCVDIiVhplQjVr1zIrx07yvKiFwZng+5+VdLSRKIafblp88bu89zt+0+K8rOPi4xUyy9ZJ9oC5U41POpQ4T4r+XkSzVIk/dqTsCh0ua94jT1Jdww7MUtaubGDKl1RE7a/6nzY2NlYlQuIm7oBPiIJoMN7e2WmJSAgE7mhS7tHCHUMT4JnkKbU1XHpZbp/dOorkv+XHrpueY7BaRodOmNc7ofHIBM2oySPdwYpXAJnCScUpnoVQVspn+XJbI8Q5sfw25zlcKMhYm/nX2zJ9sBtrACAPxAr3J0k3 root@tdx-gcp
Fingerprint: SHA256:J3I22M97gkOK5JHMUpux3L0lXFOeHohWexmw1jLkJaQ
dropbear.
Starting syslogd/klogd: done
date: unrecognized option '--iso-8601=seconds'
BusyBox v1.36.1 () multi-call binary.

Usage: date [OPTIONS] [+FMT] [[-s] TIME]
: Starting date-sync
starting surpluser
getting quote
getting associated key
got quote
got associated key
node pubkey 0297303e67d879d75c38d3cd2c0a28f20cce1d15fa154a092e6d81af98a3edc267
Using untrusted RPC URL https://eth-mainnet.g.alchemy.com/v2/demo
Using consensus RPC URL: https://www.lightclientdata.org
Requested bootstrap
Ok(())
Built client on network "mainnet" with external checkpoint fallbacks
client synced
```

> NB it will take a couple of minutes to boot fully, give it time.

# rs-modular-dstack and Helios

For those who don't know, a while ago I had started writing an sdk to write dstack implemenations which I wrote about [here](https://collective.flashbots.net/t/modularizing-dstack-sdks-and-default-patterns-for-creating-p2p-cvm-clusters/4194). We're leveraging this library to build off a very minimal guest backend for our light clients dstack implementation.

To be fair, calling it dstack is not really correct since the nodes don't need to be able to decrypt shared state to reproduce state and functionality, hence why the stripped-down new york implementation is really minimal. We just need the guest to be posting their quote to a host. The host can do wathever they want with the quote (e.g posting it on-chain) so that others can verify that the light client node ran within the TEE can be trusted.

The process as to how we can trust light client data is fairly straightforward and is similar to how vTMP <> TD mutual attestation works: the guest generates a keypair in-td and includes the pubkey in the quote report. Anyone can verify the report and know that any message signed by that pubkey was signed within the trusted domain. 

As such, see our surpluser implementation:

```rust
async fn get_associated_key() -> Result<[u8; 32]> {
    let resp: Value = reqwest::get("http://localhost:3031/getnodekey").await?.json().await?;
    let decoded = hex::decode(&resp.as_str().unwrap())?;
    Ok(decoded.try_into().unwrap())
}

pub async fn run() -> Result<()> {
    println!("starting surpluser");
    let secp = Secp256k1::new();
    println!("getting associated key");
    let node_secret_key = get_associated_key().await?;
    println!("got associated key");
    let secret_key = secp256k1::SecretKey::from_byte_array(&node_secret_key)?;
    println!("node pubkey {}", secret_key.public_key(&secp));

    let untrusted_rpc_url = "https://eth-mainnet.g.alchemy.com/v2/demo";
    println!("Using untrusted RPC URL {}", untrusted_rpc_url);

    let consensus_rpc = "https://www.lightclientdata.org";
    println!("Using consensus RPC URL: {}", consensus_rpc);

    let mut client: EthereumClient<FileDB> = EthereumClientBuilder::new()
        .network(Network::Mainnet)
        .consensus_rpc(consensus_rpc)
        .execution_rpc(untrusted_rpc_url)
        // we should turn this off in prod and find a good way to retrieve a trusted checkpoint
        .load_external_fallback()
        .data_dir(PathBuf::from("/tmp/helios"))
        .build()
        .map_err(|e| anyhow!(e.to_string()))?;

    println!(
        "Built client on network \"{}\" with external checkpoint fallbacks",
        Network::Mainnet
    );

    client.start().await.map_err(|e| anyhow!(e.to_string()))?;
    client.wait_synced().await;
    println!("client synced");

    let client = Arc::new(client);
    let get_trusted_block = warp::path("block").and_then({
        let client = client.clone();
        let node_secret_key = node_secret_key;
        move || {
            let client = client.clone();
            async move {
                let block = client.get_block_number().await.unwrap().to_string();
                let signature = sign_message(node_secret_key, &block);
                Ok::<_, warp::Rejection>(warp::reply::json(
                    &serde_json::json!({"signature": hex::encode(&signature), "blocknum": block})
                        .to_string(),
                ))
            }
        }
    });

    warp::serve(get_trusted_block)
        .run(([0, 0, 0, 0], 3032))
        .await;

    Ok(())
}
```

Here we're basically getting the secret key (of the pubkey in the quote we sent to the host) by making a request to the guest-only service that handles the keys and then signing over the data we get from helios with that secret. As I promised, this is really minimal! 

> Props to Helios! The sdk is really neat.

For reference, here's how I implemented the guest and the host using my modular-dstack sdk:

```rust
//! Flow is really simple here.
//! 
//! We don't need replication so when the guest spins up it generates a random associated key. 
//! When the replication thread starts it generates a quote using the associated key's pubkey as report
//! data and then forwards it to the host's bootstrap endpoint that can publish the quote however it wants.
//! 
//! The only tdx guest path we want to expose now is the associated key's one since the helios light client
//! will need it to sign over the messages. Anyone can verify the quote for the public key singning the message.
//! 
use async_trait::async_trait;
use dstack_core::{
    host_paths, GuestServiceInner, HostServiceInner, InnerAttestationHelper, TdxOnlyGuestServiceInner
};
use tdx_attestation::Attestation;
// TODO change types depending on the chain we're posting to.
pub struct HostServices;

// Since we're not replicating we don't need to register or onboard. but we can
// kinda bootstrap by just posting the quote somewhere for discovery.
#[async_trait]
impl HostServiceInner for HostServices {
    type Pubkey = ();
    type Quote = String;
    type Signature = ();

    async fn bootstrap(
        &self,
        quote: Self::Quote,
        _pubkeys: Vec<Self::Pubkey>,
    ) -> anyhow::Result<()> {
        // todo post quote somewhere.
        println!("Got quote");
        let decoded = hex::decode(quote)?;
        let parsed = dcap_qvl::quote::Quote::parse(&decoded)?;

        let td_report = parsed.report.as_td10().ok_or::<anyhow::Error>(anyhow::anyhow!("invalid quote type").into())?;
        println!("Static measurements:");
        println!("mrtd: {}", hex::encode(td_report.mr_td));
        
        println!("Runtime measurements:");
        println!("0: {}", hex::encode(td_report.rt_mr0));
        println!("1: {}", hex::encode(td_report.rt_mr1));
        println!("2: {}", hex::encode(td_report.rt_mr2));
        println!("3: {}", hex::encode(td_report.rt_mr3));
        Ok(())
    }

    async fn register(
        &self,
        _quote: Self::Quote,
        _pubkeys: Vec<Self::Pubkey>,
        _signatures: Vec<Self::Signature>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn onboard_thread(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct GuestServices {
    // Implementor's configs including helper objects.
    host_endpoint: String,
    // kettle's associated key.
    associated_key: [u8; 32],
    attestation: Attestation,
}

impl GuestServices {
    pub fn new() -> Self {
        //let host_address = std::env::var("HOST").unwrap_or("host.containers.internal:8000".into());
        let host_address = std::env::var("HOST").unwrap_or("127.0.0.1:8000".into());
        let associated_key = secp256k1::SecretKey::new(&mut secp256k1::rand::thread_rng()).secret_bytes();

        Self {
            host_endpoint: host_address,
            associated_key,
            attestation: Attestation::new(),
        }
    }
}

// We don't need any of the dstack replication functionalities.
#[async_trait]
impl GuestServiceInner for GuestServices {
    type Pubkey = ();
    type EncryptedMessage = ();
    type SharedKey = ();
    type Quote = String;

    // Note: the implementor decides for themselves how they want the secret to be stored in
    // [`self`]
    async fn get_secret(&self) -> anyhow::Result<Self::SharedKey> {
        Ok(())
    }

    async fn replicate_thread(&self) -> anyhow::Result<()> {
        let secp = secp256k1::Secp256k1::new();
        let associated = secp256k1::SecretKey::from_byte_array(&self.associated_key)?;
        
        println!("getting quote");
        //let quote = "realquotewillbehere".to_string();
        let quote = self.attestation.get_quote(associated.public_key(&secp).serialize().to_vec()).await?;
        println!("got quote");
        
        let client = reqwest::Client::new();

        client
            .post(format!("http://{}/bootstrap", self.host_endpoint))
            .json(&host_paths::requests::BootstrapArgs::<HostServices> {
                quote,
                pubkeys: vec![],
            })
            .send()
            .await?
            .text()
            .await?;
        println!("Requested bootstrap");
        
        Ok(())
    }

    /// Verifies the provided quote ensuring that [`pubkeys[0]`] is within the quote, if that
    /// succeeds (i.e secretkey is held only in tdx) then it encrypts the shared secret to
    /// [`pubkeys[0]`].
    async fn onboard_new_node(
        &self,
        _quote: Self::Quote,
        _pubkeys: Vec<Self::Pubkey>,
    ) -> anyhow::Result<Self::EncryptedMessage> {
        Ok(())
    }
}

/// NON host-facing paths here.
#[async_trait]
impl TdxOnlyGuestServiceInner for GuestServices {
    type Tag = ();
    type DerivedKey = ();
    type AssociatedKey = String;

    // there is no shared secret, so no derived key.
    async fn get_derived_key(&self, _tag: Self::Tag) -> anyhow::Result<Self::DerivedKey> {
        Ok(())
    }

    async fn get_associated_key(&self) -> anyhow::Result<Self::AssociatedKey> {
        Ok(hex::encode(&self.associated_key))
    }
}
```

The `GuestServices` and the helios `run` function are coupled together into a single program that is embedded in the guest stack and which boots at startup. 

Note that the `meta-dstack` patches allow to disable the tweaks when run with `PROD=true` so that you obtain a safe locked down guest stack.

# Result

We now have a functioning measured guest stack that we can query as an attestable block number source!

On the host:

<img src="/images/hostlc.png" />

Now we can request attested blocks, try it out yourself!

```
$ curl http://34.169.246.178:3032/block

"{\"signature\":\"c4191c1432c1d646354cf7d02c66e3f549c8fad947b2fcba3796f379624f21a224341a957c76b5f18377a9d89c7d62ca4f6610e616270fe6c721205be5686a91\",\"blocknum\":\"21939763\"}"
```

# Next steps

Next steps will be really awesome, trust me.

# Credits

Thanks to both Phala Network and Flashbots github org page, reading their source codes was how I achieved `meta-dstack`.
