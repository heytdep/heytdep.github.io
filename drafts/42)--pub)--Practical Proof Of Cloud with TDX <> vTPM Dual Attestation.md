`19/01/2026`

## Problem

TDX attestation proves that your code is running inside a hardware-isolated Trust Domain with specific measurements. Intel's Quoting Enclave signs the quote, and you can verify the signature against Intel's root of trust. This gives you strong guarantees about integrity and isolation.

But TDX attestation doesn't tell you where the TD is running or who is hosting it. When physical safety is important for the application's safety (which in an ideal architecture falls back solely on liveness) then knowing who is hosting the hardware and making sure we partially trust them is important.

The TDX quote is signed by Intel which doesn't know or care whether your TD is on GCP, Azure, or an attacker's machine; if it's valid TDX hardware, it gets a valid quote. 

## vTPM

GCP provisions a virtual TPM alongside each CVM. The vTPM has an Attestation Key (AK) that can sign quotes, and critically, the AK comes with a certificate chain that roots back to the cloud provider's CA.

If you can prove that a specific vTPM is attached to the TD, and that vTPM's AK certificate chains to GCP's root CA, then you've proven the TD is running on GCP infrastructure.

The challenge is proving that binding so that no party besides the one holding the AK secret can tamper with this logic.

## Event Logs Cross-Checks

The CCEL and TCG event logs are essential for proving the binding between the two processes since when the CVM boots, the same boot events get measured to both RTMRs (TD) and PCRs (vTPM).

Each measurement recorded during boot produces a digest that gets extended to both an RTMR and a PCR. The digest values are eventually identical but differ in the target register, some technlogy-specific params and the order of concatenation.

The CCEL records what was extended to the RTMRs. The TCG log records what was extended to the PCRs. By replaying both logs and comparing the digests, you can verify that the vTPM observed the exact same boot sequence as the TD.

This is the fundamental binding. If GCP tried to proxy a different vTPM to your TD, or more realistically, an attacker tries to proxy a GCP vtpm on their physically controlled TD, that vTPM would have observed a different boot sequence. Its TCG event log would contain different digests and the cross-check would fail.

## Verification Approaches

There are two ways to structure the verification, differing in where the cross-check happens and what the verifier has to trust. Because verification is often carried on trustless processes which require more efficiency, the second approach is what most applications will take. But it's worth it to mention the first approach so that it's easier to understand the concept.

### Approach 1

The verifier performs the cross-check themselves. The TD provides all the raw materials:

- TDX quote
- vTPM quote
- CCEL event log
- TCG event log
- AK public key and certificate chain

The verifier then:

1. Verifies the TDX quote.
2. Replays the CCEL and checks that computed RTMRs match the TDX quote.
3. Verifies the vTPM quote signature using the AK.
4. Replays the TCG log and checks that computed PCRs match the vTPM quote.
5. Cross-checks that CCEL and TCG contain events with identical digests.
6. Verifies the AK certificate chains to the expected cloud provider.

This approach requires no trust in the TD's code for what regards the TD<>vTPM binding. The verifier does all the work and trusts only their own verification logic.

> NB: the verifier generally needs to measure/trust the TD's code anyways.

### Approach 2

The TD performs the cross-check internally, and the verifier trusts the TD's measured code to have done it correctly.

The TD's logic (which is captured in the measurments):

1. Replays the CCEL.
2. Replays TCG event log.
3. Gets a vTPM quote and verifies PCRs match the replayed TCG values.
4. Cross-checks that CCEL and TCG digests match.
5. If everything passes, puts the AK public key hash in report_data and generates a TDX quote $Q_{AK}$.

The TD then provides only:

- $Q_{pubkey(AT)}$.
- AK pubkey + certificate chain until GCP root of trust.

The verifier:

1. Verifies the TDX quote.
2. Checks that report_data contains SHA384(pubkey(AK))
3. Verifies the AK certificate chains to the expected cloud provider

The verifier doesn't need the event logs at all. They're trusting that the code producing those specific RTMR values performed the cross-check correctly which is a reasonable trust assumption if the TD code is open source and the measurements are reproducible.

#### Understanding What Each Piece Proves

The CCEL/TCG cross-check proves that this specific vTPM observed the same boot sequence as this TD. This is the actual binding between the two attestation domains; there's no other way to prove the vTPM is attached to your TD. The AK in report_data proves that the TD's measured code validated that binding before committing to this AK. It moves the verification burden from the verifier to the TD and lets the verifier run a simpler protocol. The AK certificate chain proves that this vTPM belongs to a specific cloud provider's infrastructure. 

### Bonus approach

Another possible approach is embedding the root of trust within the TDs context, then the measurements capture it and the root of trust verification of the AK is all encompassed within the TD. This however means having to embed the RoT within the application's inner measurements values which may not be the best option for some use cases.

## Some Implementation Details

In TD, must collect:

```
# CCEL
/sys/firmware/acpi/tables/data/CCEL

# TCG event log
/sys/kernel/security/tpm0/binary_bios_measurements

# AK certificate
tpm2_nvread 0x1c10002 -o ak_cert.der
```

Both logs use the TCG crypto-agile format. For each event:

1. Read the target register index (CC_MR_INDEX for CCEL, PCR index for TCG)
2. Read the event type
3. Read the digest (SHA384 for TDX)
4. If event type is not EV_NO_ACTION, extend the register

Extend operation is: `register_new = SHA384(register_old || digest)`

> NB: For CCEL, the CC_MR_INDEX uses an offset since it includes the mrtd which is immutable:
> 
> - CC_MR_INDEX 1 = RTMR[0]
> - CC_MR_INDEX 2 = RTMR[1]
> - CC_MR_INDEX 3 = RTMR[2]
> - CC_MR_INDEX 4 = RTMR[3]

Once all events are replayed, the computed register values should match the values in the respective quotes.

Extract all (event_type, digest) pairs from both logs, excluding zero digests and EV_NO_ACTION events and find the intersection, i.e boot events measured to both the RTMRs and PCRs.

If the intersection is empty, the logs don't share common measurements and the binding cannot be established. If there are common events with identical digests, those events bind the two attestation domains together. Once this passes, we can construct TDX quote with the AK pubkey as report data, i.e now that quote now commits to this specific AK, and the RTMR values commit to the code that validated the binding.

For the TD-internal verification approach, which applications that require a simpler protocol on the verifier will most likely favor, the verifier must have context over the TDX quote, the AK root of trust and the expected measurements for the TD.
