`13/02/2026`

> I'm not a cryptographer, just a curious person working with applied cryptography to solve distributed systems problems. Feel free to call out any incorrect/imprecise maths!

(zero-dependency rust implementation available at [https://github.com/heytdep/tropicalize](https://github.com/heytdep/tropicalize)).

Before anything, a quick clarification: tropical encryption is not FHE. In FHE you encrypt your data and then someone else can evaluate arbitrary functions on the ciphertext without knowing what data they're processing. In tropical encryption you actually encrypt the *function itself*; you need to know the function at encryption time. The encrypting party takes a tropical polynomial (i.e the function that describes the computation) and scrambles its coefficients through an automorphism so that a worker can evaluate the scrambled version without learning the original structure. This is fundamentally different from FHE where the encryptor doesn't need to care about what computation will happen on their data. That said, tropical encryption does give you some homomorphic properties over $\max$ and $+$ operations on the encrypted function and with essentially zero overhead which is what makes it interesting for a narrow but useful set of problems.

This quick writeup goes over some of the math for tropical encryption and decryption and then explains at a high level how the actual implementation looks like as I wrote a rust implementation from scratch (still missing adding arbitrary complexity tho!); I also briefly go over why this differs from general purpose lattice FHE.

# **Tropical Algebra**

Tropical algebra is essentially a semiring where the two operations are slightly redefined. Addition becomes $\max$ (or $\min$, depending on convention) and multiplication becomes $+$. So a tropical polynomial $f(x) = a \oplus (b \otimes x) \oplus (c \otimes 2x)$ in the max-plus convention is just:

$$f(x) = \max(a, \; b + x, \; c + 2x)$$

Every tropical polynomial is piecewise linear and convex (in the max convention) or concave (in the min convention). The "pieces" are affine functions $a_i \cdot x + b_i$ and the tropical polynomial basically selects the dominant one at each point. This means any piecewise linear convex function is automatically a tropical polynomial; not an approximation, an exact correspondence.

Turns out a surprisingly large number of real functions are piecewise linear. Orderbook fill curves (cost to fill $q$ units, greedy from cheapest), ReLU networks, shortest path computations (tropical matrix multiplication actually solves all-pairs shortest paths), supply/demand curves with discrete price levels. If your function lives in this class you get tropical structure for free.

# **Tropical Encryption**

If $p$ is an automorphism of the coordinate ring, substituting $p(x)$ into a tropical polynomial preserves the algebraic structure. Intuitively if $f(x) = \max(a_i \cdot x + b_i)$ and $p(x) = Mx + d$ is an affine automorphism then:

$$f(p(x)) = \max(a_i \cdot (Mx + d) + b_i) = \max((a_i M) \cdot x + (a_i \cdot d + b_i))$$

The encrypted polynomial has new coefficients and intercepts; the originals are hidden behind the mixing matrix $M$. But structurally it's still a tropical polynomial which means you can still evaluate it, take the max of two encrypted polynomials, add constants; all homomorphically.

The protocol is fairly straightforward. **Key generation**: pick an invertible affine automorphism $p(x) = Mx + d$; the private key is $p^{-1}$. **Encryption**: given tropical polynomial $f$, publish $f_{enc}(x) = f(p(x))$. **Evaluation**: a worker who wants to evaluate at some point $q$ receives the encrypted query $q' = p^{-1}(q)$ from the key holder then computes $f_{enc}(q') = f(p(p^{-1}(q))) = f(q)$. Correct result but the worker never actually sees the original polynomial. **Homomorphic ops**: $\max(f_{enc}, g_{enc})$ and $f_{enc} + c$ work directly on ciphertexts.

> NB: in 1D the automorphism $q \mapsto aq + b$ is trivially breakable (two evaluations give two equations, two unknowns). real schemes use multivariate tropical polynomials where $M$ is a high-dimensional mixing matrix so that inversion is much harder. security grows with dimension but there is no formal hardness reduction to a well-studied problem which makes tropical encryption "short-lease"; fine for short-lived confidentiality, not for long-term secrets.

# **How This Slightly Differs from Lattice FHE**

As I mentioned above, tropical encryption is not FHE. But beyond the "encrypt function vs encrypt data" distinction they also differ in basically everything else.

Lattice FHE (BFV, BGV, CKKS, TFHE, etc) encrypts data into high-dimensional lattice points with noise. Every operation slightly increases noise. Once noise exceeds a threshold decryption fails. Bootstrapping (i.e homomorphically evaluating the decryption circuit) resets the noise but at very high computational cost. You can compute anything on the ciphertext, arbitrary circuits, but ciphertexts are massive and operations are expensive. For a single encrypted multiplication you're looking at milliseconds to seconds depending on the scheme.

Tropical encryption has none of this. No noise, no lattice, no bootstrapping. The ciphertext is the same size as the plaintext (just different coefficients). Encryption is basically a matrix multiplication. Evaluation is just evaluating a piecewise linear function. The overhead is essentially zero. But you can only compute $\max$, $\min$ and $+$. No multiplication. A percentage fee ($f(x) \times 1.1$) breaks the automorphism. An AMM invariant ($x \cdot y = k$) is out of reach. Lattice FHE has strong security (LWE hardness, post-quantum) while tropical encryption has no formal hardness proof. They're not really competing; different tools for different constraints.

# **tropicalize**

I wrote [tropicalize](https://github.com/heytdep/tropicalize) as a minimal zero-dependency Rust library to make this slightly more concrete. No crates, no external math libraries; just the raw linear algebra and tropical structures by hand.

The foundation is `AffinePiece`; essentially a single affine function $a \cdot x + b$ which can be multidimensional:

```rust
pub struct AffinePiece {
    coeffs: Vec<f64>,
    intercept: f64,
}
```

A `TropicalPoly` is a collection of pieces with a selection operator (max or min). The constructor infers convexity from the coefficient ordering:

```rust
pub struct TropicalPoly {
    pieces: Vec<AffinePiece>,
    select: TropicalSelect,
}
```

Evaluation iterates pieces, evaluates each and selects the max (or min):

```rust
impl PolyEval for TropicalPoly {
    fn evaluate(&self, p: &[f64]) -> f64 {
        let mut select = self.pieces.first().unwrap().evaluate(p);
        for piece in self.pieces.iter().skip(1) {
            let at_p = piece.evaluate(p);
            select = if let TropicalSelect::Max = self.select {
                select.max(at_p)
            } else {
                select.min(at_p)
            }
        }
        select
    }
}
```

Encryption is an affine substitution $p(x) = Mx + d$. The `AutomorphismMatrix` stores the mixing matrix and offset and implements encryption as the coefficient transformation $(a_i M, \; a_i \cdot d + b_i)$:

```rust
impl AffinePiece {
    pub fn encrypted(&self, matrix: &AutomorphismMatrix) -> Self {
        let dims = self.coeffs().len();
        let mut new_coeffs = vec![0_f64; dims];
        let mut new_intercept = self.intercept();

        for (coeff_idx, coeff) in self.coeffs().iter().enumerate() {
            for j in 0..dims {
                new_coeffs[j] += *coeff * matrix.matrix[coeff_idx][j]
            }
            new_intercept += coeff * matrix.offset[coeff_idx]
        }

        AffinePiece::new(new_coeffs, new_intercept)
    }
}
```

Inversion is Gauss-Jordan elimination, also from scratch. The inverse offset is $-M^{-1} d$:

```rust
pub fn invert(&self) -> Option<AutomorphismMatrix> {
    let n = self.offset.len();
    let mut aug = vec![vec![0.0; 2 * n]; n];
    // ... Gauss-Jordan elimination ...
    // inv_offset = -M^{-1} · d
    let mut inv_offset = vec![0.0; n];
    for i in 0..n {
        for j in 0..n {
            inv_offset[i] -= inv_matrix[i][j] * self.offset[j];
        }
    }
    Some(AutomorphismMatrix { matrix: inv_matrix, offset: inv_offset })
}
```

Real orderbook curves aren't given as raw affine pieces; they're given as price levels with quantities. The `optimize` module takes `CurveInputs` (basically a sequence of slopes mapped to breakpoints) and computes the correct intercepts by enforcing continuity at each breakpoint. The `checks` module validates the curve before encryption: coefficients must exist, dimensions must be uniform across pieces, the curve must be monotone in curvature (either convex or concave; mixing would break the tropical correspondence) and the automorphism matrix dimensions must match the polynomial.

# **Worked Example: Encrypted Orderbook Fill**

To make the above a bit more tangible let's trace through the `orderbook` binary. Three price levels on the bid side: buy 5 at \$10, buy 10 at \$8, buy 20 at \$6. The fill curve $C(q)$; total cost to sell $q$ units into these bids; is piecewise linear:

$$C(q) = \max(10q, \; 8q + 10, \; 6q + 30)$$

> NB: the intercepts are computed from continuity at breakpoints. at $q = 5$: $10(5) = 50$ and $8(5) + 10 = 50$. at $q = 15$: $8(15) + 10 = 130$ and $6(15) + 30 = 120$. the `optimize` module handles this automatically.

Now encrypt with automorphism $p(q) = 8q + 2$:

$$C_{enc}(q) = C(8q + 2) = \max(80q + 20, \; 64q + 26, \; 48q + 42)$$

An observer sees slopes $(80, 64, 48)$ and intercepts $(20, 26, 42)$; the original $(10, 8, 6)$ and $(0, 10, 30)$ are hidden.

To evaluate at $q = 12$ the key holder computes $q' = p^{-1}(12) = (12 - 2)/8 = 1.25$ and the worker evaluates:

$$C_{enc}(1.25) = \max(80(1.25) + 20, \; 64(1.25) + 26, \; 48(1.25) + 42) = \max(120, 106, 102) = 120$$

Which is exactly $C(12) = 120$. The worker learns the fill price but not the actual book structure.

# **Limitations**

Tropical encryption is narrow. The moment you need multiplication; percentage fees, geometric means, AMM curves, anything slightly nonlinear; it breaks. The automorphism preserves $\max$ and $+$ because those are the tropical operations but classical multiplication is not a tropical morphism.

The security is also honestly limited: there's no reduction to a well-studied hard problem. An adversary with enough query access could likely recover the automorphism. The security depends on the dimension of $M$ and the adversary's compute budget but without a formal hardness proof the best we can say is "short-lease"; the encryption only needs to hold until the data it protects becomes stale.

# **Where This Gets Interesting**

What makes tropical encryption a bit unusual is that the worker computing on ciphertexts needs nothing special. No FHE libraries, no noise tracking, no bootstrapping. They just evaluate a piecewise linear function. They don't even need to know the data is encrypted; the ciphertext *is* a valid tropical polynomial, just with scrambled coefficients. So any environment that can evaluate piecewise linear functions can essentially serve as a computation layer over encrypted data. The decryption capability stays with whoever holds the automorphism inverse.

Now consider an environment that can perform attested computation; third parties can verify *what* was computed, not just that *something* was computed; and that sits between two parties who need each other's data processed but don't trust each other with it. An intermediary that holds $p^{-1}$, evaluates queries on the encrypted structure, and whose correct execution is verifiable gives you confidentiality from a party that both sides can trust not because of reputation but because of the computation environment itself. The short-lease concern basically goes away when the lease is bounded by the intermediary's attestation cycle, and the "no formal hardness proof" concern matters a lot less when the adversary can only access the function through a verified interface that rotates keys.

I think the class of computations that are both useful and expressible as $\max / +$ over encrypted data within such an environment is probably larger than most people would expect. But more on this in a future write-up.

---

So yeah to conclude tropical encryption is not a replacement for FHE and it's not even the same kind of thing. For the narrow slice of computation that lives in $\max / +$; orderbook matching, shortest paths, piecewise linear optimization; you get homomorphic properties at essentially zero cost with the caveat that you must know the function at encryption time.
