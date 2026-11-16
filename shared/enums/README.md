# Shared Enumerations & Constant Taxonomies

This directory defines the cross-language enumeration values used across the Python backend, React frontend, and Node.js decision service.

---

## 1. `CaseStatus`
- `DRAFT`: Initial draft of case file.
- `ACTIVE`: Active ongoing digital forensics investigation.
- `UNDER_REVIEW`: Analysis complete; pending supervisor sign-off.
- `ARCHIVED`: Closed and archived.

## 2. `EvidenceType`
- `CCTV`: Video surveillance footage.
- `CDR`: Telephony Call Detail Records.
- `CHAT_EXPORT`: WhatsApp, Telegram, Signal, SMS export files.
- `DOCUMENT_SCAN`: PDF, JPEG, TIFF scans of physical case papers.
- `DISK_IMAGE`: Raw digital storage dumps (E01, RAW, DD).
- `AUDIO_RECORDING`: Wiretap or intercepted voice communications.
- `FINANCIAL_LEDGER`: Bank statements, transaction CSVs.

## 3. `EntityType`
- `PERSON`: Human subject (suspect, victim, witness).
- `PHONE_NUMBER`: MSISDN, landline number.
- `VEHICLE`: License plate number, VIN.
- `LOCATION`: Physical address, landmark, geolocation.
- `ORGANIZATION`: Corporate, syndicate, or governmental body.
- `DEVICE`: IMEI, MAC address, serial number.
- `CRYPTO_WALLET`: Blockchain address.

## 4. `RelationType`
- `COMMUNICATED_WITH`: Telephony, chat, or electronic message exchange.
- `PRESENT_AT`: Co-location at an event or geographic coordinate.
- `CO_OCCURRED_WITH`: Observed in close spatial-temporal proximity.
- `OWNS`: Property, vehicle, or account ownership.
- `ASSOCIATE_OF`: Known social or organizational affiliation.
- `CONTRADICTS`: Mutually exclusive factual claims detected across sources.

## 5. `TimePrecision`
- `EXACT_SECOND`: Hardware or NTP-synchronized timestamp.
- `MINUTE`: Timestamp known to the minute.
- `HOUR`: Coarse hourly estimate.
- `DAY`: Date only.
- `ESTIMATED`: Subjective or investigator-inferred temporal window.
