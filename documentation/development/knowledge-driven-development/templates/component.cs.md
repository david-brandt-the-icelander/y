# Component annotation — worked example

Components carry no separate document. This file shows the shape; the rules are in
[03](../03-code-annotation-convention.md).

```csharp
/// <summary>
/// Claims the next machine in the wash queue for this peer.
/// ⚠️ Does not respect station opening hours — a claim succeeds outside opening hours and the wash
/// starts. Required: reject the claim and return false when the station's schedule says closed.
/// </summary>
public async Task<bool> ClaimAsync(int machineId)
{
    if (_clock.UtcNow < _leaseUntil)
        return false;            // 🙉 exercised only via WashQueueTests; no test names this guard

    try { return await _db.ClaimAsync(machineId); }
    catch (SqlException)
    {
        // 🙈 the executor is faked in every test, so nothing reaches this handler; a real deadlock
        // takes the untested path and we do not know it returns false rather than throwing
        return false;
    }
}

/// <summary>
/// ✅ Releases a claim, returning false when the caller does not hold it.
/// </summary>
public bool Release(int machineId)
```

Reading it back:

- The member-level ⚠️ describes the missing behaviour in enough detail to implement from. That
  paragraph is the backlog item.
- ✅ on `Release` asserts **every branch** is covered by a direct unit test — not that a test exists.
- A member containing any 🙈 🙉 🙊 comment can never be ✅. `ClaimAsync` is ⚠️ because the unbuilt
  requirement is the worse fact; without it, it would be 🙈.
- Both members' tests are at the path computed by
  [the location rule](../03-code-annotation-convention.md#the-test-location-rule) — otherwise ✅ is not
  available and the mark is 🙉.
- When the ⚠️ closes and every branch is covered, **the ✅ on `Release` is deleted too**. A file with no
  gaps carries no marks. → [graduation](../02-annotation-convention.md#graduation)
