use std::process::{Child, Command};
use std::sync::Arc;
use std::time::Duration;

use loro::{LoroDoc, LoroValue};
use loro_websocket_client::LoroWebsocketClient;
use tokio::sync::Mutex;
use tokio::time::sleep;

fn map_value(doc: &LoroDoc, key: &str) -> Option<String> {
    let map = doc.get_map("translations");
    map.get(key).and_then(|v| match v.get_deep_value() {
        LoroValue::String(s) => Some(s.as_ref().to_owned()),
        _ => None,
    })
}

async fn wait_for_value(doc: &Arc<Mutex<LoroDoc>>, key: &str, expected: &str) {
    for _ in 0..100 {
        let guard = doc.lock().await;
        let ok = map_value(&guard, key).as_deref() == Some(expected);
        drop(guard);
        if ok {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    let guard = doc.lock().await;
    panic!(
        "timed out waiting for {key}={expected:?}, got {:?}",
        map_value(&guard, key)
    );
}

async fn wait_health(port: u16) {
    for _ in 0..100 {
        if reqwest::get(format!("http://127.0.0.1:{port}/"))
            .await
            .is_ok()
        {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("relay health did not become ready");
}

fn spawn_relay(port: u16, health_port: u16, db: &std::path::Path) -> Child {
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/sync-relay/Cargo.toml");
    Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&manifest)
        .env("SLIM_SYNC_BIND", format!("127.0.0.1:{port}"))
        .env("SLIM_SYNC_HEALTH_BIND", format!("127.0.0.1:{health_port}"))
        .env("SLIM_SYNC_DB", db)
        .env("SLIM_SYNC_MODE", "development")
        .spawn()
        .expect("spawn sync relay")
}

#[tokio::test(flavor = "multi_thread")]
async fn relay_round_trips_loro_updates() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("sync.sqlite");
    let port = 8792u16;
    let health_port = 8793u16;

    let mut relay = spawn_relay(port, health_port, &db);
    wait_health(health_port).await;

    let client_a = LoroWebsocketClient::connect(&format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    let doc_a = Arc::new(Mutex::new(LoroDoc::new()));
    let _room_a = client_a
        .join_loro("relay-room", doc_a.clone())
        .await
        .unwrap();

    let client_b = LoroWebsocketClient::connect(&format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    let doc_b = Arc::new(Mutex::new(LoroDoc::new()));
    let _room_b = client_b
        .join_loro("relay-room", doc_b.clone())
        .await
        .unwrap();

    // A -> relay -> B
    {
        let doc = doc_a.lock().await;
        doc.get_map("translations")
            .insert("welcome", "Hola A")
            .unwrap();
        doc.commit();
    }
    wait_for_value(&doc_b, "welcome", "Hola A").await;

    // B -> relay -> A
    {
        let doc = doc_b.lock().await;
        doc.get_map("translations")
            .insert("cancel", "Cancelar B")
            .unwrap();
        doc.commit();
    }
    wait_for_value(&doc_a, "cancel", "Cancelar B").await;

    // Offline B: disconnect, edit locally, then reconnect and push stored update.
    drop(_room_b);
    drop(client_b);
    {
        let doc = doc_b.lock().await;
        doc.get_map("translations")
            .insert("offline", "Editado offline")
            .unwrap();
        doc.commit();
    }
    let offline_update = {
        let g = doc_b.lock().await;
        g.export(loro::ExportMode::all_updates()).unwrap()
    };

    let client_b2 = LoroWebsocketClient::connect(&format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    let _room_b2 = client_b2
        .join_loro("relay-room", doc_b.clone())
        .await
        .unwrap();

    // Push the update that happened while disconnected.
    let mut low = loro_websocket_client::Client::connect(&format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    let join = loro_websocket_client::protocol::ProtocolMessage::JoinRequest {
        crdt: loro_websocket_client::protocol::CrdtType::Loro,
        room_id: "relay-room".to_string(),
        auth: Vec::new(),
        version: doc_b.lock().await.oplog_vv().encode(),
    };
    low.send(&join).await.unwrap();
    loop {
        match low.next().await.unwrap() {
            Some(loro_websocket_client::protocol::ProtocolMessage::JoinResponseOk { .. }) => break,
            Some(_) => {}
            None => panic!("relay closed during join"),
        }
    }
    low.send(
        &loro_websocket_client::protocol::ProtocolMessage::DocUpdate {
            crdt: loro_websocket_client::protocol::CrdtType::Loro,
            room_id: "relay-room".to_string(),
            updates: vec![offline_update],
        },
    )
    .await
    .unwrap();
    low.close().await.unwrap();

    wait_for_value(&doc_a, "offline", "Editado offline").await;

    // Relay restart and reconnect.
    relay.kill().unwrap();
    let _ = relay.wait();
    let mut relay2 = spawn_relay(port, health_port, &db);
    wait_health(health_port).await;

    let client_a2 = LoroWebsocketClient::connect(&format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    let _room_a2 = client_a2
        .join_loro("relay-room", doc_a.clone())
        .await
        .unwrap();
    let client_b3 = LoroWebsocketClient::connect(&format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    let _room_b3 = client_b3
        .join_loro("relay-room", doc_b.clone())
        .await
        .unwrap();

    // Both peers should still see all keys after relay restart.
    assert_eq!(
        map_value(&*doc_a.lock().await, "welcome").as_deref(),
        Some("Hola A")
    );
    assert_eq!(
        map_value(&*doc_b.lock().await, "welcome").as_deref(),
        Some("Hola A")
    );
    assert_eq!(
        map_value(&*doc_a.lock().await, "offline").as_deref(),
        Some("Editado offline")
    );
    assert_eq!(
        map_value(&*doc_b.lock().await, "offline").as_deref(),
        Some("Editado offline")
    );

    relay2.kill().unwrap();
    let _ = relay2.wait();
}
