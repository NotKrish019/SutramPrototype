# YUKTI demo case (synthetic)

Fabricated data for demonstrations and tests. No real person, number or event.

| File | What it is |
|---|---|
| `carrier_a_cdr.csv` | The suspect number's own operator CDR: Saket at 21:00, Mumbai BKC at 21:30 and 23:10 (IST). |
| `carrier_b_tower_dump.csv` | A second operator's tower dump placing the same number in Noida Sector 62 at 20:55:10 (IST). |
| `whatsapp_chat.txt` | An Android WhatsApp export between the suspect and a courier. |

What the reconstruction finds, and why it is useful in a demonstration:

* **Noida 20:55 → Saket 21:00** (about 19.8 km in under 5 minutes, two different operators'
  clocks): contradiction leads that are **FRAGILE** / **SENSITIVE**. A clock disagreement of
  roughly two minutes between the operators, or a few kilometres of tower-location error, would
  explain them, so they must be corroborated before they are relied on.
* **Saket 21:00 → Mumbai 21:30** (about 1,140 km in 30 minutes, the same operator record):
  a contradiction lead that is **ROBUST**. Both events share one clock, and no tower-location
  error explains about 1,140 km, so the lead survives every realistic error considered. It is still a
  lead (shared or cloned SIM, data error), not proof.
* The WhatsApp export has minute-precision timestamps, a multi-line message, a system notice and
  no location, so it adds timeline events and evidence gaps without adding coordinates.
