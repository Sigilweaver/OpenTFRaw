# Frequency-to-M/z Conversion

_Frequency-to-M/z Conversion_

## 32. Frequency-to-M/z Conversion

FTMS analyzers (Orbitrap, LTQ-FT) store profile data in the **frequency
domain**. The ScanEvent contains conversion coefficients to translate
frequencies to M/z values.

### 32.1 LTQ-FT Conversion (nparam == 4)

Coefficients: `[unknown, A, B, C]`

```
M/z = A + B/f + C/f²
```

Where `f` is the frequency value.

### 32.2 Orbitrap Conversion (nparam == 5 or 7)

Coefficients: `[unknown, (I,) A, B, C (, D, E)]`

```
M/z = A + B/f² + C/f⁴
```

### 32.3 Inverse Conversion (M/z to Frequency)

Used when looking up a specific M/z in frequency-domain profile data:

**LTQ-FT**: Solve quadratic `C + Bf + (A - Mz)f² = 0`:
```
f = (-B - sqrt(B² - 4C(A - Mz))) / (2(A - Mz))
```

**Orbitrap**: Solve `(A - Mz) + B/f² + C/f⁴ = 0`:

Let `x = 1/f²`:
```
Cx² + Bx + (A - Mz) = 0
x = (-B - sqrt(B² - 4C(A - Mz))) / (2C)
f = 1/sqrt(x)
```

### 32.4 Direct M/z Data

When `profile.step > 0` (positive step), the data is directly in M/z domain
and no frequency conversion is needed. The M/z of bin `i` is:
```
mz = profile.first_value + i * profile.step
```

---

