`22/02/2025`

This article is aimed at anyone that wants to understand how the vTPM<>TDX combined attestation works. Additionally, I talk a bit about using the ABI exposed by the Trusted Security Module (TSM) to get TDX quote data via configfs.

> This is a pretty technical article, some high level understanding of how TDX and TPM attestation works is recommended before reading. 

<hr/>

# Some Background on Practical TDX Attestation

TDX attestation is theoretically pretty simple at a high level: the TD generates a report (providing a report data `[u8; 64]` to embed custom data) that includes platform information, initial state of the vm firmware and vCPU, and 4 optionally updateable runtime measurement registers. The report must be signed by a quoting enclave on the same platform that verifies the report's integrity allowing it to be verified remotely.

The practical side tho has some intricancies mainly around the communication channels since we need the trusted domain and the sgx quoting enclave to communicate with each other. 

By spec, the SGX quoting enclave can only be run outside of the TDX guest (be it the host or a normal VM). The TDX guest generally can use communication channels like TCP/IP or vsock to send the report to the quoting enclave. Sometimes though, TDs aren't launched with those channels open (mainly due to security) so the guest kernels used to introduce a `GetQuote` IOCTL call.

From the Guest<>Hypervisor Comms interface specification:

> GetQuote TDG.VP.VMCALL is a doorbell-like interface used to help send a message to the
host VMM to queue operations that tend to be long-running operations. GetQuote is
designed to invoke a request to generate a TD-Quote signing by a service hosting TD-Quoting
Enclave operating in the host environment for a TD Report passed as a parameter by the TD.
TDREPORT_STRUCT is a memory operand intended to be sent via the GetQuote
TDG.VP.VMCALL to indicate the asynchronous service requested.
For the GetQuote operation, the goal is for the TDREPORT_STRUCT to be received by the TD
via a prior TDCALL[TDG.MR.REPORT] in a buffer and placed in a shared-GPA space passed to
the VMM as an operand in the GetQuote TDG.VP.VMCALL. In the case of this operation, the
VMM can access the TDREPORT_STRUCT, queue the operation for a service hosting TD-
Quoting enclave, and, when completed, return the Quote via the same, shared-memory area.

So the quoting process was quite straightforward. However most recent kernels don't expose the `GetQuote` ioctl call anymore, rather they are defaulting to TSM reports.

Of course, it's always a good practice to see what your kernel supports and how the hypervisor is managing the virtual machines to get to the best approach. Speaking only from a practical standpoint, if you are building applications that need to run on a trusted execution environment optimizing quoting isn't a concern since it's an operation that is carried very few times, sometimes even once per device depending on your application's policy.

## TSM reports via Configfs

TSM exposes an ABI through configfs for the guest to communicate and get a quote starting from an input array of 64 bytes. The process itself is straightforward, here's a brief informal description:

1. create a temp directory for the report entry into the kernel's tsm path (TSM prefix/report/tmpentry). 
2. within the temp entry write the inblob i.e the report data.
3. read the outblob with the quote.

I've created a [native rust client](https://github.com/tpluslabs/rs-configfs-tsm-quoting) to deal with the above (and the other in-betweens + checks) which is quite handy to use:

```rust
use client::{make_client, report::{self, Request}};

// You can use teenonce=$(head -c 64 /dev/urandom | xxd -p | tr -d '\n') to generate a
// random hex report.

fn main() {
    let report_data: String = std::env::args().nth(1).expect("Please provide report data");
    let bytes = hex::decode(&report_data).expect("Report data must be hex format");
    
    let client = make_client().unwrap();
    let request = Request {
        in_blob: bytes,
        privilege: None,
        get_aux_blob: false
    };

    let mut report = report::create(client, request).unwrap();
    let result = report.get().unwrap().out_blob;

    println!("{}", hex::encode(result))
}
```

<br/>

For the curious, intel's [`tdx_attest.c`](https://github.com/intel/SGXDataCenterAttestationPrimitives/blob/main/QuoteGeneration/quote_wrapper/tdx_attest/tdx_attest.c#L802) gives a good overview of the tdx quoting mechanisms:

```c
tdx_attest_error_t tdx_att_get_quote(
    const tdx_report_data_t *p_tdx_report_data,
    const tdx_uuid_t *p_att_key_id_list,
    uint32_t list_size,
    tdx_uuid_t *p_att_key_id,
    uint8_t **pp_quote,
    uint32_t *p_quote_size,
    uint32_t flags)
{
    tdx_attest_error_t ret = TDX_ATTEST_ERROR_UNEXPECTED;

    if ((!p_att_key_id_list && list_size) ||
        (p_att_key_id_list && !list_size)) {
        return TDX_ATTEST_ERROR_INVALID_PARAMETER;
    }
    if (!pp_quote) {
        return TDX_ATTEST_ERROR_INVALID_PARAMETER;
    }
    if (flags) {
        //TODO: I think we need to have a runtime version to make this flag usable.
        return TDX_ATTEST_ERROR_INVALID_PARAMETER;
    }

    // Currently only intel TDQE are supported
    if (1 < list_size) {
        return TDX_ATTEST_ERROR_INVALID_PARAMETER;
    }
    if (p_att_key_id_list && memcmp(p_att_key_id_list, &g_intel_tdqe_uuid,
                    sizeof(g_intel_tdqe_uuid))) {
        return TDX_ATTEST_ERROR_UNSUPPORTED_ATT_KEY_ID;
    }

    *pp_quote = NULL;

    struct tdx_quote_hdr *p_get_quote_blob = malloc(REQ_BUF_SIZE);
    if (!p_get_quote_blob) {
        return TDX_ATTEST_ERROR_OUT_OF_MEMORY;
    }

    uint32_t payload_body_size = 0;

    tdx_report_t tdx_report;
    memset(&tdx_report, 0, sizeof(tdx_report));

    int devfd = open(TDX_ATTEST_DEV_PATH, O_RDWR | O_SYNC);
    if (-1 == devfd) {
        TDX_TRACE;
        free(p_get_quote_blob);
        return TDX_ATTEST_ERROR_DEVICE_FAILURE;
    }

    ret = get_tdx_report(devfd, p_tdx_report_data, &tdx_report);
    if (TDX_ATTEST_SUCCESS != ret) {
        goto ret_point;
    }

    ret = generate_get_quote_blob(&tdx_report, p_get_quote_blob);
    if (TDX_ATTEST_SUCCESS != ret) {
        goto ret_point;
    }

    ret = vsock_get_quote_payload((uint8_t*)p_get_quote_blob->data, p_get_quote_blob->in_len, &payload_body_size);
    if (TDX_ATTEST_SUCCESS == ret) {
        ret = extract_quote_from_blob_payload((uint8_t*)p_get_quote_blob->data, payload_body_size, pp_quote, p_quote_size);
    }
    if (TDX_ATTEST_SUCCESS == ret || TDX_ATTEST_ERROR_NOT_SUPPORTED != ret) {
        goto ret_point;
    }

#ifdef DEBUG
    fprintf(stdout, "\ngoto configfs logic\n");
#endif

    ret = configfs_get_quote(p_tdx_report_data, pp_quote, p_quote_size);
    if (TDX_ATTEST_SUCCESS == ret || TDX_ATTEST_ERROR_NOT_SUPPORTED != ret) {
        goto ret_point;
    }

#ifdef DEBUG
    fprintf(stdout, "\ngoto legacy logic\n");
#endif

    ret = tdcall_get_quote_payload(devfd, p_get_quote_blob, &payload_body_size);
    if (TDX_ATTEST_SUCCESS == ret) {
        ret = extract_quote_from_blob_payload((uint8_t*)p_get_quote_blob->data, payload_body_size, pp_quote, p_quote_size);
    }

ret_point:
    if (TDX_ATTEST_SUCCESS == ret && p_att_key_id) {
        *p_att_key_id = g_intel_tdqe_uuid;
    }
    close(devfd);
    free(p_get_quote_blob);

    return ret;
}
```

This basically tries to find the supported approach out of the three I described. NB: The tricky part when writing code that performs this logic is that your kernel might have a different support (e.g support or not the GET_QUOTE ioctl call) or your guest be launched without the comms service (e.g without the vsock channel open), so blackblox testing your own code becomes tricky. Luckily, there are other reference implementations that can be used to check out what works on your machine and what doesn't. 

I you, like me, have gotten involved with TEEs from the web3 world you'll notice that these are all challenges that come into play when talking about reproducibility of the measurements which are very important when working with blockchains since while they are deterministic, there is some state inferred by the guest image, vmm, etc that can change the measurements so fully open and well documented guest stack is a must.

<br/>
<hr/>
<br/>

That said about TDX quotes, why does requesting a quote on a gcloud tdx capable instance look like the follwoing?

```
ak_pub: ".."
quotes: {
  quote: "\xffTCG\x80..."
  raw_sig: "\x00\x14..."
  pcrs: {
    ...
  }
}
// other quotes ...
event_log: "\x00\x00\x00\x00\x03\x00\x00\x00..."
tdx_attestation: {
  header: {
    ...
  }
  td_quote_body: {
    ..,
  }
  signed_data_size: 4299
  signed_data: {
    ...
  }
  extra_bytes: "\x00\x00\x00\x00..."
}
```

# Combined Attestations

If you've worked with TPM before, you'll recognize part of the attestation. This is in fact a TPM (well vTPM) attestation report that also optionally includes a TDX quote. 

### Why TPM attestations?

The main reason is unification of confidential attestation, more specifically unifiyng the way we can attest measurements of the guest stack across platforms, keeping still in mind that hardware measurements themselves are platform specific.

There's a good talk that covers this topic from the KVM forum 2022.

### vTPM??

At a high level, TPMs are tmper resistant chips that provide hardware-based RoT to manage encryption keys. vTPMs are software emulations of the TPM chip's hardware capabilities (roughtly attestation and key generation). 

## How it Works

<img src="/images/vtpmtdx.png">

The high level idea behind TEE<>TPM combined attestations is that we emulate TPM capabilities within a non-tamperable environment (generally, vTPMs run on the hypervisor) that is managed by the same VMM (not necessarily hardware bound, depends on the policy). 

So we have the VMM that uses the TDX module to launch two trusted domains:

1. The guest TD.
2. The vTPM TD.

And when the guest TD wants to attest, they send out the report to the vTPM TD which builds it into a TPM report by extending the PCR register with the td runtime measurement register. In order for this to happen though, we need the two vms to trust eachother, so we need to establish a secure encrypted messaging channel.

### Mutual authentication through attestation

Upon starting, the guest TD and vTPM TD establish a secure message-passing channel enforced by mutual attestation. Basically, be `H(any)` the hash function each TD generates asymmetric encryption keys and generates a TD report using `H(pubkey)` as report data that is then signed by the QE. This allows the other peer to verify the quote and know that a certain secret key corresponding to the `pubkey` included in the quote is only kept within a trusted domain they trust according to the policy they use (if you're familiar with dstack, this pretty much works the same way). 

Once the communication channel is established, the message passing is quite simple and is fully based on asymmetric encryption. Assuming the guest TD $TD_g$ wants to use `TPM_INSTRUCTION`:

1. $TD_g$ encrypts `TPM_INSTRUCTION` and stores on a shared buffer.
2. $TD_g$ notifies the VMM they've sent the instruction.
3. VMM copies the command over to the buffer it shares with the vTPM and notifies the vTPM that it has received an instruciton.
4. vTPM TD decrypts the instruction and processes it, returning the result to $TD_g$ in the same way.

### TPM vs vTPM Attestation (on TD)

Note that while TPM attestation includes endorsement keys for identity and prove hardware authenticity, vTPMs run on TD are endorsed by the associated TD quote instead of the identity certificate.

This means that the attestation process is split in two:

1. vTPM endorsement key attestation.
2. guest td attestation.

#### vTPM endorsement key attestation

Since the endorsement key is not hardware rooted, rather it's software rooted we need to make sure that the keys have been generated and are only stored within the vTPM trusted domain to ensure integrity. So the vTPM TD generates a report with report data `H(KP_p)` where `KP_p` is the public key of the generated keypair. This report is then sent to the QE and attests that `KP_s` is safely managed.

#### Guest TD attestation

This is standard TPM attestation by extending the TD measurements into the TPM's PCR and then the vTPM will generate the TPM quote that can be verified in standard TPM quote verification.
