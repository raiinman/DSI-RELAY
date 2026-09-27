# UEFN Verse telemetry example

**UEFN status: UNTESTED.** This is a project-installable template based on Epic's documented [Verse creative device lifecycle](https://dev.epicgames.com/documentation/fortnite/verse-api/fortnitedotcom/devices/creative_device), [diagnostics logger](https://dev.epicgames.com/documentation/fortnite/debugging-and-troubleshooting-in-verse), and [simulation elapsed time](https://dev.epicgames.com/documentation/fortnite/verse-api/versedotorg/simulation/getsimulationelapsedtime). It has not been compiled or run in UEFN. No real play-session result is claimed.

## Install in a project

1. In UEFN's Verse Explorer, add a Verse device named `relay_telemetry_example`. Replace its source with [relay_telemetry_example.verse](relay_telemetry_example.verse).
2. Build Verse code, then place one instance of the resulting device in the island. Epic's [device guide](https://dev.epicgames.com/documentation/fortnite/verse-api/fortnitedotcom/devices/creative_device) describes this compile-and-place flow.
3. Set `SessionNumber` on the device to a new integer for each captured session. The emitted session ID is `example_<number>`; use the same ID in RELAY's capture context. Do not put player names, account identifiers, paths, or secrets into event fields.
4. Launch a play session, then stop it normally. Inspect UEFN's Output Log for `RELAY_EVENT_V1 `. The diagnostics logger writes at Normal level to the Output Log without showing the messages on the player screen. [Epic documents that output surface](https://dev.epicgames.com/documentation/fortnite/debugging-and-troubleshooting-in-verse).
5. For the current RELAY parser, retain **only the substring starting at** `RELAY_EVENT_V1 ` from each matching UEFN log line, one message per line with a final newline. UEFN log timestamps/channel prefixes are not part of the RELAY event. This manual extraction is temporary; no trusted live capture adapter is established by this template.

## Exact event payload

For `SessionNumber = 42`, the device's three message payloads have this form, in order. Milliseconds are measured from this device's `OnBegin`; the example below uses illustrative values for the probe and end marker.

```text
RELAY_EVENT_V1 {"schema_version":1,"session_id":"example_42","sequence":1,"session_time_ms":0,"kind":"session_start"}
RELAY_EVENT_V1 {"schema_version":1,"session_id":"example_42","sequence":2,"session_time_ms":1,"kind":"project_device_ready","subject_ref":"relay_telemetry_example","state":"observed"}
RELAY_EVENT_V1 {"schema_version":1,"session_id":"example_42","sequence":3,"session_time_ms":1000,"kind":"session_end"}
```

`project_device_ready` is the example's project-defined probe. It observes only that this Verse device reached `OnBegin`; it is not a spawn, objective, or gameplay assertion. The parser in `crates/relay-verse` accepts these bounded JSON fields and checks sequence and session boundaries. If `OnEnd` does not emit or the log capture misses any line, RELAY must report the capture as incomplete. Even a complete imported capture remains unverified until a trusted adapter associates it with a real UEFN play session.
