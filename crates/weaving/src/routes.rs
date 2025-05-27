use std::{fs, io, path::Path, sync::Arc};

use crossbeam_channel::{Sender, unbounded};
use owo_colors::OwoColorize;
use rouille::{
    Request, Response,
    websocket::{self, Message},
};
use tokio::sync::Mutex;
use weaver_lib::Weaver;

use crate::sanitize_path;

pub fn serve_index(safe_path: &Path, _request: &Request) -> Response {
    let instance = Weaver::new(safe_path.to_path_buf());
    let site_index_path = format!(
        "{}/{}/index.html",
        safe_path.display(),
        instance.config.build_dir
    );
    match fs::read_to_string(&site_index_path) {
        Ok(mut content) => {
            let script = include_str!("../assets/inject-page.js");
            let sw_script = format!("<script>{}</script>", script);
            content = content.replace("</body>", &format!("{}</body>", sw_script));
            Response::html(content)
        }
        Err(err) => {
            eprintln!("Error reading index.html: {}", err);
            Response::text("Error: Could not load index.html").with_status_code(500)
        }
    }
}

pub fn serve_service_worker(safe_path: &Path) -> Response {
    let instance = Weaver::new(safe_path.to_path_buf());
    let serve_address = instance.config.serve_config.address.clone();
    println!("{}", "Serving service worker".green());
    let worker_code = include_str!("../assets/service-worker.js");
    Response::text(worker_code.replace("{SERVE_ADDRESS}", serve_address.as_str()))
        .with_additional_header("Content-Type", "application/javascript")
}

pub fn serve_websocket(
    request: &Request,
    clients: Arc<tokio::sync::Mutex<Vec<Sender<Message>>>>, // Example using tokio::sync::Mutex
    tokio_handle: tokio::runtime::Handle,
) -> Response {
    println!("{}", "Attempting to serve websocket".green());

    match websocket::start::<String>(request, None) {
        Ok((response_for_client, websocket_connection)) => {
            // Renamed and made mutable
            let clients_for_ws_thread = clients.clone();

            tokio_handle.spawn(async move {
                // `websocket_connection` is MOVED here
                println!("[WS Handler] New WebSocket connection established!");

                let (tx_for_broadcast_list, _rx_unused) = unbounded::<Message>();
                {
                    let mut guard = clients_for_ws_thread.lock().await;
                    guard.push(tx_for_broadcast_list.clone());
                }

                println!("[WS Handler] Listening for messages...");
                loop {
                    match websocket_connection.recv() {
                        Ok(mut websocket) => {
                            while let Some(message) = websocket.next() {
                                match message {
                                    Message::Text(txt) => {
                                        println!("[WS Handler] Received text: {}", txt.cyan());
                                        // Example: Echo the message back
                                        if let Err(e) =
                                            websocket.send_text(&format!("Echo: {}", txt))
                                        {
                                            println!(
                                                "[WS Handler] Error sending echo: {:?}",
                                                e.red()
                                            );
                                            break; // Stop on send error
                                        }
                                    }
                                    Message::Binary(data) => {
                                        println!(
                                            "[WS Handler] Received binary message (len: {})",
                                            data.len()
                                        );
                                        // Example: Echo the binary message back
                                        if let Err(e) = websocket.send_binary(&data) {
                                            println!(
                                                "[WS Handler] Error sending binary echo: {:?}",
                                                e.red()
                                            );
                                            break; // Stop on send error
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("[WS Handler] Error receiving message: {:?}", e.red());
                            break; // Other error
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

pub fn serve_catchall(safe_path: &Path, request: &Request) -> Response {
    let req_path = request.url();
    let instance = Weaver::new(safe_path.to_path_buf());
    println!(
        "Received {} request for: {}",
        request.method().blue(),
        req_path.yellow()
    );

    let sanitized_req_path = sanitize_path(&req_path);

    let mut file_path = format!(
        "{}/{}",
        instance.config.build_dir,
        &sanitized_req_path.display()
    );

    if req_path.ends_with('/') || req_path == "/" {
        file_path = format!("{}index.html", file_path);
    }

    println!("Serving: {:?}", &file_path.green());

    match fs::read_to_string(&file_path) {
        Ok(mut content) => {
            let mime_type = mime_guess::from_path(&file_path).first_or_octet_stream();
            let script = include_str!("../assets/inject-page.js");
            let sw_script = format!("<script>{}</script>", script);
            content = content.replace("</body>", &format!("{}</body>", sw_script));

            Response::from_data(mime_type.to_string(), content)
        }
        Err(err) => {
            let status = match err.kind() {
                io::ErrorKind::NotFound => 404,
                _ => 500,
            };

            eprintln!("Error reading file {:?}: {}", file_path.yellow(), err.red());
            Response::text(format!("Error: {}", err)).with_status_code(status)
        }
    }
}
