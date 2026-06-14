use std::sync::Arc;
use sysinfo::Networks;
use crate::AppState;

/// Periodically refreshes network interface data and broadcasts real-time bandwidth usage over WebSocket.
pub async fn run_network_monitor(state: Arc<AppState>) {
    tracing::info!("📶 Network monitor started: tracking network interfaces");

    let mut networks = Networks::new_with_refreshed_list();

    loop {
        // Sleep for 1 second to calculate bytes per second
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        networks.refresh();

        let mut interfaces_data = serde_json::json!({});

        for (interface_name, data) in &networks {
            // Ignore loopback interface
            if interface_name == "lo" || interface_name.starts_with("veth") || interface_name.starts_with("br-") {
                continue;
            }

            let rx_bytes = data.received(); // Bytes received since last refresh (1s ago)
            let tx_bytes = data.transmitted(); // Bytes transmitted since last refresh (1s ago)

            interfaces_data[interface_name] = serde_json::json!({
                "rx_bytes_sec": rx_bytes,
                "tx_bytes_sec": tx_bytes,
            });
        }

        // Broadcast to all WebSocket clients
        if let Ok(json) = serde_json::to_string(&serde_json::json!({
            "type": "network_traffic",
            "interfaces": interfaces_data,
        })) {
            let _ = state.tx_broadcast.send(json);
        }
    }
}
