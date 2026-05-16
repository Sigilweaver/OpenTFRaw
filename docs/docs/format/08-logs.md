# Error and Instrument Logs

_Error Log, Instrument Log_

## 29. Error Log

Array of error entries located at `RunHeader.error_log_addr`. The number of
entries is given by `SampleInfo.error_log_length`.

### 29.1 Error Entry

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | Float32 | time | Retention time (minutes) |
| 2 | PascalStringWin32 | message | Error message text |

---

## 30. Instrument Log

Instrument log stream located at `RunHeader.inst_log_addr`. The number of
entries is given by `SampleInfo.inst_log_length`.

### 30.1 Stream Layout

| Order | Type | Description |
|-------|------|-------------|
| 1 | GenericDataHeader | Schema for log entries |
| 2 | GenericRecord[n] | One record per log entry |

Log entries typically include temperatures, pressures, voltages, and other
instrument operating parameters recorded periodically during acquisition.

---

