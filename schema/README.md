# Vendored mzML schemas

PSI-MS mzML 1.1 XSD schemas, vendored so CI can validate converter
output offline (no network fetch during the build).

| File | Root element | Purpose |
| --- | --- | --- |
| `mzML1.1.0.xsd` | `mzML` | Base schema. Validates non-indexed output. |
| `mzML1.1.2_idx.xsd` | `indexedmzML` | Index wrapper. `xs:include`s `mzML1.1.0.xsd`, so both files must live in the same directory. Validates indexed output. |

## Provenance

Retrieved from the HUPO-PSI mzML schema repository on 2026-07-04:

- Source: <https://github.com/HUPO-PSI/mzML/tree/master/schema/schema_1.1>
- `mzML1.1.0.xsd` sha256 `141877fd774653617ad1c067533bc74d51e63fb9f19f78558faff50a3e14a206`
- `mzML1.1.2_idx.xsd` sha256 `2496da7d0f66d465a82a903ca71be28013e63a109cbf278a5db20820a194a8b0`

These files are unmodified. The `_idx` schema references the base by
the relative path `mzML1.1.0.xsd`; keep them together.

## Usage

```sh
# non-indexed output
xmllint --noout --schema schema/mzML1.1.0.xsd out.mzML
# indexed output
xmllint --noout --schema schema/mzML1.1.2_idx.xsd out_indexed.mzML
```
