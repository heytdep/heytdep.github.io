
`17/08/2025`

An interesting aspect of the new dstack research directions shared in San Francisco was the evolution of how the virtualization layout of trusted domains looks.
If we are already virtualizing for compatibility and auditability of measurements, why not also support trusted domains with multiple applications running at the same time—and be able to dynamically add and remove those applications?


# The Attestation Challenge

If you’re familiar with TDX attestations, then the idea of running multiple unrelated applications within the same trusted domain probably raised some questions.

The main challenge: runtime measurements are extend-only, but we want the ability to add and remove applications dynamically. On top of that, we want per-application measurement isolation for reproducibility across VMs.

This essentially leaves using report data as the only logical approach.

# The Kernel "Problem" And How It Relates To This

If you've been followig what we've been working on at tplus in the TEE world, you'll know that we've done some work with deploying on GCPs confidential VM service. Deploying the vms there made us realize that [the upstream kernel didn't merge yet](https://github.com/google/go-tdx-guest/issues/54#issuecomment-2487221226) (at least at the time of trying it out, about a few months ago), so we went with the approach of trying out report data based attestations vs extending runtime measurements.

Using report data instead of RTMR3 means you are limited (or not, depending on perspective) to your OS’s sandboxing safety around quote generation, since report data is arbitrary—and thus revertible and tamperable. So I took a stab at writing [the implementation](https://github.com/tpluslabs/meta-dstack/tree/main/server/src) for this behavior to have things work out of the box.

In this small post I want to outline how this works in hope of it being helpful for others and have this approach reviewed as well.

### Some Considerations

Keep in mind that the implementation here is not supposed to keep within the same VM multiple applications that don't have to do with each other rather applications that must/can co-exist. Thus, the goal is to bind the applications together into a single attestation; this is for safety because applications are meant to interact with each other and share the same network interface.

Also, note that the current implementation embeds the expected allowed pods within the operating system if a certain compile-time flag is enabled. This is for safety and is not necessarily how we expect to roll tplus out considering it would make OS measurement strictly dependant on the pods. For debug purposes, there's a separate compile-time flag that allows a one-time request to set the allowed pods upon VM initialization. Note that in my opinion this approach can easily be adapted for a gated production deployment.

### How Does it Work?

It’s actually simple: pods should not have privileges to access report generation through TSM directly, so they must proxy through the OS.
The dstack server exposes a get-quote endpoint that pods can call to generate an attestation.
The server then combines:

1. A hash of all currently deployed pods, and
2. The application-provided report data.

The concatenation is used as the final report data.

```rust=
async fn get_quote(&self, report_data: String) -> Result<String, warp::Rejection> {
    let report_data = [
        self.aggregate_digest(),
        hex::decode(report_data).map_err(|_| Wrapper("invalid hex".into()))?,
    ]
    .concat();

    let quote = quote::get_quote(&report_data).await;

    match quote {
        Ok(quote) => {
            tracing::info!("successfully obtained quote");
            Ok(quote)
        }

        Err(e) => {
            tracing::error!("failed to obtain quote: {:?}", e);
            Ok("failed to get quote".into())
        }
    }
}
```

The set of deployed pods is cached at deployment (or deletion) time, so the server always knows what to include.

## Now what?

As you can see, this isn't directly related to what Phala has been thinking for their own sandboxing solution but it follows the same assumptions around using report data and sandboxes to make things safe.

Some considerations on allowing unrelated multiple applications to work together:

1. Sandbox safety. This adds another layer of code that must be carefully audited to make sure that first of all the pods can't have access to the tsm interface, but mainly that the OS's server is safe against attack that might try to tamper with the report data forwarded to the quoting enclave.
2. Networking. Applications should be completely isolated unless directly specified by both. Thankfully, products like podman make this simpler allowing each pod to define their networking interface, but I'm still curious to see what the final implementation will look like on this end.

## Improving The Report-Data Approach

Using report data for attestations can be both dangerous and powerful for cloud services that want to scale and wisely allocate resources. Below are some ideas that flow through my mind while writing this post.

One of the more interesting aspects of this design is that it doesn’t have to stop at binding all pods into a single attestation. Because the server controls how the `aggregate_digest` is computed, we can extend the model to support finer-grained scopes. 

1. Application-only attestations: If authentication for the deployed pods is introduced upon quote generation, instead of hashing all deployed pods, the server can generate the quote only for that application's pod. The server must however ensure that all the rules that are set by pod are enforced (e.g pod isolation). 

2. Subset attestations on demand: Similarly to the above scenario, a pod might request a quote for itself and only the services it explicitly depends on. The OS can compute a digest over that subset and fold it into the report data. The server would have to make sure that the dependant services are deployed and that the sandbox rules are enforced on those as well. 

In other words, the current approach is just the baseline. Because the server defines how pod state is digested, it’s straightforward to turn this into a more expressive framework that can support different security policies without changing the trust assumptions around the operating system. Kind of implicit, but it's also worth it to mention that the digest generation should be deterministic and not dependant on stuff like deploying ordering etc (not implemented in this case), to enforce this we can use for example lexicographic ordering.
