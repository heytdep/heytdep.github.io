`30/01/2025`

In this article I expand a bit more about my new MeV + node behavior verification proposal by demistifying the PoW-based hashing attack vector for optimal ordering. 

<br/>

MeV has always been a hot topic in crypto, and there's generally been the common knowledge that it's not possible on the Stellar Network. MEV can take place in different shapes: atomic arbitrage, liquidations, front-running, back-running (or both), and in my MeV proposal I only talk about atomic arbitrage and liquidations, which only really impact traders.

Sandwiching on the other hand, has historically raised some concerns on other chains' MeV clients due to its impact on retail users, but that's out of the scope of my MeV proposal, but the reason why it's not in the list has less to do with the community not wanting to introduce front and back-running on stellar (we don't want to fwiw) and more to do with the feasibility of such MeV. In the end I will also talk about how the producer behavior verification part of the proposal makes sandwiching impossible in the network without the malicious validator's experiencing consequences.

# Optimal Transaction Ordering on Stellar.

Let's take a look at why it's not deterministally possible to optimally order transactions on Stellar to perform sandwich MeV, and what would the only non-deterministically possible attack be.

At a high level, when a node receives a new transaction the default behavior is to flood it to other physically connected nodes through the overlay, validators construct their own transaction sets which then get gathered by the nomination protocol which eventually produces a single composite value, which is then finalized by the ballot protocol. 

Without covering the whole workflow of transactions, let's zoom in the parts relevant to transaction ordering and see what this actually means.

## Sorting Transactions for Apply Order

The first shield the stellar newtork places towards randomizing the ordering of the transactions, or better, making it complex to obtain optimal ordering, is to pseudo-randomly (we still need to enforce deterministic behaviour) re-order transactions within the same batch (transactions are grouped into different batches depending on the account's sequence number since that must be respected) by sorting each transaction using a sorter derived by XORing the transaction hash itself and the hash of the transaction set:

```c
TxFrameList
sortedForApplySequential(TxFrameList const& txs, Hash const& txSetHash)
{
    // build txBatches based off sequence numbers ...
    for (auto& batch : txBatches)
    {
        // randomize each batch using the hash of the transaction set as a way to randomize even more
        ApplyTxSorter s(txSetHash);
        std::sort(batch.begin(), batch.end(), s);
        // push new pseudo-randomized array to retList
    }
    //..
    return retList;
}
```

What this actually means is that ordering $n$ transactions within the same batch is cryptographically hard: to order $n$ transactions after transaction $i$ means we need to find a a sorter

$$sortKey( \ tx \ ) = H( \ txset \ || \ txid)$$

so that: 

$$sortKey( \ tx_{i} \ ) < .. < sortKey( \ tx_{i+n} \ )$$

That makes it so the chance of getting those $n$ transactions ordered is $P = 1/(n!)$.

*If* $n$ is a small number, then we have a pretty decent chance of this happening (e.g if $n=2$ or $n=3$). Even if we can carry sandwich attacks out of a probabilitic vector, the chances are pretty slim ($P = 1/6$). Simply monitoring the mempool and frontrunning a transaction out of probability is a much simpler and functional approach and something [I actually experimented with](https://x.com/heytdep/status/1851245346728616054).

In any case, probabilitic optimal transaction ordering is a really fair compromise, probably the fairest, and that's why there is no evidence of sandwich attacks happening on the Stellar network. However, what if we didn't want to take any chances and wanted to make sure that the condition:

$$sortKey( \ tx_1 \ ) < sortKey( \ tx_2 \ ) < sortKey( \ tx_3 \ )$$ 

always holds effectively sandwiching a target transaction $tx_2$?

Well, even if we cannot invert the hash funciton, turns out it's not that much complex in theory since to order $n$ transactions we need about (approximation) $n!$ attempts at variating the txset to obtain the target permutation. 

Not bad! I think? Generally when talking about MeV clients, transactions are bundled for optimal extraction i.e we need to bundle more than three transactions; and I think we all know how the $y = x!$ chart looks like. That said, if I was a validator that wanted to sandwich a single transaction $TX_t$ it is theoretically pretty feasible. 

However, ...

## Highest Hash Order

Without diving into the details of how SCP nomination works (that's maybe for another post), let's break down what happens at a high level: (i) nomination converges a set of candidate values for the slot, where the candidates are the various proposed transaction sets. (ii) The nodes deterministically combine the candidates into one resulting composite value.

While the method to carry out step 2 isn't exactly specified in the SCP whitepaper, it is in stellar-core's code:

```c
ValueWrapperPtr
HerderSCPDriver::combineCandidates(uint64_t slotIndex,
                                   ValueWrapperPtrSet const& candidates)
{
    // ...
    // take the txSet with the biggest size, highest xored hash that we have
    {
        auto highest = candidateValues.cend();
        TxSetXDRFrameConstPtr highestTxSet;
        ApplicableTxSetFrameConstPtr highestApplicableTxSet;
        for (auto it = candidateValues.cbegin(); it != candidateValues.cend();
             ++it)
        {
            auto const& sv = *it;
            auto cTxSet = mPendingEnvelopes.getTxSet(sv.txSetHash);
            releaseAssert(cTxSet);
            // Only valid applicable tx sets should be combined.
            auto cApplicableTxSet = cTxSet->prepareForApply(mApp);
            releaseAssert(cApplicableTxSet);
            if (cTxSet->previousLedgerHash() == lcl.hash)
            {

                if (!highestTxSet ||
                    compareTxSets(*highestApplicableTxSet, *cApplicableTxSet,
                                  highest->txSetHash, sv.txSetHash,
                                  highestTxSet->encodedSize(),
                                  cTxSet->encodedSize(), lcl.header,
                                  candidatesHash))
                {
                    highest = it;
                    highestTxSet = cTxSet;
                    highestApplicableTxSet = std::move(cApplicableTxSet);
                }
            }
        }
        if (highest == candidateValues.cend())
        {
            throw std::runtime_error(
                "No highest candidate transaction set found");
        }
        comp = *highest;
    }
    // ... and return
}
```

As mentioned above, in this code nomination is deciding which of the candidate transaction sets becomes the single composite value. More specifically, we're searching a candidate that: (i) respectes the required constraints, i.e previous ledger matches. (ii) compares the currently examined txset with the currently highest candidate, if the new candidate is better then we place it as the highest candidate.

How does the algorithm decide which candidate is better than the other though?

```c
// returns true if l < r
// lh, rh are the hashes of l,h
static bool
compareTxSets(ApplicableTxSetFrame const& l, ApplicableTxSetFrame const& r,
              Hash const& lh, Hash const& rh, size_t lEncodedSize,
              size_t rEncodedSize, LedgerHeader const& header, Hash const& s)
{
    auto lSize = l.size(header);
    auto rSize = r.size(header);
    if (lSize != rSize)
    {
        return lSize < rSize;
    }
    if (protocolVersionStartsFrom(header.ledgerVersion,
                                  SOROBAN_PROTOCOL_VERSION))
    {
        auto lBids = l.getTotalInclusionFees();
        auto rBids = r.getTotalInclusionFees();
        if (lBids != rBids)
        {
            return lBids < rBids;
        }
    }
    if (protocolVersionStartsFrom(header.ledgerVersion, ProtocolVersion::V_11))
    {
        auto lFee = l.getTotalFees(header);
        auto rFee = r.getTotalFees(header);
        if (lFee != rFee)
        {
            return lFee < rFee;
        }
    }
    if (protocolVersionStartsFrom(header.ledgerVersion,
                                  SOROBAN_PROTOCOL_VERSION))
    {
        if (lEncodedSize != rEncodedSize)
        {
            // Look for the smallest encoded size.
            return lEncodedSize > rEncodedSize;
        }
    }
    return lessThanXored(lh, rh, s);
}
```

Reading the code we can easily gain insight at which transaction sets are considered better:

1. If the two sets have different sizes, the one with the highest amount of transactions wins.
2. On two sets with the same size, nomination will prefer those with the highest amount of inclusion fees.
3. If two sets have different encoded sizes, we choose the smaller one.
4. If none of the above conditions is triggered (i.e the two sets look identical, but maybe the ordering is different), nomination performs a hash-based XOR comparison over parameter `x` as tie-break:

```c
bool
lessThanXored(Hash const& l, Hash const& r, Hash const& x)
{
    Hash v1, v2;
    for (size_t i = 0; i < l.size(); i++)
    {
        v1[i] = x[i] ^ l[i];
        v2[i] = x[i] ^ r[i];
    }

    return v1 < v2;
}
```

The code shows us that we either replace or not the currently highest transaction set pseudo-randomly (we still need to be deterministic!) by XORing the two hashes over `x`, effectively:

$$R = (lh \oplus s ) < (rh \oplus s )$$

Where, if you paid attention, `s` is `candidatesHash` within `combineCandidates` which is the hash of incrementally XORing of all the candidate values:

```c
Hash candidatesHash;
std::vector<StellarValue> candidateValues;
for (auto const& c : candidates)
{
    candidateValues.emplace_back();
    StellarValue& sv = candidateValues.back();
    xdr::xdr_from_opaque(c->getValue(), sv);
    candidatesHash ^= sha256(c->getValue());
}
```

This yields a hash that depends on all of the candidates and by relying on XOR's commutative property we make it so it doesn't depend on the order of how those where processed. 

It wasn't so easy wasn't it? Along with having to brute-force the txset to have the applyorder respect our constraints, we also need to make sure that our txset is the best one! Intuitively this is something we cannot determinisically do unless we know what are the other nodes' transaction sets *before* nomination starts.

<br/>

To wrap it up, in order to optimally order the transaction set to extract value a validator would need to: (i) construct a txset that respects the ordering they wish for once pseudo-randomized. We cannot algebraically invert the hashing function so we need to brute-force our txset. (ii) while respecting the above charateristics, the txset must also be the best candidate.

While not impossible, these two shields that the stellar network sets up greatly increase the difficulty of coming up with out winner txset **in time**, all while taking into account that our attack is non-deterministic.

If however, a skilled validator was able to figure out an algorithm that grants much higher probabilities of optimal inclusion than the good old probabilistic attack, we generally wouldn't be able to know it! That's not good, but is also why my newly advanced proposal to verify proposer behavior becomes important.

# Eclipse MeV and Producer Behaviour Verification Proposal.

The TLDR of the proposal is that I want to help validators perform eclipse MeV, i.e having first dibs on inventory-less MeV ops like atomic arbitrage and liquidations, all while having external watcher nodes make sure that such validators are not abusing their power by eclisping non-MeV transactions (effectively censoring the network). 

We do this check by comparing the applied transaction set with the valid transactions that the network was flooded with. While this requires verifiers to have a very complete view of all of the transactions the network was flooded with to be able to detect a single/small transaction censorships, we can fairly easily observe a node that tries to forward their own optimally ordered transaction set. That's because the malicious node would need to insert or exclude multiple transactions in the set to even just brute-force their way into a correct apply set.

As a result, if optimal transaction ordering was really difficult before, it will be impossible now. Or better, any block producer (due to how scp priority works it will be reputable t1 orgs) that tries to employ the above-described attack vector will be easily picked up by external observers loosing all their reputation and resulting in the organization's trust within the quorum being likely revoked by other nodes. That's the beauty of Federated Byzantine Agreement systems!

Here's the link to my original MeV proposal: [https://docs.google.com/document/d/1sBdKqgMdh8Qgc0jrgZzY4ELQY2jyi1gX0pJ056YUIug/edit?usp=sharing](https://docs.google.com/document/d/1sBdKqgMdh8Qgc0jrgZzY4ELQY2jyi1gX0pJ056YUIug/edit?usp=sharing). You can join [the discussion](https://discord.com/channels/897514728459468821/1333564536695033937/1333564536695033937) on the stellar developers discord server.

<br/>

That's it for today, if you enjoyed the read make sure to share it so we can grow awareness towards (i) how neat stellar's arbitrary tx ordering protection design is and (ii) to further shine light on community research in the network!
