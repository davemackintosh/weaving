use std::{
    fs::{self, File},
    io::{self, Cursor},
    path::Path,
    sync::Arc,
};

use owo_colors::OwoColorize;
use rouille::{
    Request, Response,
    websocket::{self, Message},
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use weaver_lib::Weaver;

use crate::sanitize_path;

pub fn serve_websocket(
    request: &Request,
    clients: Arc<tokio::sync::Mutex<Vec<UnboundedSender<Message>>>>, // Example using tokio::sync::Mutex
    tokio_handle: tokio::runtime::Handle,
) -> Response {
    println!("{}", "Attempting to serve websocket".green());

    match websocket::start::<String>(request, None) {
        Ok((response_for_client, ws_object_receiver)) => {
            let clients_for_ws_thread = clients.clone();

            tokio_handle.spawn(async move {
                println!("[WS Handler] New WebSocket connection established!");

                let (tx_for_broadcast_list, mut rx_for_broadcast_list) = unbounded_channel();
                {
                    let mut guard = clients_for_ws_thread.lock().await;
                    guard.push(tx_for_broadcast_list.clone());
                    println!(
                        "[WS Setup] Added client's broadcast sender. Total Senders: {}",
                        guard.len()
                    );
                }

                println!("[WS Handler] Attempting to receive actual WebSocket object from initial receiver...");
                let mut actual_network_conn = match ws_object_receiver.recv() {
                    Ok(conn) => {
                        println!("[WS Handler] Successfully received WebSocket object.");
                        conn
                    }
                    Err(e) => {
                        eprintln!("[WS Handler] Failed to receive WebSocket object from initial receiver: {:?}. Terminating task.", e.red());
                        let mut guard = clients_for_ws_thread.lock().await;
                        guard.retain(|s| !s.same_channel(&tx_for_broadcast_list));
                        return;
                    }
                };
                println!("[WS Handler] Worker started with actual WebSocket object.");

                println!("[WS Handler] Sending 'hello' message...");
                if let Err(e) = actual_network_conn.send_text("hello") {
                    eprintln!("[WS Handler] Failed to send 'hello': {:?}. Closing.", e.red());
                    let mut guard = clients_for_ws_thread.lock().await;
                    guard.retain(|s| !s.same_channel(&tx_for_broadcast_list));
                    return;
                }
                println!("[WS Handler] 'hello' sent.");

                println!("[WS Handler] Listening for messages...");
                while let Some(message) = rx_for_broadcast_list.recv().await {
                            match message {
                                Message::Text(txt) => {
                                    println!("[WS Handler] Received text: {}", txt.cyan());
                                    if let Err(e) =
                                        actual_network_conn.send_text(&txt)
                                    {
                                        println!("[WS Handler] Error sending echo: {:?}", e.red());
                                        break;
                                    }
                                }
                                Message::Binary(data) => {
                                    println!(
                                        "[WS Handler] Received binary message (len: {})",
                                        data.len()
                                    );
                                    if let Err(e) = actual_network_conn.send_binary(&data) {
                                        println!(
                                            "[WS Handler] Error sending binary echo: {:?}",
                                            e.red()
                                        );
                                        break;
                                    }
                                }
                            }
                }

                println!("[WS Handler] WebSocket connection processing finished.");
            });

            println!("[WS Server] Spawned handler, returning 101 Switching Protocols.");
            response_for_client
        }
        Err(e) => {
            let error_message = format!("WebSocket upgrade failed: {:?}", e);
            println!("{}", error_message.red());
            Response::text(error_message).with_status_code(400)
        }
    }
}

fn is_probably_binary(path: String) -> std::io::Result<bool> {
    let content = fs::read(path)?;
    let reader = Cursor::new(content);

    match bindet::detect(reader) {
        Ok(Some(_matches)) => Ok(true),
        Ok(None) => Ok(false),
        Err(_e) => Ok(false),
    }
}

pub fn serve_catchall(safe_path: &Path, request: &Request) -> Response {
    let req_path = request.url();
    let instance = Weaver::new(safe_path.to_path_buf());
    println!(
        "Received {} request for: {}",
        request.method().blue(),
        req_path.yellow()
    );

    let sanitized_req_path = sanitize_path(&req_path, false);
    let public_root = instance
        .config
        .public_dir
        .strip_prefix(&instance.config.base_dir)
        .unwrap();

    let mut file_path = sanitize_path(
        format!(
            "/{}/{}",
            instance.config.build_dir,
            &sanitized_req_path.display()
        )
        .as_str(),
        true,
    );

    file_path = if req_path.ends_with('/') || req_path == "/" {
        format!("{}/index.html", file_path.display()).into()
    } else if req_path.starts_with(public_root) {
        file_path
    } else if !file_path.exists() || file_path.is_dir() {
        format!("{}/index.html", file_path.display()).into()
    } else {
        file_path
    };

    println!("Serving: {:?}", &file_path.green());
    let serve_address = instance.config.serve_config.address.clone();

    if let Ok(is_binary) = is_probably_binary(file_path.to_string_lossy().to_string()) {
        if is_binary {
            let mime_type = mime_guess::from_path(&file_path).first_or_octet_stream();
            return Response::from_file(
                mime_type.to_string(),
                File::open(&file_path).unwrap_or_else(|_| {
                    panic!("failed to open {} for reading.", file_path.display())
                }),
            );
        }
    }

    match fs::read_to_string(&file_path) {
        Ok(mut content) => {
            let mime_type = mime_guess::from_path(&file_path).first_or_octet_stream();
            let script = include_str!("../assets/inject-page.js")
                .replace("{SERVE_ADDRESS}", serve_address.as_str());
            let sw_script = format!("<script>{}</script>", script);
            content = content.replace("</body>", &format!("{}</body>", sw_script));

            Response::from_data(mime_type.to_string(), content)
        }
        Err(err) => {
            eprintln!("Error reading file {:?}: {}", file_path.yellow(), err.red());
            let status = match err.kind() {
                io::ErrorKind::NotFound => 404,
                _ => 500,
            };

            // If there's a custom 404 page, render that instead.
            if status == 404
                && !req_path.starts_with("/404")
                && fs::exists(format!("{}/404.md", instance.config.content_dir)).unwrap()
            {
                let new_request = Request::fake_http("GET", "/404", vec![], vec![]);
                return serve_catchall(safe_path, &new_request);
            }

            Response::text(format!("Error: {}", err)).with_status_code(status)
        }
    }
}
