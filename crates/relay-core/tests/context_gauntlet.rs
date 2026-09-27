//! Deterministic first Context Gauntlet fixture. No model or tokenizer is used.
use relay_contracts::{CommandRequest, RequestContext};
use relay_core::service::{CoreConfig, RelayCore, RuntimeContext};
use serde_json::{Value, json};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

const BUDGET: usize = 1024;

struct Fixture {
    name: &'static str,
    payload: Value,
    retained: Vec<(&'static str, Value)>,
    absent: Vec<&'static str>,
}

fn request(id: &str, command: &str, arguments: Value) -> CommandRequest {
    CommandRequest {
        request_id: id.into(),
        command: command.into(),
        command_version: Some(1),
        arguments,
        idempotency_key: None,
        context: RequestContext {
            actor_id: Some("ACTOR-gauntlet".into()),
            client_id: Some("CLIENT-gauntlet".into()),
            delegator_id: None,
        },
    }
}

fn placement(name: &'static str, target_index: usize) -> Fixture {
    let mut entries: Vec<Value> = (0..101)
        .map(|index| json!({ "message": format!("unrelated event {index:03}") }))
        .collect();
    entries[target_index] = json!({
        "message": "target is embedded in prose-heavy history",
        "target_id": "TARGET-EXACT-42",
        "generation": 73
    });
    Fixture {
        name,
        payload: json!({ "entries": entries }),
        retained: vec![
            ("/entries/target_id", json!("TARGET-EXACT-42")),
            ("/entries/generation", json!(73)),
        ],
        absent: vec![],
    }
}

fn fixtures() -> Vec<Fixture> {
    let mut cases = vec![
        placement("target_at_start", 0),
        placement("target_in_middle", 50),
        placement("target_at_end", 100),
    ];
    cases.push(Fixture {
        name: "stale_then_current",
        payload: json!({
            "history": [
                { "state": "READY", "generation": 8, "message": "old state" },
                { "state": "STALE", "generation": 9, "message": "continuity was lost" }
            ],
            "current": { "state": "READY", "generation": 10, "message": "verified again" }
        }),
        retained: vec![
            ("/history/0/state", json!("READY")),
            ("/history/1/state", json!("STALE")),
            ("/current/state", json!("READY")),
            ("/current/generation", json!(10)),
        ],
        absent: vec![],
    });
    cases.push(Fixture {
        name: "repeated_warnings",
        payload: json!({
            "target_id": "TARGET-EXACT-42",
            "warnings": vec![json!({ "warning_code": "W-REPEATED" }); 600]
        }),
        retained: vec![("/target_id", json!("TARGET-EXACT-42"))],
        absent: vec!["/warnings/599/warning_code"],
    });
    cases.push(Fixture {
        name: "coordinate_in_noise",
        payload: json!({
            "target_id": "TARGET-EXACT-42",
            "coordinate": { "x": 12.125, "y": -44.5, "z": 0.75 },
            "logs": vec!["unrelated detail"; 120]
        }),
        retained: vec![("/target_id", json!("TARGET-EXACT-42"))],
        absent: vec!["/coordinate/x", "/coordinate/y", "/coordinate/z"],
    });
    cases
}

#[test]
fn deterministic_context_gauntlet_first_fixture() {
    for fixture in fixtures() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "relay-context-gauntlet-{}-{}-{suffix}",
            std::process::id(),
            fixture.name
        ));
        let runtime = RuntimeContext::in_process();
        let core = RelayCore::open(CoreConfig::new(&dir));
        let original_payload = fixture.payload.clone();
        let mut put = request(
            "REQ-gauntlet-put",
            "result.put",
            json!({
                "kind": "TEST",
                "payload": fixture.payload
            }),
        );
        put.idempotency_key = Some("IDEMP-gauntlet-put".into());
        let stored = core.execute(put, &runtime);
        assert!(stored.ok, "{}: {:?}", fixture.name, stored.error);
        let result_id = stored.result.unwrap()["id"].as_str().unwrap().to_string();
        let full = core.execute(
            request(
                "REQ-gauntlet-full",
                "result.get",
                json!({
                    "result_id": result_id
                }),
            ),
            &runtime,
        );
        assert!(full.ok, "{}: {:?}", fixture.name, full.error);
        let full = full.result.unwrap();
        assert_eq!(full["payload"], original_payload, "{}", fixture.name);
        let full_bytes = serde_json::to_vec(&full["payload"]).unwrap().len();
        let context_request = || {
            request(
                "REQ-gauntlet-context",
                "result.context",
                json!({
                    "result_id": result_id, "max_bytes": BUDGET
                }),
            )
        };
        let compiled = core.execute(context_request(), &runtime);
        assert!(compiled.ok, "{}: {:?}", fixture.name, compiled.error);
        let compact = compiled.result.unwrap();
        let context_bytes = serde_json::to_vec(&compact).unwrap().len();
        assert!(context_bytes <= BUDGET, "{}", fixture.name);
        assert_eq!(compact["payload_bytes"], full_bytes, "{}", fixture.name);
        assert_eq!(compact["result_id"], result_id, "{}", fixture.name);
        assert_eq!(compact["payload_sha256"], full["payload_sha256"]);
        assert_eq!(compact["full_result_command"], "result.get");
        let repeated = core.execute(context_request(), &runtime);
        assert!(repeated.ok);
        assert_eq!(
            serde_json::to_vec(&compact).unwrap(),
            serde_json::to_vec(&repeated.result.unwrap()).unwrap(),
            "{} must serialize identically on repeat",
            fixture.name
        );

        let facts = compact["facts"].as_array().unwrap();
        for (suffix, expected) in &fixture.retained {
            let pointer = if suffix.starts_with("/entries/") {
                let target_index = match fixture.name {
                    "target_at_start" => 0,
                    "target_in_middle" => 50,
                    "target_at_end" => 100,
                    _ => unreachable!(),
                };
                suffix.replacen("/entries/", &format!("/entries/{target_index}/"), 1)
            } else {
                suffix.to_string()
            };
            assert_eq!(
                full["payload"].pointer(&pointer),
                Some(expected),
                "{} source payload lacks {}",
                fixture.name,
                pointer
            );
            assert!(
                facts
                    .iter()
                    .any(|fact| fact["pointer"] == pointer && fact["value"] == *expected),
                "{} lost exact value at {}",
                fixture.name,
                pointer
            );
        }
        for pointer in &fixture.absent {
            assert!(
                full["payload"].pointer(pointer).is_some(),
                "{} source payload lacks {}",
                fixture.name,
                pointer
            );
            assert!(
                facts.iter().all(|fact| fact["pointer"] != *pointer),
                "{} unexpectedly retained {}",
                fixture.name,
                pointer
            );
        }
        println!(
            "{}",
            json!({
                "fixture": fixture.name,
                "full_payload_bytes": full_bytes,
                "context_body_bytes": context_bytes,
                "budget_bytes": BUDGET,
                "retained_assertions": fixture.retained.len(),
                "expected_absences": fixture.absent.len(),
                "selected_facts": facts.len(),
                "omitted_scalars": compact["omitted_scalar_count"],
                "truncated": compact["truncated"]
            })
        );
        drop(core);
        fs::remove_dir_all(dir).unwrap();
    }
}
