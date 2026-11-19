# Phase 2C.3: Temporal and Spatial Integration Architecture

**Project:** YUKTI (formerly SUTRAM 2026)  
**Author:** Lead Digital Forensics Systems Engineer  
**Date:** 2026-11-19T22:49:15+05:30  
**Phase:** Phase 2C — End-to-End Forensic Reconstruction Integration  

---

## 1. Temporal Reconstruction Subsystem (`temporal.rs`)

### 1.1 Mathematical Principles of Normalization
Forensic events originate from diverse digital sources with divergent local clocks, timezones, and precisions:
- **Timezone Localization:** Naive datetimes (without explicit UTC offset) are localized in the exhibit's declared IANA timezone (`chrono_tz::Tz`). If the local time falls in an ambiguous or nonexistent DST boundary, an explicit error is returned requiring human clarification.
- **Additive Clock Drift (DEC-001):**
  $$\text{UTC}_{\text{corrected}} = \text{UTC}_{\text{raw}} + \text{Duration}(\text{clock\_offset\_seconds})$$
- **Half-Open Uncertainty Intervals $[start, end)$ (DEC-002):**
  - `EXACT_SECOND`: $[\text{second}, \text{second} + 1\text{ s})$
  - `MINUTE`: floored to local minute boundary, $[M, M + 1\text{ min})$
  - `HOUR`: floored to local hour boundary, $[H, H + 1\text{ hr})$
  - `DAY`: floored to local midnight in source timezone, $[D_{00:00:00}, (D+1)_{00:00:00})$
  - `ESTIMATED`: $[t - w, t + w)$ where $w = \text{estimated\_half\_width\_seconds}$ (default 900 s / 15 min).

### 1.2 Timeline Ordering and Concurrency Classification
Events are sorted chronologically by `nominal_utc`. For each event $E_i$, its uncertainty window $[S_i, E_i)$ is compared pairwise with other events:
- If an event overlaps in time with any other event ($\max(S_i, S_j) < \min(E_i, E_j)$), its status is flagged as `CONCURRENT_UNCERTAIN` and the overlapping event IDs are recorded.
- If an event's uncertainty window is disjoint from all others, it is classified as `STRICLY_ORDERED`.

---

## 2. Spatial Trajectory Subsystem (`spatial.rs`)

### 2.1 WGS84 Geographic Constraints
All coordinates are strictly validated as finite WGS84 coordinates:
$$-90.0 \le \text{lat} \le 90.0, \quad -180.0 \le \text{lon} \le 180.0$$
Points with invalid or non-finite numbers return an explicit `AppError::BadRequest`.

### 2.2 Great-Circle Distance via Haversine Formula (DEC-003)
Given two locations $P_1(\phi_1, \lambda_1)$ and $P_2(\phi_2, \lambda_2)$:
$$\Delta \phi = \phi_2 - \phi_1, \quad \Delta \lambda = \lambda_2 - \lambda_1$$
$$a = \sin^2\left(\frac{\Delta \phi}{2}\right) + \cos(\phi_1)\cos(\phi_2)\sin^2\left(\frac{\Delta \lambda}{2}\right)$$
$$d = 2 \cdot R \cdot \arcsin\left(\sqrt{\text{clamp}(a, 0, 1)}\right)$$
where $R = 6371.0088\text{ km}$ (WGS84 mean spherical radius).

### 2.3 Conservative Minimum Required Speed Calculation
To prevent misleading accusations of impossible transit, speed is calculated using the maximum possible elapsed time across uncertainty windows:
$$\Delta t_{\text{nominal}} = t_{B,\text{nominal}} - t_{A,\text{nominal}}$$
$$\Delta t_{\text{max}} = t_{B,\text{end}} - t_{A,\text{start}}$$
$$v_{\text{min required}} = \begin{cases}
0.0 & \text{if } d = 0 \\
\text{None} & \text{if } \Delta t_{\text{max}} \le 0 \\
\frac{d \cdot 3600.0}{\Delta t_{\text{max}}} & \text{if } \Delta t_{\text{max}} > 0
\end{cases}$$
If $v_{\text{min required}} > v_{\text{threshold}}$, `SpeedThresholdEvaluation::ExceedsConfiguredThreshold` is recorded.
