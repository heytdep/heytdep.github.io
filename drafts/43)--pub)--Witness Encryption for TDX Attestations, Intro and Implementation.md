`08/02/2026`

> I'm not a cryptographer, just a curious person working with applied cryptography to solve distributed systems problems. Feel free to call out any incorrect/imprecise maths, constraints or even logic!

(rust reference implementation available at https://github.com/heytdep/rs-wkem-dcap-circom)

Classic asymmetric encryption schemes are widely used and incredibly effective for any sort of communication between two known parties, it is for instance the exact construction we use for P2P communication between two peers which must prove they are attested beforehand while making the connection; the attestation part can occur in multiple places in respect to the handshake but you get the point.

Such encryption schemes don't trivially cover situations where the peer is not known however as it always requires some degree of interactivity in its construction. This is fine on the surface, but we discover it is not optimal for a series of mechanisms.

Imagine for instance two peer wanting to establish an encrypted connection without wanting to intiate the interaction with an intermediary (due e.g to censorship). If they interactively both initiate a handshake with a known intermediary they can be easily pinned by a censoring party. But what if the process was not interactive, i.e intermediary gains knowledge of both parties in an indirect yet secure way and can then use trivial obfuscation (since they are the origin) to mask the peers across a subset of addresses.

Or more tangibly, imagine a system such as [tplus](https://tplus.cx) where state in encrypted memory must be persisted even in the occurrence that all TEEs in the system are killed (this imposes the assumption that no live TEE fallback can exist). How do I persist the encrypted state so that only a valid trusted domain can decrypt it if the trusted domain supposed to decrypt the payload doesn't exist yet, or better, hasn't initiated any interaction with the system?

This is a problem statement that I've been thinking about recently and this write-up shares my findings regarding relying on witness encryption schemes so that we can decrypt a confidential message iff we possess a valid TDX attestation (intuitively, we must also enforce a report data check so that we are guaranteed the decyrption only happens within the trusted domain's execution). The result is this write-up which goes over some of the mathematically-bound challenges, and an actual working rust library implementation that can be already used to test this idea out.

---

# Why We Can't Use "Standard" WE Constructions

In case you did not know, WE is often considered impractical since to encrypt to an arbitrary NP statement we must rely on multilinear maps of an unbounded degree which are impractical or highly experimental. 

### Small note on multilinear maps

Imagine the following NP statement:

> "I know $w_1, w_2, w_3$ such that $w_1 \cdot w_2 \cdot w_3 = 40$"

The verification circuit algo is:
1. $w_1 \cdot w_2 = x$
2. $x \cdot w_3 = x$
3. $x == 40?$

Say the witness is $w_1 = 2, w_2 = 4, w_3 = 5$. The decryptor encodes these at level 1:

$$[2]_1, \quad [4]_1, \quad [5]_1$$

To evaluate the circuit, we need a multilinear map of $k = 3$:

$$[2]_1 \cdot [4]_1 = [8]_2$$
$$[8]_2 \cdot [5]_1 = [40]_3$$

Now, at the top level, the encryptor embedded $[40]_3$ during encryption from the public statement. The decryptor arrived at $[40]_3$ via the witness. They match, i.e decryption succeeds. But again, this requires $k$ to be unbounded for any non trivial circuit, and efficient mutlilinear maps are yet to be found.

Note that in evaluating this circuit, you don't have constraints, just evaluating it step by step in the exponent.

### Bilinear maps

What works and is tangible today is bilinear maps, e.g EC pairings.

If you're slightly familiar with EC cryptography this should spark some interest around looking into ZK proofs that already encode verification into EC pairings, such as groth16 (interestingly, this simple construction is the one I've been the most familiar with as a non-cryptographer). 

**the main idea**: we create a circuit so that it verifies a tdx attestation (it's ecsda signature verifications, hashing, and eq checks assuming we pass collateral as public inputs!) and reuse the proof and it's verification equation to construct a WE encryption scheme so that a valid proof verification yields the used encryption key. In, fact the groth16 verification equation is:

$$e(A, B) = e([\alpha]_1, [\beta]_2) + e(L, [\gamma]_2) + e(C, [\delta]_2)$$


where $[]_n$ means the value is encoded in $g_n$.

Pairings are bilinear maps, so intuitively we can reuse the equation in our WE construction. This trivial approach doesn't work in practice while theoretically sound on the surface. If you try and do the math you eventually end up noticing that you, as the proving party, cannot deterministically come up with the encryption key as you are not able to derive ranomization factor of the proof (there can be multiple valid proof constructions, but we have need randomization guard!). 

More specifically, the groth16 proof $(A, B, C)$ is computed by the prover as:

$$A = \left[\alpha + \sum_{i=0}^{m} a_i u_i(\tau) + r\delta\right]_1$$

$$B = \left[\beta + \sum_{i=0}^{m} a_i v_i(\tau) + s\delta\right]_2$$

$$C = \left[\frac{\sum_{i=\ell+1}^{m} a_i(\beta u_i(\tau) + \alpha v_i(\tau) + w_i(\tau)) + h(\tau)t(\tau)}{\delta} + sA + rB - rs\delta\right]_1$$

where $r, s$ are chosen freely by the prover for zero-knowledge. 

Different $(r, s)$ yield different valid proofs for the same statement and witness.

Now suppose you try the standard WE approach: define the encryption key as some function of the proof, say $k = e(A, B)$. 

These terms eventually expand to things like $e([\alpha]_1, [s\delta]_2)$, $e([r\delta]_1, [\beta]_2)$, $e([r\delta]_1, [s\delta]_2)$, etc. These change
with every choice of $(r, s)$, so different provers get different $k$ as randomization defeats canonical views; prover with $(r, s)$ and prover with $(r', s')$ arrive at different $e(A, B)$ values which may both be valid but since we can't derive a canonical key wouldn't be able to reconstruct k.

> NB: if you remove the randomization  from the protocol entirely, you not only loose zero-knowledge but the WE scheme would depend on the decryptor knowing the exact witnesses, which in the situation of TDX attestations defats the whole purpose of using WE as there can be multiple valid witness values.

Luckily, I found [this amazing paper](https://eprint.iacr.org/2024/264) and a [beautifully written related construction](https://limaois.me/en/groth16-we/). Enter WKEM.

# WKEM

What if we can flip the zk snark so that we reuse the pairings and the values we use as provers to construct a hint $\sigma$ so that a TEE that could construct a valid proof, i.e knows the secret witnesses, can rely on combining $\sigma$ and the witnesses to derive the encryption secret?

Instead of using the groth16 construction fully, we still compile the circuit into R1CS constraints

> NB: here the constraints are all of degree 2. Compared to a multilinear map we're constructing an arithmetic circuit composed of constraints of the same degree instead of evaluating all the exponents once.

We then reuse the QAP pairing structure; instead of the "standard" approach of embedding the secret in relation to a proof and having the decryptor produce a proof to try and derive the secret (which doesn't work due to non canonicality as I explained earlier), we evaluate the QAP polynomials of the circuit at a secret point $x$ and embed public-input QAP sums into the secret thanks to pairings. 

The ciphertext, $\sigma$, gives out "g-encoded" evaluations $[r \cdot u_i(x)]_1$, $[r \cdot v_i(x)]_2$ for all variables in the circuit , plus combined witness terms divided by $\delta$.

> NB: proofs are still randomized, but the QAP evaluation at $x$ remains deterministic given the assignment. The decryptor who knows the witness can compute the full assignment, which determines the QAP sums uniquely. Intuitively, since a valid witness computes the full assignment which is able to determine the QAP sums provided in $\sigma$ these terms will cancel out and reduce to the encryption key (we'll see this fully in the next section).

So the idea is that the publisher computes a proof over a TEE attestation circuit so that the circuit proves that a certain attestation is indeed a valid TEE attestation given the intel collateral values and the expected measurements. Then with that proof we can compute two values: $s$ and $\sigma$.

- $s$ is the encryption secret s.t $H(s)$ is the encryption key for the confidential message.
- $\sigma$ is a hint that the publisher gives out to whoever knows the witnesses so that if the decryptor does not know the witnesses it's impossible (or at least, mathmatically very complex) to decrypt but if they know the witnesses $\sigma$ is crucial for decryption.

how are these two values computed then?

Before seeing the actual encryptiona and decryption scheme, a note the circuit structure. The DCAP verification circuit I set up takes public inputs (expected measurements, PCK public key, TCB level, etc) and private witness inputs (the raw attestation data, ECDSA signatures, QE report, etc) and returns a single public value `valid` which is $1$ if the attestation checks pass and $0$ if it doesn't. In the circuit terminology, `valid` sits at non-witness variable index 1 (right after the constant $1$), so if I mention the `valid` variable keep in mind that it's referencing this.

## Setup

Since we're working on the QAP directly we have access to the toxic waste out of the box! This makes the implementation easier as we need to rearrange the ciphertext to involve encoded vals for the toxic waste. Note that since the encryptor is carrying out the setup we don't even need to worry about a trusted cerimony, etc. This is solely used for encryption, we do not care if the encryptor can decrypt their own payloads.

### Toxic waste generation

The encryptor picks random scalars (analogous to Groth16's trusted setup, but per-encryption):

$$\alpha, \beta, \delta$$

$$r \in \text{(randomness for hiding)}$$

$$x \in \text{(secret evaluation point)}$$

Again, we can keep these on the setup but if encrypting party wants to keep cryptographic security on the decryption they must not share the toxic values. It's also intersting that we can reuse these values for multiple encryption cycles unless I am missing something in the safety.

### QAP Polynomials

If you don't know how R1CS circuits are generated, they split the circuit into constraints of the form $A \cdot B = C$. So for every constraint we will have a row in the 3 matrices $(A, B, C)$ with the circuits parameters as columns and where the values represent the coefficient of the parameter in the column in either A, B or C.

For instance we take the circuit $a \cdot b = c$ into account. We have one constraint and four variables: the constant $1$, $a$, $b$, $c$. The witness vector is $w = (1, a, b, c)$. The R1CS check is $(A \cdot w) \times (B \cdot w) = C \cdot w$, so:

$$A = \begin{bmatrix} 0 & 1 & 0 & 0 \end{bmatrix}, \quad B = \begin{bmatrix} 0 & 0 & 1 & 0 \end{bmatrix}, \quad C = \begin{bmatrix} 0 & 0 & 0 & 1 \end{bmatrix}$$

Now, from the matrices $(A, B, C)$ for $n$ constraints (over $m$ variables), build polynomials $u_i(X), v_i(X), w_i(X)$ for each variable $i$ so that:

$$u_i(\omega^j) = A_{j,i}, \quad v_i(\omega^j) = B_{j,i}, \quad w_i(\omega^j) = C_{j,i}$$

The vanishing polynomial is $t(X) = X^n - 1$.

> NB: The first $\ell + 1$ variables are non-witness variables (the constant 1, the output `valid`, and the public inputs: expected measurements, PCK pubkey, etc.). The remaining $m - \ell$ variables are witness variables.

We evaluate all QAP polynomials at the secret point $x$ via Lagrange interpolation. This spares us having to interpolate each poly into coefficient form then evaluate at $x$, so we keep complexity at O(domain_size) instead of it being quadratic. Precompute $L_j(x) = \frac{\omega^j \cdot (x^n - 1)}{n \cdot (x - \omega^j)}$ for each $j$ in the domain, then $u_i(x) = \sum_j A_{j,i} \cdot L_j(x)$.

### Computing s

From the public assignment $a_0 = 1, a_1, \ldots, a_\ell$ (which includes `valid` $= 1$ and all expected measurements), compute the public QAP sums:

$$A_{\text{pub}} = \sum_{i=0}^{\ell} a_i \cdot u_i(x)$$

$$B_{\text{pub}} = \sum_{i=0}^{\ell} a_i \cdot v_i(x)$$

$$C_{\text{pub}} = \sum_{i=0}^{\ell} a_i \cdot w_i(x)$$

The encryption secret is:

$$s = e([\alpha]_1, [\beta]_2) \cdot e([r \cdot A_{\text{pub}}]_1, [\beta]_2) \cdot e([\alpha]_1, [r \cdot B_{\text{pub}}]_2) \cdot e([r^2 \cdot C_{\text{pub}}]_1, [1]_2)$$

Or equivalently in target group notation:

$$s = [\alpha\beta]_T \cdot [r\beta \cdot A_{\text{pub}}]_T \cdot [r\alpha \cdot B_{\text{pub}}]_T \cdot [r^2 \cdot C_{\text{pub}}]_T$$

The encryption secret is $k = H(s)$. 

> NB: $s$ depends only on the public inputs and the toxic waste, no witness is needed to compute it.

### Computing $\sigma$

The ciphertext $\sigma$ contains:

- $[\alpha]_1, [\beta]_2, [\delta]_2$ group elements for the toxic waste
- $\lbrace[r \cdot u_i(x)]_1\rbrace_{i=0}^{m}$ $r$-scaled $u_i$ evaluations in $\mathbb{G}_1$ (for **all** variables)
- $\lbrace[r \cdot v_i(x)]_2\rbrace_{i=0}^{m}$ $r$-scaled $v_i$ evaluations in $\mathbb{G}_2$ (for **all** variables)
- $\lbrace[(r\beta \cdot u_i(x) + r\alpha \cdot v_i(x) + r^2 \cdot w_i(x))/\delta]_1\rbrace_{i > \ell}$ combined terms for witness variables only.
- $\lbrace[r^2 \cdot x^j \cdot t(x) / \delta]_1\rbrace_{j=0}^{n-2}$ elements for computing $h(x) \cdot t(x)$

> NB: the combined witness terms are only provided for witness variables ($i > \ell$), not for public input variables which is the key of this construction; to recover the public contributions to $C$ (which are embedded in $s$ thus the key), the decryptor must use the QAP relation $$A(x)B(x)-C(x)=h(x)t(x)$$ and the only way the relation holds is knowing the full witness that are valid for the circuit. 

> NB: intuitively decapsulation is going to rely on the QAP equation to substitute parts of the $s$ equation with values that decryptor holds, i.e without revealing toxic parameters or the randomization scale.

## Decapsulation

The prerequisite is that the decryptor is a TEE that holds a valid DCAP attestation. The core idea is that it then runs the circuit to obtain the full witness $(a_0, \ldots, a_m)$ and uses $\sigma$ to reconstruct $s$.

The TEE feeds its attestation data (td_report_body, ECDSA signatures, QE report, etc) into the circuit's witness generator, producing the full assignment $(a_0, a_1, \ldots, a_m)$. 

> NB: if the attestation is valid and measurements match, `valid` $= 1$.

The following decapsulation algorithm is as it is implemented in the experiemtal crate over at my [github profile](https://github.com/heytdep).

1. Using the full assignment and the ciphertext's $[r \cdot u_i(x)]_1$, $[r \cdot v_i(x)]_2$ elements we compute the QAP sums:

$$[r \cdot A_{\text{full}}]_1 = \sum_{i=0}^{m} a_i \cdot [r \cdot u_i(x)]_1$$

$$[r \cdot B_{\text{full}}]_2 = \sum_{i=0}^{m} a_i \cdot [r \cdot v_i(x)]_2$$

2. compute the witness-variable contributions, i.e witness-only sums:

$$[r \cdot A_{\text{wit}}]_1 = \sum_{i=\ell+1}^{m} a_i \cdot [r \cdot u_i(x)]_1$$

$$[r \cdot B_{\text{wit}}]_2 = \sum_{i=\ell+1}^{m} a_i \cdot [r \cdot v_i(x)]_2$$

3. by subtraction in a division of pairings:

$$[r\beta \cdot A_{\text{pub}}]_T = e([r \cdot A_{\text{full}}]_1, [\beta]_2) \cdot e([r \cdot A_{\text{wit}}]_1, [\beta]_2)^{-1}$$

$$[r\alpha \cdot B_{\text{pub}}]_T = e([\alpha]_1, [r \cdot B_{\text{full}}]_2) \cdot e([\alpha]_1, [r \cdot B_{\text{wit}}]_2)^{-1}$$

> NB: a gotcha here is that division in $\mathbb{G}_T$ is equivalent to multiplying by the inverse. $A_{\text{full}} = A_{\text{pub}} + A_{\text{wit}}$, so the public part falls out by subtraction.

4. we recover $[r^2 \cdot C_{\text{full}}]_T$ via QAP identity. By the QAP equation $A(x)B(x) - C(x) = h(x)t(x)$:

$$[r^2 \cdot h(x) \cdot t(x) / \delta]_1 = \sum_{j} h_j \cdot [r^2 \cdot x^j \cdot t(x) / \delta]_1$$

$$[r^2 \cdot C_{\text{full}}]_T = e([r \cdot A_{\text{full}}]_1, [r \cdot B_{\text{full}}]_2) \cdot e([r^2 \cdot h(x) \cdot t(x)/\delta]_1, [\delta]_2)^{-1}$$

This works because $e([rA]_1, [rB]_2) = [r^2 \cdot A \cdot B]_T = [r^2(C + ht)]_T$, and dividing out $[r^2 \cdot ht]_T$ leaves $[r^2 \cdot C_{\text{full}}]_T$.

5. we are almost there. using the comined witness terms in $\sigma$ we can recover $[r^2 \cdot C_{\text{wit}}]_T$:

$$e\left(\sum_{i > \ell} a_i \cdot \left[\frac{r\beta \cdot u_i + r\alpha \cdot v_i + r^2 \cdot w_i}{\delta}\right]_1, [\delta]_2\right) = [r\beta \cdot A_{\text{wit}}]_T \cdot [r\alpha \cdot B_{\text{wit}}]_T \cdot [r^2 \cdot C_{\text{wit}}]_T$$

Which we continue as

$$[r^2 \cdot C_{\text{wit}}]_T = e(\text{witness-combined}, [\delta]_2) \cdot [r\beta \cdot A_{\text{wit}}]_T^{-1} \cdot [r\alpha \cdot B_{\text{wit}}]_T^{-1}$$

6. lastly we recover $[r^2 \cdot C_{\text{pub}}]_T$ which enables the decrytpor to compute $s$.

$$[r^2 \cdot C_{\text{pub}}]_T = [r^2 \cdot C_{\text{full}}]_T \cdot [r^2 \cdot C_{\text{wit}}]_T^{-1}$$

$$s' = [\alpha\beta]_T \cdot [r\beta \cdot A_{\text{pub}}]_T \cdot [r\alpha \cdot B_{\text{pub}}]_T \cdot [r^2 \cdot C_{\text{pub}}]_T$$

As I noted in the encryption notes, as long as the the witness is valid, i.e all constraints are satisfied and `valid` $= 1$, then $s' = s$, so $k' = H(s') = H(s) = k$. If the QAP equation was violated, say $A(x) \cdot B(x) - C(x) \neq h(x) \cdot t(x)$, then $h(x)$ does not exist as a poly (i.e the division has a remainder), and the $h$ coefficients will be the wrong ones, causing $[r^2 \cdot C_{\text{full}}]_T$ to be incorrect == wrong key!

---

This is incredibly cool math which is implemented in my `wkem-dcap` crate. The non-interactivity of this encryption scheme unlocks a broad range of use cases for TEEs for both resilience and CR, interestingly CR applications were the driving factor for me to try and tackle this problem (more on this in some future research! I'm very excited to work on constructions that can rely on these non interactivity assumptions).

To test out the library:

```rust
// encryption
let wkem = WkemDcap::load(&r1cs_path, &wasm_path, params)?;
let (ct, enc_key) = wkem.encapsulate()?;

// decryption
let full_witness = wkem.compute_witness(&witness)?;
let dec_key = wkem.decapsulate_raw(&ct, &full_witness)?;
```

Unfortunately, this construction is not quantum resistant :( this makes it infeasible to work as is in the longer term but the [original idea](https://limaois.me/en/groth16-we/) can likely be reused for other quantum-resistant zk-SNARKs.
